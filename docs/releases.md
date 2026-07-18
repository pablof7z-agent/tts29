# Standalone releases

The `pablof7z-agent/tts29` repository owns product versions, build inputs, and
release notes. A skill installation directory is never a source or signing
input.

## Product identities

| Product | Identifier | Version source |
| --- | --- | --- |
| iPhone/iPad player | `com.pablof7z.tts29` | `apple/Config/Shared.xcconfig` |
| macOS player | `com.pablof7z.tts29.macos` | `apple/TTS29Mac/Config/Shared.xcconfig` |
| Daemon, CLI, and MCP | Git tag and crate versions | This repository |

Both Apple products use team `456SHKPP26`. Their generated Info.plists declare
`ITSAppUsesNonExemptEncryption = NO` in source. Release tags use one product
version such as `v0.1.0`; Apple build numbers are monotonically increasing UTC
timestamps supplied at archive time.

## Reproducible Apple gates

Build both simulator and device Rust libraries before invoking Xcode:

```bash
scripts/build-rust-ios.sh
xcodebuildmcp simulator test \
  --workspace-path apple/TTS29.xcworkspace \
  --scheme TTS29 \
  --simulator-name "iPhone 17 Pro"
xcodebuildmcp device build \
  --workspace-path apple/TTS29.xcworkspace \
  --scheme TTS29 \
  --configuration Release \
  --extra-args CODE_SIGNING_ALLOWED=NO

scripts/build-rust-macos.sh
xcodebuildmcp swift-package test \
  --package-path apple/TTS29Package \
  --configuration debug
xcodebuildmcp macos build \
  --workspace-path apple/TTS29Mac/TTS29Mac.xcworkspace \
  --scheme TTS29Mac \
  --configuration Release \
  --extra-args CODE_SIGNING_ALLOWED=NO
```

A TestFlight export additionally requires a local Apple Distribution identity,
an installed App Store provisioning profile for `com.pablof7z.tts29`, and an
App Store Connect API key. Export must use manual App Store signing; cloud or
automatic export signing is not an accepted release path. These credentials
remain outside the repository and must never be copied into logs or issues.

The first signed delivery is tracked separately from source readiness because
it changes App Store Connect state and depends on operator-owned credentials.
