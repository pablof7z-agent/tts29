import SwiftUI
import TTS29Feature

@main
struct TTS29App: App {
    var body: some Scene {
        WindowGroup {
            ContentView(initialSnapshot: Self.injectedSnapshot)
        }
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
