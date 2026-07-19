import SwiftUI

/// The queue: quiet rows, or a purposeful full-screen state for loading,
/// emptiness, filtering, and failure. Failure earns a screen with a way out,
/// never a red footnote.
struct QueueListView: View {
    let items: [SpokenItem]
    let snapshot: QueueSnapshot
    @Bindable var playback: PlaybackController
    let isFiltering: Bool
    let namespace: Namespace.ID
    let onOpen: (SpokenItem) -> Void
    let onPlay: (SpokenItem) -> Void
    let onEditConnection: () -> Void

    var body: some View {
        switch snapshot.phase {
        case .failed:
            failure
        case .starting where snapshot.items.isEmpty:
            skeleton
        default:
            if items.isEmpty {
                emptyState
            } else {
                list
            }
        }
    }

    private var list: some View {
        List(items) { item in
            SpokenItemRow(
                item: item,
                playback: playback,
                onOpen: { onOpen(item) },
                onPlay: { onPlay(item) }
            )
            .zoomSource(item.id, in: namespace)
        }
        .listStyle(.plain)
        .accessibilityIdentifier("tts29.queue")
    }

    private var skeleton: some View {
        List(0..<5, id: \.self) { _ in
            HStack(alignment: .top, spacing: 12) {
                Circle().frame(width: 34, height: 34)
                VStack(alignment: .leading, spacing: 6) {
                    Text("Spoken update title").font(.headline)
                    Text("A one line summary of what changed").font(.subheadline)
                    Text("agent · 2m").font(.caption)
                }
                Spacer()
            }
            .padding(.vertical, 6)
            .redacted(reason: .placeholder)
        }
        .listStyle(.plain)
        .allowsHitTesting(false)
        .accessibilityLabel("Loading updates")
    }

    @ViewBuilder
    private var emptyState: some View {
        if isFiltering {
            ContentUnavailableView.search
        } else {
            ContentUnavailableView(
                "No spoken updates",
                systemImage: "waveform",
                description: Text("New items from the group will appear here.")
            )
        }
    }

    private var failure: some View {
        ContentUnavailableView {
            Label("Can't reach the group", systemImage: "antenna.radiowaves.left.and.right.slash")
        } description: {
            Text(snapshot.error ?? "The queue could not start.")
        } actions: {
            Button("Connection Settings…", action: onEditConnection)
                .buttonStyle(.borderedProminent)
        }
        .accessibilityIdentifier("tts29.queue.failure")
    }
}
