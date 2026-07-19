import Foundation

public extension PlaybackController {
    /// The seconds moved by the skip-back / skip-forward controls.
    static let skipInterval: TimeInterval = 15

    var isPlaying: Bool { phase == .playing || phase == .loading }

    func isActive(_ item: SpokenItem) -> Bool { selectedItemID == item.id }

    /// Seeks to an absolute time, clamped to the loaded duration.
    func seek(to time: TimeInterval) {
        guard duration > 0 else { return }
        let target = min(max(time, 0), duration)
        currentTime = target
        backend.seek(to: target)
        if phase == .completed { phase = wantsPlayback ? .playing : .paused }
        notifyChange()
    }

    /// Seeks to a 0…1 position on the timeline.
    func seek(toFraction fraction: Double) {
        seek(to: fraction * duration)
    }

    func skip(by seconds: TimeInterval) {
        seek(to: currentTime + seconds)
    }

    func skipBackward() { skip(by: -Self.skipInterval) }
    func skipForward() { skip(by: Self.skipInterval) }

    /// Restarts the current item from the top, keeping it playing.
    func replay() {
        guard selectedItem != nil else { return }
        wantsPlayback = true
        seek(to: 0)
        backend.play()
        phase = .playing
        notifyChange()
    }

    /// Advances the transport capsule to the next listening speed and remembers
    /// it for the current agent.
    func cycleRate() {
        setRate(rateStore.nextRate(after: rate))
    }

    func setRate(_ newRate: Float) {
        rate = newRate
        backend.setRate(newRate)
        if let agent = selectedItem?.agentName {
            rateStore.setRate(newRate, for: agent)
        }
        notifyChange()
    }

    /// When an item finishes, continue down the queue to the next item that has
    /// playable audio — the podcast-queue behaviour, and a device-local policy.
    func autoplayNextIfPossible() {
        guard autoplayEnabled, let current = selectedItemID else { return }
        guard let index = orderedItems.firstIndex(where: { $0.id == current }) else { return }
        let next = orderedItems[(index + 1)...].first { $0.playableURL != nil }
        guard let next else { return }
        toggle(next)
    }

    /// How many queued items would autoplay after the given item — the number
    /// shown on the item surface's back control.
    func upNextCount(after item: SpokenItem) -> Int {
        guard let index = orderedItems.firstIndex(where: { $0.id == item.id }) else { return 0 }
        return orderedItems[(index + 1)...].filter { $0.playableURL != nil }.count
    }

    /// The block of the transcript most likely being spoken right now.
    func focusedBlockID(in document: TranscriptDocument) -> Int? {
        guard let index = document.focusedIndex(at: progress) else { return nil }
        return document.blocks[index].id
    }
}
