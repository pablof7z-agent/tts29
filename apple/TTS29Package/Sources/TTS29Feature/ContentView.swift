import SwiftUI

public struct ContentView: View {
    @State private var store: TTS29Store
    @State private var playback = PlaybackController()
    @State private var nowPlaying = NowPlayingCenter()
    @State private var path = NavigationPath()
    @State private var search = ""
    @State private var agentFilter: Set<String> = []
    @State private var showsConnectionSettings = false
    @Namespace private var zoom
    private let autoPlayItemID: String?

    public init(initialSnapshot: QueueSnapshot? = nil, autoPlayItemID: String? = nil) {
        _store = State(initialValue: TTS29Store(initialSnapshot: initialSnapshot))
        self.autoPlayItemID = autoPlayItemID
    }

    public var body: some View {
        NavigationStack(path: $path) {
            QueueListView(
                items: filteredItems,
                snapshot: store.snapshot,
                playback: playback,
                isFiltering: isFiltering,
                namespace: zoom,
                onOpen: { path.append($0) },
                onPlay: { playback.toggle($0) },
                onEditConnection: { showsConnectionSettings = true }
            )
            .navigationTitle("TTS29")
            .toolbar { toolbar }
            .navigationDestination(for: SpokenItem.self) { item in
                NowPlayingView(item: item, playback: playback)
                    .zoomDestination(item.id, in: zoom)
            }
            .safeAreaInset(edge: .bottom) {
                MiniPlayerView(playback: playback) { path.append($0) }
                    .animation(.snappy, value: playback.selectedItemID)
            }
        }
        .searchable(text: $search, prompt: "Search updates")
        .task {
            nowPlaying.attach(to: playback)
            await store.run()
        }
        .onChange(of: store.snapshot.items, initial: true) { _, items in
            playback.synchronize(with: items)
            autoPlayIfNeeded(items)
        }
        .sheet(isPresented: $showsConnectionSettings) {
            ConnectionSettingsView()
        }
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItem(placement: .primaryAction) {
            Menu {
                if !availableAgents.isEmpty {
                    Section("Filter by agent") {
                        ForEach(availableAgents, id: \.self) { agent in
                            Toggle(agent, isOn: agentBinding(agent))
                        }
                    }
                }
                Toggle("Autoplay next", isOn: $playback.autoplayEnabled)
                Divider()
                Button("Connection…", systemImage: "antenna.radiowaves.left.and.right") {
                    showsConnectionSettings = true
                }
            } label: {
                Label("Filter", systemImage: isFiltering
                    ? "line.3.horizontal.decrease.circle.fill"
                    : "line.3.horizontal.decrease.circle")
            }
            .accessibilityIdentifier("tts29.menu")
        }
    }

    private var availableAgents: [String] {
        var seen = Set<String>()
        return store.snapshot.items.compactMap { item in
            let name = AgentIdentity(item).displayName
            return seen.insert(name).inserted ? name : nil
        }
    }

    private var isFiltering: Bool { !search.isEmpty || !agentFilter.isEmpty }

    private var filteredItems: [SpokenItem] {
        store.snapshot.items.filter { item in
            let name = AgentIdentity(item).displayName
            if !agentFilter.isEmpty, !agentFilter.contains(name) { return false }
            guard !search.isEmpty else { return true }
            let haystack = [item.subject, item.summary, item.body, name]
                .joined(separator: " ").lowercased()
            return haystack.contains(search.lowercased())
        }
    }

    private func agentBinding(_ agent: String) -> Binding<Bool> {
        Binding(
            get: { agentFilter.contains(agent) },
            set: { isOn in
                if isOn { agentFilter.insert(agent) } else { agentFilter.remove(agent) }
            }
        )
    }

    private func autoPlayIfNeeded(_ items: [SpokenItem]) {
        guard playback.selectedItemID == nil,
              let autoPlayItemID,
              let item = items.first(where: { $0.id == autoPlayItemID }) else { return }
        playback.toggle(item)
    }
}
