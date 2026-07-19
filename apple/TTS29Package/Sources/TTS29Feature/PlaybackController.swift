import Foundation
import Observation

enum AudioPlaybackEvent: Equatable {
    case ready(duration: TimeInterval)
    case progress(current: TimeInterval, duration: TimeInterval)
    case completed
    case interrupted
    case failed(String)
}

@MainActor
protocol AudioPlaybackBackend: AnyObject {
    var onEvent: ((AudioPlaybackEvent) -> Void)? { get set }
    func load(_ url: URL)
    func play()
    func pause()
    func stop()
    func seek(to time: TimeInterval)
    func setRate(_ rate: Float)
}

extension AudioPlaybackBackend {
    func seek(to time: TimeInterval) {}
    func setRate(_ rate: Float) {}
}

public enum PlaybackPhase: Equatable, Sendable {
    case idle
    case loading
    case playing
    case paused
    case completed
    case failed
}

@Observable
@MainActor
public final class PlaybackController {
    public private(set) var selectedItem: SpokenItem?
    public internal(set) var phase: PlaybackPhase = .idle
    public internal(set) var currentTime: TimeInterval = 0
    public internal(set) var duration: TimeInterval = 0
    public private(set) var failureMessage: String?
    public internal(set) var rate: Float = 1.0
    public var autoplayEnabled = true

    /// Invoked whenever now-playing state materially changes, so a platform
    /// media surface (Control Center, lock screen) can mirror it.
    public var onPlaybackChange: ((PlaybackController) -> Void)?

    let backend: AudioPlaybackBackend
    let rateStore: PlaybackRateStore
    private(set) var orderedItems: [SpokenItem] = []
    var selectedAudioURL: String?
    var wantsPlayback = false

    public var selectedItemID: String? { selectedItem?.id }

    public init() {
        backend = AVPlayerBackend()
        rateStore = PlaybackRateStore()
        connectBackend()
    }

    init(backend: AudioPlaybackBackend, rateStore: PlaybackRateStore = PlaybackRateStore()) {
        self.backend = backend
        self.rateStore = rateStore
        connectBackend()
    }

    public var progress: Double {
        guard duration.isFinite, duration > 0 else { return 0 }
        return min(max(currentTime / duration, 0), 1)
    }

    public var statusText: String {
        switch phase {
        case .idle: "Ready"
        case .loading: "Loading audio…"
        case .playing: "Playing"
        case .paused: "Paused"
        case .completed: "Finished"
        case .failed: failureMessage ?? "Audio unavailable"
        }
    }

    public func toggle(_ item: SpokenItem) {
        guard let source = item.playableURL, let url = admittedURL(source) else {
            selectedItem = item
            selectedAudioURL = item.playableURL
            fail("Audio URL is unavailable.")
            return
        }
        if selectedItemID != item.id || selectedAudioURL != source || phase == .failed {
            backend.stop()
            selectedItem = item
            selectedAudioURL = source
            currentTime = 0
            duration = 0
            failureMessage = nil
            wantsPlayback = true
            phase = .loading
            rate = rateStore.rate(for: item.agentName)
            backend.setRate(rate)
            backend.load(url)
            backend.play()
            notifyChange()
            return
        }
        switch phase {
        case .playing, .loading:
            wantsPlayback = false
            backend.pause()
            phase = .paused
        case .paused, .completed, .idle:
            wantsPlayback = true
            backend.play()
            phase = .playing
        case .failed:
            break
        }
        notifyChange()
    }

    public func synchronize(with items: [SpokenItem]) {
        orderedItems = items
        guard let selectedItemID else { return }
        // Narrated children are nested, so search the whole tree, not just the
        // top-level queue, before deciding a selected item has disappeared.
        guard let current = Self.flatten(items).first(where: { $0.id == selectedItemID }),
              current.playableURL == selectedAudioURL else {
            reset()
            return
        }
        // Keep the retained item current so metadata (agent, subject) stays fresh.
        selectedItem = current
    }

    static func flatten(_ items: [SpokenItem]) -> [SpokenItem] {
        items.flatMap { [$0] + flatten($0.children) }
    }

    public func symbol(for item: SpokenItem) -> String {
        guard selectedItemID == item.id else { return "play.circle.fill" }
        return switch phase {
        case .playing, .loading: "pause.circle.fill"
        case .failed: "exclamationmark.circle.fill"
        default: "play.circle.fill"
        }
    }

    public func label(for item: SpokenItem) -> String {
        guard selectedItemID == item.id else { return "Play \(item.subject)" }
        return switch phase {
        case .playing, .loading: "Pause \(item.subject)"
        case .failed: "Retry \(item.subject)"
        default: "Play \(item.subject)"
        }
    }

    private func connectBackend() {
        backend.onEvent = { [weak self] event in
            self?.receive(event)
        }
    }

    private func receive(_ event: AudioPlaybackEvent) {
        switch event {
        case let .ready(duration):
            self.duration = validTime(duration)
            backend.setRate(rate)
            if wantsPlayback {
                phase = .playing
            }
            notifyChange()
        case let .progress(current, duration):
            currentTime = validTime(current)
            self.duration = validTime(duration)
            notifyChange()
        case .completed:
            currentTime = duration
            wantsPlayback = false
            phase = .completed
            notifyChange()
            autoplayNextIfPossible()
        case .interrupted:
            wantsPlayback = false
            phase = .paused
            notifyChange()
        case let .failed(message):
            fail(message)
        }
    }

    func reset() {
        backend.stop()
        selectedItem = nil
        selectedAudioURL = nil
        phase = .idle
        currentTime = 0
        duration = 0
        failureMessage = nil
        wantsPlayback = false
        notifyChange()
    }

    func fail(_ message: String) {
        backend.stop()
        failureMessage = message
        wantsPlayback = false
        phase = .failed
        notifyChange()
    }

    func notifyChange() {
        onPlaybackChange?(self)
    }
}

func admittedURL(_ value: String) -> URL? {
    guard let url = URL(string: value) else { return nil }
    if url.scheme == "https" { return url }
#if DEBUG
    if url.isFileURL { return url }
#endif
    return nil
}

func validTime(_ value: TimeInterval) -> TimeInterval {
    value.isFinite && value >= 0 ? value : 0
}
