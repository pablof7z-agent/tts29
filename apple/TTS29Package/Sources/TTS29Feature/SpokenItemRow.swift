import SwiftUI

/// A quiet queue row. Playback UI lives in the mini-player and item surface, so
/// the row only signals identity, content, unheard/playing state, and badges.
/// Tapping the row opens the item; the small trailing control plays it directly.
struct SpokenItemRow: View {
    let item: SpokenItem
    let playback: PlaybackController
    let onOpen: () -> Void
    let onPlay: () -> Void

    private var identity: AgentIdentity { AgentIdentity(item) }
    private var isPlaying: Bool { playback.isActive(item) && playback.isPlaying }
    private var isArchived: Bool {
        item.acknowledgement?.state == .archived || item.acknowledgement?.state == .dismissed
    }

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Button(action: onOpen) {
                HStack(alignment: .top, spacing: 12) {
                    AgentAvatar(identity: identity, size: 34, showsUnheard: !item.isHeard && !isArchived)
                        .padding(.top, 2)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(item.subject).font(.headline).lineLimit(1)
                        if !item.summary.isEmpty {
                            Text(item.summary)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                        metadata
                    }
                    Spacer(minLength: 4)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(accessibilityText)
            .accessibilityAddTraits(.isButton)

            trailing
        }
        .padding(.vertical, 6)
        .opacity(isArchived ? 0.55 : 1)
        .listRowBackground(rowBackground)
    }

    private var metadata: some View {
        HStack(spacing: 6) {
            Text(identity.displayName).lineLimit(1)
            Text("·")
            Text(Formatting.timestamp(item.createdDate))
            ItemBadges(item: item)
            if let top = item.reactions.first, top.count > 0 {
                ReactionChip(reaction: top).font(.caption2)
            }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .padding(.top, 1)
    }

    @ViewBuilder
    private var trailing: some View {
        if isPlaying {
            Image(systemName: "waveform")
                .font(.title3)
                .foregroundStyle(Color.accentColor)
                .symbolEffect(.variableColor.iterative, isActive: true)
                .frame(width: 34, height: 34)
                .accessibilityHidden(true)
        } else {
            Button(action: onPlay) {
                Image(systemName: playback.isActive(item) && playback.phase == .failed
                    ? "exclamationmark.circle" : "play.circle")
                    .font(.title2)
                    .foregroundStyle(.secondary)
                    .frame(width: 34, height: 34)
            }
            .buttonStyle(.plain)
            .disabled(item.playableURL == nil)
            .accessibilityLabel(playback.label(for: item))
            .accessibilityIdentifier("tts29.play.\(item.id)")
        }
    }

    private var rowBackground: some View {
        (playback.isActive(item) ? Color.accentColor.opacity(0.06) : Color.clear)
            .animation(.easeInOut(duration: 0.25), value: playback.isActive(item))
    }

    private var accessibilityText: String {
        var parts = [item.subject, item.summary, "from \(identity.displayName)",
                     Formatting.timestamp(item.createdDate)]
        if !item.isHeard && !isArchived { parts.append("unheard") }
        if item.hasAttachments { parts.append("\(item.attachments.count) attachments") }
        if item.hasQuestions { parts.append(item.isAnswered ? "answered" : "awaiting answer") }
        return parts.filter { !$0.isEmpty }.joined(separator: ", ")
    }
}
