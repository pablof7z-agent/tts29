import SwiftUI

public struct ContentView: View {
    @State private var store = TTS29Store()

    public init() {}

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
                        SpokenItemRow(item: item)
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
    }
}

private struct SpokenItemRow: View {
    let item: SpokenItem

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(item.subject)
                .font(.headline)
            Text(item.summary)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .lineLimit(3)
        }
        .padding(.vertical, 4)
        .accessibilityIdentifier("tts29.item.\(item.id)")
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
