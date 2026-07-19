import SwiftUI

/// The fixed transport at the base of the item surface: a scrubber with elapsed
/// and remaining time above a single row of glass controls. The play/pause
/// button is the one tinted element; everything else is plain interactive glass.
struct TransportCluster: View {
    @Bindable var playback: PlaybackController
    @State private var scrubbing: Double?

    private var fraction: Double { scrubbing ?? playback.progress }
    private var remaining: TimeInterval {
        max(playback.duration - fraction * playback.duration, 0)
    }

    var body: some View {
        GlassContainer(spacing: 18) {
            VStack(spacing: 12) {
                scrubber
                controls
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 14)
            .glassSurface(in: RoundedRectangle(cornerRadius: 26, style: .continuous))
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 8)
    }

    private var scrubber: some View {
        VStack(spacing: 4) {
            Slider(
                value: Binding(
                    get: { fraction },
                    set: { scrubbing = $0 }
                ),
                in: 0...1
            ) { editing in
                if !editing, let value = scrubbing {
                    playback.seek(toFraction: value)
                    scrubbing = nil
                }
            }
            .disabled(playback.duration <= 0)
            .accessibilityIdentifier("tts29.playback.scrubber")

            HStack {
                Text(Formatting.clock(fraction * playback.duration))
                Spacer()
                Text("-" + Formatting.clock(remaining))
            }
            .font(.caption2.monospacedDigit())
            .foregroundStyle(.secondary)
        }
    }

    private var controls: some View {
        HStack {
            TransportButton(system: "gobackward.15", label: "Back 15 seconds") {
                playback.skipBackward()
            }
            Spacer()
            TransportButton(system: "backward.end.fill", label: "Replay from start") {
                playback.replay()
            }
            Spacer()
            playPause
            Spacer()
            TransportButton(system: "goforward.15", label: "Forward 15 seconds") {
                playback.skipForward()
            }
            Spacer()
            SpeedCapsule(playback: playback)
        }
    }

    private var playPause: some View {
        Button {
            if let item = playback.selectedItem { playback.toggle(item) }
        } label: {
            Image(systemName: playback.isPlaying ? "pause.fill" : "play.fill")
                .font(.title)
                .frame(width: 56, height: 56)
                .contentTransition(.symbolEffect(.replace))
        }
        .buttonStyle(.plain)
        .foregroundStyle(Color.accentColor)
        .glassSurface(in: Circle(), tint: .accentColor, interactive: true)
        .sensoryFeedback(.impact(weight: .light), trigger: playback.isPlaying)
        .accessibilityLabel(playback.isPlaying ? "Pause" : "Play")
        .accessibilityIdentifier("tts29.playback.toggle")
    }
}

private struct TransportButton: View {
    let system: String
    let label: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: system)
                .font(.title3)
                .frame(width: 44, height: 44)
        }
        .buttonStyle(.plain)
        .foregroundStyle(.primary)
        .glassSurface(in: Circle(), interactive: true)
        .accessibilityLabel(label)
    }
}

/// The cycling speed control: tap advances the rate, long-press picks any rate.
private struct SpeedCapsule: View {
    @Bindable var playback: PlaybackController

    var body: some View {
        Menu {
            Picker("Speed", selection: Binding(
                get: { playback.rate },
                set: { playback.setRate($0) }
            )) {
                ForEach(PlaybackRateStore.menu, id: \.self) { rate in
                    Text(rate.rateLabel).tag(rate)
                }
            }
        } label: {
            Text(playback.rate.rateLabel)
                .font(.caption.monospacedDigit().weight(.bold))
                .contentTransition(.numericText())
                .frame(width: 44, height: 44)
                .glassSurface(in: Circle(), interactive: true)
        } primaryAction: {
            withAnimation(.snappy) { playback.cycleRate() }
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .foregroundStyle(.primary)
        .sensoryFeedback(.selection, trigger: playback.rate)
        .accessibilityLabel("Playback speed \(playback.rate.rateLabel)")
        .accessibilityIdentifier("tts29.playback.speed")
    }
}
