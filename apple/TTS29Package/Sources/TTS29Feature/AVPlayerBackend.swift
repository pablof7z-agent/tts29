@preconcurrency import AVFoundation
import Foundation

@MainActor
final class AVPlayerBackend: AudioPlaybackBackend {
    var onEvent: ((AudioPlaybackEvent) -> Void)?

    private let player = AVPlayer()
    private var currentItem: AVPlayerItem?
    private var timeObserver: Any?
    private var itemNotifications: [NSObjectProtocol] = []
    private var reportedReady = false
    private var reportedFailure = false
    private var desiredRate: Float = 1.0
#if os(iOS)
    private var interruptionNotification: NSObjectProtocol?
#endif

    init() {}

    func load(_ url: URL) {
        installPlaybackObservers()
        removeItemNotifications()
        reportedReady = false
        reportedFailure = false
        let item = AVPlayerItem(url: url)
        currentItem = item
        player.replaceCurrentItem(with: item)
        let center = NotificationCenter.default
        itemNotifications.append(center.addObserver(
            forName: .AVPlayerItemDidPlayToEndTime,
            object: item,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.onEvent?(.completed)
            }
        })
        itemNotifications.append(center.addObserver(
            forName: .AVPlayerItemFailedToPlayToEndTime,
            object: item,
            queue: .main
        ) { [weak self] note in
            let message = (note.userInfo?[AVPlayerItemFailedToPlayToEndTimeErrorKey] as? NSError)?
                .localizedDescription ?? "Audio playback failed."
            MainActor.assumeIsolated {
                self?.reportFailure(message)
            }
        })
    }

    private func installPlaybackObservers() {
        guard timeObserver == nil else { return }
        let interval = CMTime(seconds: 0.25, preferredTimescale: 600)
        timeObserver = player.addPeriodicTimeObserver(
            forInterval: interval,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.tick()
            }
        }
#if os(iOS)
        interruptionNotification = NotificationCenter.default.addObserver(
            forName: AVAudioSession.interruptionNotification,
            object: AVAudioSession.sharedInstance(),
            queue: .main
        ) { [weak self] note in
            let rawType = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
            MainActor.assumeIsolated {
                self?.handleInterruption(rawType)
            }
        }
#endif
    }

    func play() {
#if os(iOS)
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playback, mode: .spokenAudio)
            try session.setActive(true)
        } catch {
            reportFailure("The audio session is unavailable.")
            return
        }
#endif
        if player.currentItem?.currentTime() == player.currentItem?.duration {
            player.seek(to: .zero)
        }
        player.rate = desiredRate
    }

    func pause() {
        player.pause()
    }

    func seek(to time: TimeInterval) {
        let target = CMTime(seconds: max(time, 0), preferredTimescale: 600)
        player.seek(to: target, toleranceBefore: .zero, toleranceAfter: .zero)
    }

    func setRate(_ rate: Float) {
        desiredRate = rate
        // Only steer live playback; a paused item keeps its rate for resume.
        if player.rate != 0 {
            player.rate = rate
        }
    }

    func stop() {
        player.pause()
        removeItemNotifications()
        currentItem = nil
        player.replaceCurrentItem(with: nil)
        if let timeObserver {
            player.removeTimeObserver(timeObserver)
            self.timeObserver = nil
        }
#if os(iOS)
        if let interruptionNotification {
            NotificationCenter.default.removeObserver(interruptionNotification)
            self.interruptionNotification = nil
        }
#endif
        reportedReady = false
        reportedFailure = false
#if os(iOS)
        try? AVAudioSession.sharedInstance().setActive(
            false,
            options: .notifyOthersOnDeactivation
        )
#endif
    }

    private func tick() {
        guard let item = currentItem else { return }
        if item.status == .failed {
            reportFailure(item.error?.localizedDescription ?? "Audio is unavailable.")
            return
        }
        let duration = item.duration.seconds
        if item.status == .readyToPlay, !reportedReady {
            reportedReady = true
            onEvent?(.ready(duration: duration))
        }
        if item.status == .readyToPlay {
            onEvent?(.progress(current: item.currentTime().seconds, duration: duration))
        }
    }

#if os(iOS)
    private func handleInterruption(_ raw: UInt?) {
        guard let raw,
              AVAudioSession.InterruptionType(rawValue: raw) == .began else { return }
        player.pause()
        onEvent?(.interrupted)
    }
#endif

    private func reportFailure(_ message: String) {
        guard !reportedFailure else { return }
        reportedFailure = true
        player.pause()
        onEvent?(.failed(message))
    }

    private func removeItemNotifications() {
        for notification in itemNotifications {
            NotificationCenter.default.removeObserver(notification)
        }
        itemNotifications.removeAll()
    }
}
