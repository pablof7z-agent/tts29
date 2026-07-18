# TTS29 Apple shell

This workspace contains the native iOS shell for TTS29. The app target is
minimal, and the local Swift package renders bounded snapshots from the
app-specific Rust kernel.

Swift does not interpret Nostr events, decide queue order, cache protocol
state, or call relays. Those responsibilities remain on the Rust side of the
boundary, with all Nostr access going through NMP's public APIs.

Build the Rust simulator library before invoking the workspace:

```bash
../scripts/build-rust-ios.sh
xcodebuildmcp simulator test \
  --workspace-path TTS29.xcworkspace \
  --scheme TTS29 \
  --simulator-name "iPhone 17 Pro"
```

The project was initially scaffolded with XcodeBuildMCP. Continue to use its
CLI for builds, tests, launches, logs, and simulator UI inspection.
