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
    public private(set) var selectedItemID: String?
    public private(set) var phase: PlaybackPhase = .idle
    public private(set) var currentTime: TimeInterval = 0
    public private(set) var duration: TimeInterval = 0
    public private(set) var failureMessage: String?

    private let backend: AudioPlaybackBackend
    private var selectedAudioURL: String?
    private var wantsPlayback = false

    public init() {
        backend = AVPlayerBackend()
        connectBackend()
    }

    init(backend: AudioPlaybackBackend) {
        self.backend = backend
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
        guard let source = item.audioURL, let url = admittedURL(source) else {
            selectedItemID = item.id
            selectedAudioURL = item.audioURL
            fail("Audio URL is unavailable.")
            return
        }
        if selectedItemID != item.id || selectedAudioURL != source || phase == .failed {
            backend.stop()
            selectedItemID = item.id
            selectedAudioURL = source
            currentTime = 0
            duration = 0
            failureMessage = nil
            wantsPlayback = true
            phase = .loading
            backend.load(url)
            backend.play()
            return
        }
        switch phase {
        case .playing:
            wantsPlayback = false
            backend.pause()
            phase = .paused
        case .loading:
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
    }

    public func synchronize(with items: [SpokenItem]) {
        guard let selectedItemID else { return }
        guard let current = items.first(where: { $0.id == selectedItemID }),
              current.audioURL == selectedAudioURL else {
            reset()
            return
        }
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
            if wantsPlayback {
                phase = .playing
            }
        case let .progress(current, duration):
            currentTime = validTime(current)
            self.duration = validTime(duration)
        case .completed:
            currentTime = duration
            wantsPlayback = false
            phase = .completed
        case .interrupted:
            wantsPlayback = false
            phase = .paused
        case let .failed(message):
            fail(message)
        }
    }

    private func reset() {
        backend.stop()
        selectedItemID = nil
        selectedAudioURL = nil
        phase = .idle
        currentTime = 0
        duration = 0
        failureMessage = nil
        wantsPlayback = false
    }

    private func fail(_ message: String) {
        backend.stop()
        failureMessage = message
        wantsPlayback = false
        phase = .failed
    }
}

private func admittedURL(_ value: String) -> URL? {
    guard let url = URL(string: value) else { return nil }
    if url.scheme == "https" { return url }
#if DEBUG
    if url.isFileURL { return url }
#endif
    return nil
}

private func validTime(_ value: TimeInterval) -> TimeInterval {
    value.isFinite && value >= 0 ? value : 0
}
