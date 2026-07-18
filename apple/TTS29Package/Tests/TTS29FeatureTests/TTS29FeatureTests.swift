import Foundation
import Testing
@testable import TTS29Feature

@Test func initialSnapshotCommunicatesStartup() {
    let snapshot = QueueSnapshot.initial

    #expect(snapshot.phase == .starting)
    #expect(snapshot.items.isEmpty)
    #expect(snapshot.statusMessage == "Starting NMP queue…")
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
