# Apple playback boundary

The Rust projection chooses and validates queue items. The Apple shell receives
only bounded `SpokenItem` values and treats their durable audio URLs as native
capability input.

`PlaybackController` owns device-local selection, loading, play/pause intent,
progress, completion, interruption, and failure presentation. None of those
facts cross FFI or become Nostr events. When the selected item disappears from
the next Rust snapshot or its audio URL changes, the controller immediately
stops the old player and clears its local state.

`AVPlayerBackend` is the capability adapter. It owns `AVPlayer`, configures the
spoken-audio session only when playback begins, observes readiness and bounded
progress, handles completion/failure notifications, and pauses on audio-session
interruption. The controller depends on a small backend protocol so lifecycle
behavior is unit-testable without moving policy into AVFoundation callbacks.

Production projections admit HTTPS audio. Debug builds additionally admit a
file URL so the UI test can write a deterministic WAV inside the app sandbox.
That launch fixture injects a completed queue snapshot and never starts the
Rust kernel; it is available only under the `DEBUG` compilation condition.

The simulator gate verifies both local state transitions and an actual AVPlayer
start through XcodeBuildMCP. Unavailable audio remains a visible row-local
failure and never crashes or mutates the shared queue.
