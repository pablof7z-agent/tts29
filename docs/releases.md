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
timestamps committed to both `Shared.xcconfig` files before the release tag.

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

The notarized macOS artifact additionally requires a local Developer ID
Application identity. The Apple Development identity used for development
builds is not a substitute for either distribution identity.

## Release preflight

Validate repository-owned metadata while preparing a change:

```bash
scripts/check-release-readiness.sh --source-only
```

In a release-preparation pull request, set `CURRENT_PROJECT_VERSION` in both
Apple `Shared.xcconfig` files to the same 12-digit UTC timestamp. Merge that
change, tag the clean release commit, and supply only credential metadata and
paths to the full preflight. The script verifies the configured build number
and never prints credential or profile contents:

```bash
TTS29_RELEASE_TAG=v0.1.0 \
TTS29_BUILD_NUMBER=202607181830 \
TTS29_ASC_KEY_ID=KEY_ID \
TTS29_ASC_ISSUER_ID=ISSUER_UUID \
scripts/check-release-readiness.sh
```

The default API key location is
`~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8`. Override it with
`TTS29_ASC_KEY_PATH` and override the provisioning-profile directory with
`TTS29_PROFILE_DIR` when the operator keeps either outside Xcode's standard
MobileDevice or UserData profile locations.

## Signed delivery

1. Obtain explicit authority for App Store Connect upload and notarization.
2. Merge the shared timestamp build-number change, then tag that clean commit
   with the shared marketing version.
3. Run the Rust builds and every XcodeBuildMCP gate above, then run the full
   release preflight.
4. Archive the iOS app in Xcode and distribute it through Organizer using the
   local Apple Distribution identity and matching App Store profile. Keep
   signing manual and upload to App Store Connect.
5. After processing completes, install the TestFlight build on a real device
   and verify launch, group projection, and audio playback.
6. Archive the macOS app in Xcode, distribute it with the local Developer ID
   Application identity, submit it for notarization, staple the accepted
   ticket, and verify it on a clean Mac.
7. Record the complete non-secret result using
   [release-evidence.md](release-evidence.md).

The installed XcodeBuildMCP CLI currently covers the required source builds,
tests, and simulator/device gates but does not expose archive or export
commands. Until it does, signed archive/export uses Xcode Organizer; raw
`xcodebuild`, `xcrun`, and `simctl` commands remain outside this repository's
approved workflow.

The first signed delivery is tracked separately from source readiness because
it changes App Store Connect state and depends on operator-owned credentials.
