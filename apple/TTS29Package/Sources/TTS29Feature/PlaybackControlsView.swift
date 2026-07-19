import SwiftUI

/// A compact transport: a scrubber with elapsed / remaining time beside a single
/// play/pause button. Skip and replay are intentionally absent — the scrubber
/// and tap-to-seek transcript cover them — and speed lives in the toolbar.
struct TransportCluster: View {
    let playback: PlaybackController
    @State private var scrubbing: Double?

    private var fraction: Double { scrubbing ?? playback.progress }
    private var remaining: TimeInterval {
        max(playback.duration - fraction * playback.duration, 0)
    }

    var body: some View {
        HStack(spacing: 14) {
            playPause
            VStack(spacing: 3) {
                Slider(
                    value: Binding(get: { fraction }, set: { scrubbing = $0 }),
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
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .glassSurface(in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .padding(.horizontal, 16)
        .padding(.bottom, 8)
    }

    private var playPause: some View {
        Button {
            if let item = playback.selectedItem { playback.toggle(item) }
        } label: {
            Image(systemName: playback.isPlaying ? "pause.fill" : "play.fill")
                .font(.title3)
                .frame(width: 44, height: 44)
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

/// The cycling speed control, sized for a toolbar slot: tap advances the rate,
/// long-press picks any rate.
struct SpeedControl: View {
    let playback: PlaybackController

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
                .font(.footnote.monospacedDigit().weight(.semibold))
                .contentTransition(.numericText())
                .frame(minWidth: 38)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .sensoryFeedback(.selection, trigger: playback.rate)
        .accessibilityLabel("Playback speed \(playback.rate.rateLabel)")
        .accessibilityIdentifier("tts29.playback.speed")
    }
}
