import Foundation

public enum TranscriptBlockKind: Equatable, Sendable {
    case heading(Int)
    case paragraph
    case bullet
    case ordered(String)
    case quote
    case code
}

/// One renderable unit of a transcript. Blocks carry the audio fraction they
/// are estimated to span, so playback progress can softly focus the passage
/// most likely being spoken — an honest approximation, since the contract
/// carries no per-word synthesis timing.
public struct TranscriptBlock: Identifiable, Equatable, Sendable {
    public let id: Int
    public let kind: TranscriptBlockKind
    public let text: String
    public let speakableWeight: Int
    public var startFraction: Double
    public var endFraction: Double
}

/// The sentence estimated to be spoken at a given progress: which block, and
/// which sentence within it.
public struct TranscriptFocus: Equatable, Sendable {
    public let block: Int
    public let sentence: Int
}

public struct TranscriptDocument: Equatable, Sendable {
    public let blocks: [TranscriptBlock]
    private let units: [Unit]

    private struct Unit: Equatable, Sendable {
        let block: Int
        let sentence: Int
        let endFraction: Double
    }

    public init(_ body: String) {
        var raw = Self.segment(body)
        let total = max(raw.reduce(0) { $0 + $1.speakableWeight }, 1)
        var cursor = 0
        for index in raw.indices {
            let start = Double(cursor) / Double(total)
            cursor += raw[index].speakableWeight
            let end = Double(cursor) / Double(total)
            raw[index].startFraction = start
            raw[index].endFraction = index == raw.indices.last ? 1 : end
        }
        blocks = raw

        // Sub-divide each block into sentences so the highlight advances even
        // inside a single paragraph. Weights are shared with the block total.
        var units: [Unit] = []
        var sentenceCursor = 0
        for block in raw {
            let sentences = Self.sentences(in: block)
            for (offset, sentence) in sentences.enumerated() {
                sentenceCursor += max(Self.speakableWeight(sentence), 1)
                units.append(Unit(
                    block: block.id,
                    sentence: offset,
                    endFraction: Double(sentenceCursor) / Double(total)
                ))
            }
        }
        if let last = units.indices.last {
            units[last] = Unit(block: units[last].block, sentence: units[last].sentence, endFraction: 1)
        }
        self.units = units
    }

    public var isEmpty: Bool { blocks.isEmpty }

    /// The sentences a block is split into for read-along; code and headings
    /// stay whole.
    public static func sentences(in block: TranscriptBlock) -> [String] {
        switch block.kind {
        case .code, .heading:
            return [block.text]
        default:
            return Sentences.split(block.text)
        }
    }

    /// The block and sentence most likely being spoken at 0…1 progress.
    public func focus(at progress: Double) -> TranscriptFocus? {
        guard !units.isEmpty else { return nil }
        let clamped = min(max(progress, 0), 1)
        let unit = units.first { clamped < $0.endFraction } ?? units[units.count - 1]
        return TranscriptFocus(block: unit.block, sentence: unit.sentence)
    }

    /// The block most likely being spoken at the given 0…1 audio progress.
    public func focusedIndex(at progress: Double) -> Int? {
        guard !blocks.isEmpty else { return nil }
        let clamped = min(max(progress, 0), 1)
        if clamped >= 1 { return blocks.count - 1 }
        return blocks.firstIndex { clamped < $0.endFraction } ?? blocks.count - 1
    }

    public func startFraction(ofBlock id: Int) -> Double {
        blocks.first { $0.id == id }?.startFraction ?? 0
    }

    private static func segment(_ body: String) -> [TranscriptBlock] {
        var blocks: [TranscriptBlock] = []
        var paragraph: [String] = []
        var quote: [String] = []
        var code: [String] = []
        var inCode = false
        var next = 0

        func emit(_ kind: TranscriptBlockKind, _ text: String) {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty || kind == .code else { return }
            blocks.append(TranscriptBlock(
                id: next,
                kind: kind,
                text: text,
                speakableWeight: max(Self.speakableWeight(text), 1),
                startFraction: 0,
                endFraction: 0
            ))
            next += 1
        }
        func flushParagraph() {
            if !paragraph.isEmpty { emit(.paragraph, paragraph.joined(separator: " ")); paragraph = [] }
        }
        func flushQuote() {
            if !quote.isEmpty { emit(.quote, quote.joined(separator: " ")); quote = [] }
        }

        for line in body.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") {
                if inCode { emit(.code, code.joined(separator: "\n")); code = []; inCode = false }
                else { flushParagraph(); flushQuote(); inCode = true }
                continue
            }
            if inCode { code.append(line); continue }
            if trimmed.isEmpty { flushParagraph(); flushQuote(); continue }
            if let level = headingLevel(trimmed) {
                flushParagraph(); flushQuote()
                emit(.heading(level), String(trimmed.drop(while: { $0 == "#" }).drop(while: { $0 == " " })))
            } else if trimmed.hasPrefix("> ") {
                flushParagraph()
                quote.append(String(trimmed.dropFirst(2)))
            } else if let marker = orderedMarker(trimmed) {
                flushParagraph(); flushQuote()
                emit(.ordered(marker), String(trimmed.dropFirst(marker.count + 1).drop(while: { $0 == " " })))
            } else if trimmed.hasPrefix("- ") || trimmed.hasPrefix("* ") || trimmed.hasPrefix("+ ") {
                flushParagraph(); flushQuote()
                emit(.bullet, String(trimmed.dropFirst(2)))
            } else {
                flushQuote()
                paragraph.append(trimmed)
            }
        }
        if inCode { emit(.code, code.joined(separator: "\n")) }
        flushParagraph(); flushQuote()
        return blocks
    }

    private static func headingLevel(_ line: String) -> Int? {
        let hashes = line.prefix { $0 == "#" }.count
        guard hashes >= 1, hashes <= 6, line.dropFirst(hashes).first == " " else { return nil }
        return hashes
    }

    private static func orderedMarker(_ line: String) -> String? {
        let digits = line.prefix { $0.isNumber }
        guard !digits.isEmpty, line.dropFirst(digits.count).first == "." else { return nil }
        return "\(digits)."
    }

    /// Approximate spoken length: letters and digits only, so markdown syntax
    /// and punctuation do not skew the proportional mapping.
    static func speakableWeight(_ text: String) -> Int {
        text.unicodeScalars.reduce(0) { count, scalar in
            (CharacterSet.alphanumerics.contains(scalar)) ? count + 1 : count
        }
    }
}

/// Splits prose into sentences on terminal punctuation followed by whitespace.
/// An approximation — good enough to move the read-along highlight within a
/// paragraph without real per-word timing.
public enum Sentences {
    public static func split(_ text: String) -> [String] {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return [] }
        var sentences: [String] = []
        var start = trimmed.startIndex
        var index = trimmed.startIndex
        while index < trimmed.endIndex {
            let next = trimmed.index(after: index)
            if ".!?".contains(trimmed[index]),
               next == trimmed.endIndex || trimmed[next].isWhitespace {
                let sentence = trimmed[start...index].trimmingCharacters(in: .whitespaces)
                if !sentence.isEmpty { sentences.append(sentence) }
                var skip = next
                while skip < trimmed.endIndex, trimmed[skip].isWhitespace {
                    skip = trimmed.index(after: skip)
                }
                start = skip
                index = skip
            } else {
                index = next
            }
        }
        if start < trimmed.endIndex {
            let tail = trimmed[start...].trimmingCharacters(in: .whitespaces)
            if !tail.isEmpty { sentences.append(tail) }
        }
        return sentences.isEmpty ? [trimmed] : sentences
    }
}
