import SwiftUI

/// The floating glass mini-player docked at the base of the queue. It appears
/// only while an item is selected and is the tap target back into that item's
/// surface. A hairline along its base tracks progress without adding chrome.
struct MiniPlayerView: View {
    @Bindable var playback: PlaybackController
    let onOpen: (SpokenItem) -> Void

    var body: some View {
        if let item = playback.selectedItem {
            content(for: item)
                .padding(.horizontal, 16)
                .padding(.bottom, 6)
                .transition(.move(edge: .bottom).combined(with: .opacity))
        }
    }

    private func content(for item: SpokenItem) -> some View {
        HStack(spacing: 12) {
            AgentAvatar(identity: AgentIdentity(item), size: 26)
            VStack(alignment: .leading, spacing: 1) {
                Text(item.subject).font(.subheadline.weight(.medium)).lineLimit(1)
                Text(playback.statusText).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                    .accessibilityIdentifier("tts29.playback.status")
            }
            Spacer(minLength: 8)
            Button {
                playback.toggle(item)
            } label: {
                Image(systemName: playback.isPlaying ? "pause.fill" : "play.fill")
                    .font(.body)
                    .frame(width: 40, height: 40)
                    .contentTransition(.symbolEffect(.replace))
            }
            .buttonStyle(.plain)
            .foregroundStyle(Color.accentColor)
            .accessibilityLabel(playback.isPlaying ? "Pause" : "Play")
            .accessibilityIdentifier("tts29.miniplayer.toggle")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .glassCapsule(interactive: true)
        .overlay(alignment: .bottom) { progressHairline }
        .contentShape(Capsule())
        .onTapGesture { onOpen(item) }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("tts29.miniplayer")
    }

    private var progressHairline: some View {
        GeometryReader { geometry in
            Capsule()
                .fill(Color.accentColor)
                .frame(width: geometry.size.width * playback.progress, height: 2)
                .animation(.linear(duration: 0.25), value: playback.progress)
        }
        .frame(height: 2)
        .padding(.horizontal, 18)
        .allowsHitTesting(false)
    }
}
