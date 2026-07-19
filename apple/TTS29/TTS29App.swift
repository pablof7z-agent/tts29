import SwiftUI
import TTS29Feature

@main
struct TTS29App: App {
    var body: some Scene {
        WindowGroup {
            ContentView(
                initialSnapshot: Self.injectedSnapshot,
                autoPlayItemID: Self.autoPlayItemID,
                openItemID: Self.openItemID
            )
        }
    }

    private static var autoPlayItemID: String? {
#if DEBUG
        ProcessInfo.processInfo.environment["TTS29_UI_AUTOPLAY_ID"]
#else
        nil
#endif
    }

    /// DEBUG-only: push straight into a real item's surface on launch so it can
    /// be captured without a fragile programmatic tap. Matches items from the
    /// live relay projection by event id.
    private static var openItemID: String? {
#if DEBUG
        ProcessInfo.processInfo.environment["TTS29_UI_OPEN_ID"]
#else
        nil
#endif
    }

    private static var injectedSnapshot: QueueSnapshot? {
#if DEBUG
        guard let encoded = ProcessInfo.processInfo.environment["TTS29_UI_AUDIO_BASE64"],
              let audio = Data(base64Encoded: encoded) else {
            return nil
        }
        let fixtureURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("tts29-ui-fixture.wav")
        try? audio.write(to: fixtureURL, options: .atomic)
        let item = SpokenItem(
            id: "ui-fixture",
            author: String(repeating: "a", count: 64),
            createdAt: 1,
            subject: "Simulator playback",
            summary: "A local deterministic audio fixture.",
            body: "Simulator playback fixture.",
            audioURL: fixtureURL.absoluteString
        )
        return QueueSnapshot(
            phase: .listening,
            relay: "fixture",
            groupID: "tts",
            items: [item],
            evidence: QueueEvidence(sourceCount: 1, shortfallCount: 0),
            error: nil
        )
#else
        return nil
#endif
    }
}
