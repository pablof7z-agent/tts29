import Foundation
import MediaPlayer

/// Mirrors playback to the system now-playing surfaces (Control Center, lock
/// screen, media keys) and routes their remote commands back to the player.
/// This is an Apple platform capability, not product policy.
@MainActor
final class NowPlayingCenter {
    private weak var playback: PlaybackController?
    private var wired = false

    func attach(to playback: PlaybackController) {
        self.playback = playback
        playback.onPlaybackChange = { [weak self] controller in
            self?.update(from: controller)
        }
        wireCommands()
        update(from: playback)
    }

    private func wireCommands() {
        guard !wired else { return }
        wired = true
        let center = MPRemoteCommandCenter.shared()

        center.playCommand.addTarget { [weak self] _ in
            self?.resume(); return .success
        }
        center.pauseCommand.addTarget { [weak self] _ in
            self?.pauseIfPlaying(); return .success
        }
        center.togglePlayPauseCommand.addTarget { [weak self] _ in
            guard let item = self?.playback?.selectedItem else { return .noSuchContent }
            self?.playback?.toggle(item); return .success
        }
        center.skipForwardCommand.preferredIntervals = [NSNumber(value: PlaybackController.skipInterval)]
        center.skipForwardCommand.addTarget { [weak self] _ in
            self?.playback?.skipForward(); return .success
        }
        center.skipBackwardCommand.preferredIntervals = [NSNumber(value: PlaybackController.skipInterval)]
        center.skipBackwardCommand.addTarget { [weak self] _ in
            self?.playback?.skipBackward(); return .success
        }
        center.changePlaybackPositionCommand.addTarget { [weak self] event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            self?.playback?.seek(to: event.positionTime); return .success
        }
    }

    private func resume() {
        guard let playback, let item = playback.selectedItem, !playback.isPlaying else { return }
        playback.toggle(item)
    }

    private func pauseIfPlaying() {
        guard let playback, let item = playback.selectedItem, playback.isPlaying else { return }
        playback.toggle(item)
    }

    private func update(from playback: PlaybackController) {
        let center = MPNowPlayingInfoCenter.default()
        guard let item = playback.selectedItem else {
            center.nowPlayingInfo = nil
            center.playbackState = .stopped
            return
        }
        var info: [String: Any] = [:]
        info[MPMediaItemPropertyTitle] = item.subject
        info[MPMediaItemPropertyArtist] = AgentIdentity(item).displayName
        if playback.duration > 0 {
            info[MPMediaItemPropertyPlaybackDuration] = playback.duration
        }
        info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = playback.currentTime
        info[MPNowPlayingInfoPropertyPlaybackRate] = playback.isPlaying ? playback.rate : 0
        center.nowPlayingInfo = info
        center.playbackState = playback.isPlaying ? .playing : .paused
    }
}
