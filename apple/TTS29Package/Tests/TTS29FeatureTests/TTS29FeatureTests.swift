import Foundation
import Testing
@testable import TTS29Feature

@Test func initialSnapshotCommunicatesStartup() {
    let snapshot = QueueSnapshot.initial

    #expect(snapshot.phase == .starting)
    #expect(snapshot.items.isEmpty)
    #expect(snapshot.statusMessage == "Starting NMP queue…")
}

@Test func accountAndAnswerReceiptDecodeWithoutExposingASecret() throws {
    let json = """
    {
      "phase":"listening",
      "relay":"wss://relay.example.com",
      "group_id":"tts",
      "items":[],
      "evidence":{"source_count":1,"shortfall_count":0},
      "error":null,
      "identity":{"phase":"signed_in","pubkey":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","error":null},
      "credential_request":null,
      "answer_submissions":[{"item_id":"item","phase":"published","receipt_id":7,"event_id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","error":null}]
    }
    """

    let snapshot = try JSONDecoder().decode(QueueSnapshot.self, from: Data(json.utf8))

    #expect(snapshot.identity.phase == .signedIn)
    #expect(snapshot.identity.shortPubkey == "aaaaaaaa…aaaa")
    #expect(snapshot.answerSubmission(for: "item")?.phase == .published)
    #expect(!json.contains("nsec"))
}

@Test func savedConnectionOverridesBundledBootstrapIndependently() {
    let suite = "tts29.connection.\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suite)!
    defer { defaults.removePersistentDomain(forName: suite) }
    let fallback = ConnectionSettings(
        relay: "wss://default.example",
        groupID: "default"
    )
    defaults.set("wss://user.example", forKey: ConnectionSettings.relayKey)

    let resolved = ConnectionSettings.resolve(defaults: defaults, fallback: fallback)

    #expect(resolved.relay == "wss://user.example")
    #expect(resolved.groupID == "default")
}

@MainActor
@Test func playbackIsLocalAndStopsWhenTheProjectedItemDisappears() {
    let backend = FakePlaybackBackend()
    let playback = PlaybackController(backend: backend)
    let item = fixtureItem()

    playback.toggle(item)
    #expect(playback.phase == .loading)
    #expect(backend.loadedURL == URL(string: item.audioURL!))
    #expect(backend.playCount == 1)

    backend.emit(.ready(duration: 10))
    #expect(playback.phase == .playing)
    #expect(backend.playCount == 1)

    backend.emit(.progress(current: 4, duration: 10))
    #expect(playback.progress == 0.4)
    playback.toggle(item)
    #expect(playback.phase == .paused)
    #expect(backend.pauseCount == 1)

    playback.synchronize(with: [])
    #expect(playback.phase == .idle)
    #expect(playback.selectedItemID == nil)
    #expect(backend.stopCount == 2)
}

@MainActor
@Test func invalidAudioFailsWithoutLoadingTheBackend() {
    let backend = FakePlaybackBackend()
    let playback = PlaybackController(backend: backend)
    let item = SpokenItem(
        id: "invalid",
        author: "author",
        createdAt: 1,
        subject: "Invalid",
        summary: "Invalid audio",
        body: "Invalid audio",
        audioURL: "http://insecure.example/audio.mp3"
    )

    playback.toggle(item)

    #expect(playback.phase == .failed)
    #expect(playback.selectedItemID == item.id)
    #expect(playback.statusText == "Audio URL is unavailable.")
    #expect(backend.loadedURL == nil)
}

@MainActor
@Test func playbackStopsWhenTheProjectedAudioChanges() {
    let backend = FakePlaybackBackend()
    let playback = PlaybackController(backend: backend)
    let item = fixtureItem()

    playback.toggle(item)
    playback.synchronize(with: [SpokenItem(
        id: item.id,
        author: item.author,
        createdAt: item.createdAt,
        subject: item.subject,
        summary: item.summary,
        body: item.body,
        audioURL: "https://cdn.example/replaced.mp3"
    )])

    #expect(playback.phase == .idle)
    #expect(playback.selectedItemID == nil)
    #expect(backend.stopCount == 2)
}

private func fixtureItem() -> SpokenItem {
    SpokenItem(
        id: "item",
        author: "author",
        createdAt: 1,
        subject: "Update",
        summary: "A spoken update",
        body: "A spoken update",
        audioURL: "https://cdn.example/audio.mp3"
    )
}

@MainActor
private final class FakePlaybackBackend: AudioPlaybackBackend {
    var onEvent: ((AudioPlaybackEvent) -> Void)?
    var loadedURL: URL?
    var playCount = 0
    var pauseCount = 0
    var stopCount = 0

    func load(_ url: URL) {
        loadedURL = url
    }

    func play() {
        playCount += 1
    }

    func pause() {
        pauseCount += 1
    }

    func stop() {
        stopCount += 1
    }

    func emit(_ event: AudioPlaybackEvent) {
        onEvent?(event)
    }
}
