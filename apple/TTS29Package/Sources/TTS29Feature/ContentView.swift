import SwiftUI

public struct ContentView: View {
    @State private var store: TTS29Store
    @State private var playback = PlaybackController()

    public init(initialSnapshot: QueueSnapshot? = nil) {
        _store = State(initialValue: TTS29Store(initialSnapshot: initialSnapshot))
    }

    public var body: some View {
        NavigationStack {
            Group {
                if store.snapshot.items.isEmpty {
                    ContentUnavailableView(
                        "No spoken updates",
                        systemImage: "waveform",
                        description: Text("New items from the group will appear here.")
                    )
                } else {
                    List(store.snapshot.items) { item in
                        SpokenItemRow(item: item, playback: playback)
                    }
                    .listStyle(.plain)
                }
            }
            .navigationTitle("TTS29")
            .safeAreaInset(edge: .bottom) {
                QueueStatus(snapshot: store.snapshot)
            }
        }
        .task {
            await store.run()
        }
        .onChange(of: store.snapshot.items.map(PlaybackSource.init), initial: true) {
            playback.synchronize(with: store.snapshot.items)
        }
    }
}

private struct PlaybackSource: Equatable {
    let id: String
    let audioURL: String?

    init(_ item: SpokenItem) {
        id = item.id
        audioURL = item.audioURL
    }
}

private struct SpokenItemRow: View {
    let item: SpokenItem
    let playback: PlaybackController

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text(item.subject)
                    .font(.headline)
                Text(item.summary)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .lineLimit(3)
                if playback.selectedItemID == item.id {
                    PlaybackProgress(playback: playback)
                }
            }
            Spacer(minLength: 8)
            Button {
                playback.toggle(item)
            } label: {
                Image(systemName: playback.symbol(for: item))
                    .font(.title2)
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
            .disabled(item.audioURL == nil)
            .accessibilityLabel(playback.label(for: item))
            .accessibilityIdentifier("tts29.play.\(item.id)")
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .contain)
    }
}

private struct PlaybackProgress: View {
    let playback: PlaybackController

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            ProgressView(value: playback.progress)
                .accessibilityIdentifier("tts29.playback.progress")
            Text(playback.statusText)
                .font(.caption)
                .foregroundStyle(playback.phase == .failed ? Color.red : Color.secondary)
                .accessibilityIdentifier("tts29.playback.status")
        }
    }
}

private struct QueueStatus: View {
    let snapshot: QueueSnapshot

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: snapshot.phase.symbolName)
            Text(snapshot.statusMessage)
                .font(.footnote)
            Spacer()
            Text("\(snapshot.items.count) queued")
                .font(.caption.monospacedDigit())
        }
        .foregroundStyle(snapshot.phase == .failed ? Color.red : Color.secondary)
        .padding(.horizontal)
        .padding(.vertical, 10)
        .background(.bar)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("tts29.status")
    }
}
