import Foundation
import Testing
@testable import TTS29Feature

@Test func snapshotDecodesRichItemFields() throws {
    let json = """
    {
      "phase": "listening",
      "relay": "wss://relay.example",
      "group_id": "tts",
      "items": [{
        "id": "item1",
        "author": "abc123def456ghi789",
        "created_at": 1000,
        "agent_name": "indigo-claude",
        "subject": "Release Ready",
        "summary": "The build is signed.",
        "body": "# Done\\n\\nThe build shipped.",
        "audio_url": "https://cdn.example/a.mp3",
        "audio": {"url": "https://cdn.example/a.mp3", "sha256": "ab", "media_type": "audio/mpeg", "byte_count": 1234},
        "attachments": [{"url": "https://cdn.example/p.md", "sha256": "cd", "media_type": "text/markdown", "byte_count": 42, "label": "Proposal"}],
        "questions": [{"id": "q1", "kind": "single_choice", "short_title": "Ship?", "title": "Ship it now?", "description": null, "options": [{"id": "yes", "title": "Yes", "description": null}]}],
        "answer": {"event_id": "e1", "author": "abc123", "created_at": 1500, "answers": [{"question_id": "q1", "values": ["yes"]}]},
        "acknowledgement": {"event_id": "a1", "author": "abc123", "created_at": 1600, "state": "heard", "reason": null},
        "reactions": [{"emoji": "🎉", "count": 2, "authors": ["x", "y"]}]
      }],
      "evidence": {"source_count": 1, "shortfall_count": 0, "rejected_event_count": 0},
      "error": null
    }
    """
    let snapshot = try JSONDecoder().decode(QueueSnapshot.self, from: Data(json.utf8))
    let item = try #require(snapshot.items.first)
    #expect(item.agentName == "indigo-claude")
    #expect(item.attachments.first?.label == "Proposal")
    #expect(item.questions.first?.kind == .singleChoice)
    #expect(item.answer?.values(for: "q1") == ["yes"])
    #expect(item.isHeard)
    #expect(item.reactions.first?.emoji == "🎉")
    #expect(item.playableURL == "https://cdn.example/a.mp3")
}

@Test func timestampFormatsRelativeThenAbsolute() {
    let now = Date(timeIntervalSince1970: 1_000_000)
    #expect(Formatting.timestamp(now.addingTimeInterval(-30), now: now) == "just now")
    #expect(Formatting.timestamp(now.addingTimeInterval(-600), now: now) == "10m")
    #expect(Formatting.timestamp(now.addingTimeInterval(-7_200), now: now) == "2h")
    #expect(!Formatting.timestamp(now.addingTimeInterval(-200_000), now: now).contains("h"))
}

@Test func agentIdentityGradientIsDeterministic() {
    let a = AgentIdentity(agentName: "indigo-claude", author: "pubkeyhex1234")
    let b = AgentIdentity(agentName: "indigo-claude", author: "pubkeyhex1234")
    let c = AgentIdentity(agentName: "amber-codex", author: "otherpubkey99")
    #expect(a.gradientColors == b.gradientColors)
    #expect(a.gradientColors != c.gradientColors)
    #expect(a.initials == "IC")
    #expect(a.displayName == "indigo-claude")
}

@Test func transcriptFocusFollowsProgress() {
    let document = TranscriptDocument("# Title\n\nFirst paragraph here.\n\n- a bullet point\n\nClosing thoughts.")
    #expect(document.blocks.count == 4)
    let first = document.focusedIndex(at: 0)
    let last = document.focusedIndex(at: 1)
    #expect(first == 0)
    #expect(last == document.blocks.count - 1)
    // Progress moves the focus forward monotonically.
    let mid = document.focusedIndex(at: 0.5) ?? 0
    #expect(mid >= first! && mid <= last!)
}

@MainActor
@Test func rateStoreRemembersPerAgent() {
    let suite = "tts29.rate.\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suite)!
    defer { defaults.removePersistentDomain(forName: suite) }
    let store = PlaybackRateStore(defaults: defaults)
    #expect(store.rate(for: "indigo") == 1.0)
    store.setRate(1.5, for: "indigo")
    #expect(store.rate(for: "indigo") == 1.5)
    #expect(store.rate(for: "amber") == 1.0)
    #expect(store.nextRate(after: 1.0) == 1.2)
    #expect(store.nextRate(after: 0.9) == 1.0)
}

@MainActor
@Test func transportSeeksSkipsAndCyclesRate() {
    let backend = TransportFake()
    let store = PlaybackRateStore(defaults: UserDefaults(suiteName: "t.\(UUID())")!)
    let playback = PlaybackController(backend: backend, rateStore: store)
    let item = SpokenItem(
        id: "i", author: "a", createdAt: 1, subject: "S", summary: "", body: "b",
        audioURL: "https://cdn.example/a.mp3", agentName: "indigo"
    )
    playback.toggle(item)
    backend.emit(.ready(duration: 100))
    backend.emit(.progress(current: 50, duration: 100))

    playback.skipBackward()
    #expect(backend.lastSeek == 35)
    playback.skipForward()
    #expect(backend.lastSeek == 50)
    playback.seek(toFraction: 0.25)
    #expect(backend.lastSeek == 25)

    #expect(playback.rate == 1.0)
    playback.cycleRate()
    #expect(playback.rate == 1.2)
    #expect(backend.lastRate == 1.2)
    #expect(store.rate(for: "indigo") == 1.2)
}

@MainActor
@Test func completionAutoplaysNextPlayableItem() {
    let backend = TransportFake()
    let playback = PlaybackController(backend: backend)
    let first = SpokenItem(id: "1", author: "a", createdAt: 2, subject: "One", summary: "", body: "b", audioURL: "https://cdn.example/1.mp3")
    let second = SpokenItem(id: "2", author: "a", createdAt: 1, subject: "Two", summary: "", body: "b", audioURL: "https://cdn.example/2.mp3")
    playback.synchronize(with: [first, second])
    playback.toggle(first)
    backend.emit(.ready(duration: 10))
    backend.emit(.completed)
    #expect(playback.selectedItemID == "2")
    #expect(playback.phase == .loading || playback.phase == .playing)
}

@MainActor
private final class TransportFake: AudioPlaybackBackend {
    var onEvent: ((AudioPlaybackEvent) -> Void)?
    var lastSeek: TimeInterval?
    var lastRate: Float?

    func load(_ url: URL) {}
    func play() {}
    func pause() {}
    func stop() {}
    func seek(to time: TimeInterval) { lastSeek = time }
    func setRate(_ rate: Float) { lastRate = rate }
    func emit(_ event: AudioPlaybackEvent) { onEvent?(event) }
}
