import SwiftUI

/// Read-along focus for one block: which sentence (if any) is being spoken.
enum SentenceFocus: Equatable {
    case inactive        // nothing playing — show the whole transcript
    case focused(Int)    // this block holds the spoken sentence at that index
    case dimmed          // the spoken sentence is in another block

    var isActive: Bool { self != .dimmed }
}

/// The read-along transcript. Focus advances at sentence granularity (an honest
/// approximation, since the contract carries no per-word timing): the spoken
/// sentence is primary, the rest recede. Inline `[label](attachment:)`
/// references resolve to a file attachment (image inline / tappable link) or to
/// a narrated child branch (tappable → opens that branch in the same player).
struct TranscriptBlocks: View {
    let document: TranscriptDocument
    let focus: TranscriptFocus?
    let attachments: [DurableArtifact]
    let children: [SpokenItem]
    let onSeek: (TranscriptBlock) -> Void
    let onOpenAttachment: (DurableArtifact) -> Void
    let onOpenChild: (SpokenItem) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            ForEach(document.blocks) { block in
                TranscriptBlockView(
                    block: block,
                    focus: sentenceFocus(for: block),
                    attachments: attachments,
                    children: children,
                    onOpenAttachment: onOpenAttachment
                )
                .id(block.id)
                .contentShape(Rectangle())
                .onTapGesture { onSeek(block) }
            }
        }
        .padding(.horizontal, 20)
        .environment(\.openURL, OpenURLAction { url in
            switch url.scheme {
            case AttachmentLink.attachmentScheme:
                if let i = Int(url.lastPathComponent), attachments.indices.contains(i) {
                    onOpenAttachment(attachments[i]); return .handled
                }
            case AttachmentLink.childScheme:
                if let i = Int(url.lastPathComponent), children.indices.contains(i) {
                    onOpenChild(children[i]); return .handled
                }
            default:
                break
            }
            return .systemAction
        })
    }

    private func sentenceFocus(for block: TranscriptBlock) -> SentenceFocus {
        guard let focus else { return .inactive }
        return focus.block == block.id ? .focused(focus.sentence) : .dimmed
    }
}

private struct TranscriptBlockView: View {
    let block: TranscriptBlock
    let focus: SentenceFocus
    let attachments: [DurableArtifact]
    let children: [SpokenItem]
    let onOpenAttachment: (DurableArtifact) -> Void

    var body: some View {
        content
            .frame(maxWidth: .infinity, alignment: .leading)
            .animation(.easeInOut(duration: 0.35), value: focus)
            .accessibilityAddTraits(focus.isActive ? .isSelected : [])
    }

    @ViewBuilder
    private var content: some View {
        switch block.kind {
        case let .heading(level):
            Text(inline(block.text))
                .font(level <= 1 ? .title3.bold() : .headline)
                .foregroundStyle(focus.isActive ? AnyShapeStyle(.primary) : AnyShapeStyle(.secondary))
                .padding(.top, 4)
        case .paragraph:
            textWithInlineImages(block.text, font: .body)
        case .bullet:
            marker("•", block.text)
        case let .ordered(number):
            marker(number, block.text)
        case .quote:
            Text(styled(block.text))
                .font(.body.italic())
                .tint(.accentColor)
                .padding(.leading, 12)
                .overlay(alignment: .leading) {
                    Capsule().fill(Color.accentColor.opacity(0.5)).frame(width: 3)
                }
        case .code:
            Text(block.text)
                .font(.callout.monospaced())
                .foregroundStyle(focus.isActive ? AnyShapeStyle(.primary) : AnyShapeStyle(.secondary))
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.primary.opacity(0.06), in: RoundedRectangle(cornerRadius: 10))
        }
    }

    private func marker(_ glyph: String, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(glyph)
                .font(.body.monospacedDigit())
                .foregroundStyle(focus.isActive ? AnyShapeStyle(.secondary) : AnyShapeStyle(.tertiary))
            textWithInlineImages(text, font: .body)
        }
    }

    @ViewBuilder
    private func textWithInlineImages(_ text: String, font: Font) -> some View {
        let referenced = AttachmentLink.referencedImages(in: text, attachments: attachments)
        VStack(alignment: .leading, spacing: 10) {
            Text(styled(text)).font(font).lineSpacing(4).tint(.accentColor)
            ForEach(referenced) { image in
                InlineAttachmentImage(attachment: image) { onOpenAttachment(image) }
                    .opacity(focus.isActive ? 1 : 0.55)
            }
        }
    }

    /// Builds the block's text with the spoken sentence primary and the rest
    /// secondary, preserving inline attachment links.
    private func styled(_ text: String) -> AttributedString {
        let sentences = Sentences.split(text)
        var result = AttributedString()
        for (index, sentence) in sentences.enumerated() {
            var attributed = inline(sentence)
            let active: Bool
            switch focus {
            case .inactive: active = true
            case let .focused(spoken): active = index == spoken
            case .dimmed: active = false
            }
            if !active { attributed.foregroundColor = .secondary }
            if index > 0 { result += AttributedString(" ") }
            result += attributed
        }
        return result
    }

    private func inline(_ text: String) -> AttributedString {
        let rewritten = AttachmentLink.rewrite(text, attachments: attachments, children: children)
        let options = AttributedString.MarkdownParsingOptions(
            interpretedSyntax: .inlineOnlyPreservingWhitespace
        )
        return (try? AttributedString(markdown: rewritten, options: options)) ?? AttributedString(text)
    }
}

private struct InlineAttachmentImage: View {
    let attachment: DurableArtifact
    let onOpen: () -> Void

    var body: some View {
        Button(action: onOpen) {
            AsyncImage(url: URL(string: attachment.url)) { image in
                image.resizable().scaledToFit()
            } placeholder: {
                RoundedRectangle(cornerRadius: 12)
                    .fill(Color.secondary.opacity(0.12))
                    .frame(height: 160)
                    .overlay(ProgressView())
            }
            .frame(maxWidth: .infinity)
            .frame(maxHeight: 240)
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Image attachment \(attachment.label ?? "")")
    }
}

/// Encodes and resolves inline `[label](attachment:)` references against both
/// file attachments and narrated child branches.
enum AttachmentLink {
    static let attachmentScheme = "ttsattach"
    static let childScheme = "ttschild"

    static func rewrite(_ text: String, attachments: [DurableArtifact], children: [SpokenItem]) -> String {
        matches(in: text).reversed().reduce(text) { current, match in
            let replacement: String
            if let index = attachments.firstIndex(where: { $0.label == match.label }) {
                replacement = "[\(match.label)](\(attachmentScheme)://a/\(index))"
            } else if let index = children.firstIndex(where: { $0.subject == match.label }) {
                replacement = "[\(match.label)](\(childScheme)://c/\(index))"
            } else {
                return current
            }
            return (current as NSString).replacingCharacters(in: match.range, with: replacement)
        }
    }

    static func referencedImages(in text: String, attachments: [DurableArtifact]) -> [DurableArtifact] {
        var seen = Set<Int>()
        var images: [DurableArtifact] = []
        for match in matches(in: text) {
            guard let index = attachments.firstIndex(where: { $0.label == match.label }),
                  !seen.contains(index),
                  AttachmentKind(mediaType: attachments[index].mediaType) == .image else { continue }
            seen.insert(index)
            images.append(attachments[index])
        }
        return images
    }

    private struct Match { let range: NSRange; let label: String }

    private static func matches(in text: String) -> [Match] {
        guard let regex = try? NSRegularExpression(pattern: "\\[([^\\]]+)\\]\\(attachment:\\)") else {
            return []
        }
        let ns = text as NSString
        return regex.matches(in: text, range: NSRange(location: 0, length: ns.length)).map {
            Match(range: $0.range, label: ns.substring(with: $0.range(at: 1)))
        }
    }
}
