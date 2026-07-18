# TTS29 macOS shell

This XcodeBuildMCP-scaffolded workspace is the native Mac shell for the shared
TTS29 queue and playback feature.

The app imports `../TTS29Package`; it does not duplicate projection, queue, or
playback policy. The shared Rust kernel consumes NMP and emits bounded queue
snapshots. SwiftUI renders those snapshots, while AVPlayer executes playback
locally on this Mac.

Build the host Rust library before invoking the workspace:

```bash
../../scripts/build-rust-macos.sh
xcodebuildmcp swift-package test \
  --package-path ../TTS29Package \
  --configuration debug
xcodebuildmcp macos build-and-run \
  --workspace-path TTS29Mac.xcworkspace \
  --scheme TTS29Mac
```

The app sandbox permits outgoing network access for the NMP relay and durable
audio URLs. Shared controller tests verify local playback lifecycle behavior;
`build-and-run` verifies that the AVPlayer-linked native app launches on macOS.
Standalone version and signing ownership is documented in
[../../docs/releases.md](../../docs/releases.md).
