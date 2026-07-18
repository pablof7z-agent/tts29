@preconcurrency import AVFoundation
import Foundation

@MainActor
final class AVPlayerBackend: AudioPlaybackBackend {
    var onEvent: ((AudioPlaybackEvent) -> Void)?

    private let player = AVPlayer()
    private var currentItem: AVPlayerItem?
    private var timeObserver: Any?
    private var notifications: [NSObjectProtocol] = []
    private var reportedReady = false
    private var reportedFailure = false

    init() {
        let interval = CMTime(seconds: 0.25, preferredTimescale: 600)
        timeObserver = player.addPeriodicTimeObserver(
            forInterval: interval,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.tick()
            }
        }
        let interruption = NotificationCenter.default.addObserver(
            forName: AVAudioSession.interruptionNotification,
            object: AVAudioSession.sharedInstance(),
            queue: .main
        ) { [weak self] note in
            let rawType = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
            MainActor.assumeIsolated {
                self?.handleInterruption(rawType)
            }
        }
        notifications.append(interruption)
    }

    func load(_ url: URL) {
        removeItemNotifications()
        reportedReady = false
        reportedFailure = false
        let item = AVPlayerItem(url: url)
        currentItem = item
        player.replaceCurrentItem(with: item)
        let center = NotificationCenter.default
        notifications.append(center.addObserver(
            forName: .AVPlayerItemDidPlayToEndTime,
            object: item,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.onEvent?(.completed)
            }
        })
        notifications.append(center.addObserver(
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

    func play() {
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playback, mode: .spokenAudio)
            try session.setActive(true)
            if player.currentItem?.currentTime() == player.currentItem?.duration {
                player.seek(to: .zero)
            }
            player.play()
        } catch {
            reportFailure("The audio session is unavailable.")
        }
    }

    func pause() {
        player.pause()
    }

    func stop() {
        player.pause()
        removeItemNotifications()
        currentItem = nil
        player.replaceCurrentItem(with: nil)
        reportedReady = false
        reportedFailure = false
        try? AVAudioSession.sharedInstance().setActive(
            false,
            options: .notifyOthersOnDeactivation
        )
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

    private func handleInterruption(_ raw: UInt?) {
        guard let raw,
              AVAudioSession.InterruptionType(rawValue: raw) == .began else { return }
        player.pause()
        onEvent?(.interrupted)
    }

    private func reportFailure(_ message: String) {
        guard !reportedFailure else { return }
        reportedFailure = true
        player.pause()
        onEvent?(.failed(message))
    }

    private func removeItemNotifications() {
        guard notifications.count > 1 else { return }
        for notification in notifications.dropFirst() {
            NotificationCenter.default.removeObserver(notification)
        }
        notifications.removeSubrange(1...)
    }
}
