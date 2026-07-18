import Testing
@testable import TTS29Feature

@Test func initialSnapshotCommunicatesStartup() {
    let snapshot = QueueSnapshot.initial

    #expect(snapshot.phase == .starting)
    #expect(snapshot.items.isEmpty)
    #expect(snapshot.statusMessage == "Starting NMP queue…")
}
