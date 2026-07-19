import SwiftUI

/// A deterministic agent avatar: a pubkey-derived gradient with initials, and
/// an optional accent dot marking an item the viewer has not heard yet.
struct AgentAvatar: View {
    let identity: AgentIdentity
    var size: CGFloat = 32
    var showsUnheard: Bool = false

    var body: some View {
        Circle()
            .fill(identity.gradient)
            .frame(width: size, height: size)
            .overlay(
                Text(identity.initials)
                    .font(.system(size: size * 0.4, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
                    .shadow(color: .black.opacity(0.25), radius: 1, y: 0.5)
            )
            .overlay(alignment: .topTrailing) {
                if showsUnheard {
                    Circle()
                        .fill(Color.accentColor)
                        .frame(width: size * 0.28, height: size * 0.28)
                        .overlay(Circle().stroke(Color.systemBackgroundCompat, lineWidth: 1.5))
                        .offset(x: size * 0.04, y: -size * 0.04)
                }
            }
            .accessibilityHidden(true)
    }
}

/// A compact reaction tally (👍 3). Read-only for now.
struct ReactionChip: View {
    let reaction: ReactionSummary

    var body: some View {
        HStack(spacing: 3) {
            Text(reaction.emoji)
            if reaction.count > 1 {
                Text("\(reaction.count)")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
        .font(.caption)
        .padding(.horizontal, 7)
        .padding(.vertical, 3)
        .background(.ultraThinMaterial, in: Capsule())
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(reaction.count) \(reaction.emoji) reactions")
    }
}

/// The small trailing badges on a queue row: attachments, questions, answer.
struct ItemBadges: View {
    let item: SpokenItem

    var body: some View {
        HStack(spacing: 8) {
            if item.hasAttachments {
                Label("\(item.attachments.count)", systemImage: "paperclip")
                    .accessibilityLabel("\(item.attachments.count) attachments")
            }
            if item.hasQuestions {
                Image(systemName: item.isAnswered ? "checkmark.bubble" : "questionmark.bubble")
                    .foregroundStyle(item.isAnswered ? Color.secondary : Color.accentColor)
                    .accessibilityLabel(item.isAnswered ? "Answered" : "Awaiting answer")
            }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .labelStyle(.compact)
    }
}

private struct CompactLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 2) {
            configuration.icon
            configuration.title
        }
    }
}

extension LabelStyle where Self == CompactLabelStyle {
    static var compact: CompactLabelStyle { CompactLabelStyle() }
}

extension ShapeStyle where Self == Color {
    /// A cross-platform "system background" for hairline strokes.
    static var systemBackgroundCompat: Color {
        #if os(iOS)
        Color(uiColor: .systemBackground)
        #else
        Color(nsColor: .windowBackgroundColor)
        #endif
    }
}
