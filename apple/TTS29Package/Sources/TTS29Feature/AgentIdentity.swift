import SwiftUI

/// A deterministic visual identity derived from an item's signing pubkey and
/// agent name. The pubkey is what actually distinguishes one producer from
/// another, so we turn it into a stable gradient — provenance you can learn as
/// a colour, with no image assets and nothing to configure.
public struct AgentIdentity: Equatable, Sendable {
    public let agentName: String
    public let author: String

    public init(agentName: String, author: String) {
        self.agentName = agentName
        self.author = author
    }

    public init(_ item: SpokenItem) {
        self.init(agentName: item.agentName, author: item.author)
    }

    /// The name to show. Falls back to a shortened pubkey when no agent name
    /// was attributed.
    public var displayName: String {
        let trimmed = agentName.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? shortAuthor : trimmed
    }

    /// A compact, monospace-friendly rendering of the pubkey for provenance.
    public var shortAuthor: String {
        guard author.count > 12 else { return author.isEmpty ? "unknown" : author }
        let head = author.prefix(6)
        let tail = author.suffix(4)
        return "\(head)…\(tail)"
    }

    /// One or two initials for the avatar, taken from the agent name when
    /// present and otherwise from the pubkey.
    public var initials: String {
        let words = agentName
            .split(whereSeparator: { $0 == " " || $0 == "-" || $0 == "_" })
            .prefix(2)
        if let letters = Optional(words.compactMap(\.first)), !letters.isEmpty {
            return String(letters).uppercased()
        }
        return String(author.prefix(2)).uppercased()
    }

    private var seed: UInt64 {
        // FNV-1a over the identity so the gradient is stable across launches
        // and independent of Swift's per-process Hasher seed.
        let source = author.isEmpty ? agentName : author
        var hash: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in source.utf8 {
            hash ^= UInt64(byte)
            hash = hash &* 0x0000_0100_0000_01B3
        }
        return hash
    }

    private func hue(_ shift: UInt64) -> Double {
        Double((seed >> shift) & 0xFFFF) / Double(0xFFFF)
    }

    /// Two clamped hues that read on both light and dark backgrounds.
    public var gradientColors: [Color] {
        let first = hue(0)
        let second = (first + 0.12 + hue(24) * 0.18).truncatingRemainder(dividingBy: 1)
        return [
            Color(hue: first, saturation: 0.55, brightness: 0.82),
            Color(hue: second, saturation: 0.68, brightness: 0.62),
        ]
    }

    public var gradient: LinearGradient {
        LinearGradient(
            colors: gradientColors,
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }
}
