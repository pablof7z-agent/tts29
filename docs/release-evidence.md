# Release evidence

Copy this document into the GitHub release or the delivery issue and replace
every placeholder. Do not attach private keys, certificates, provisioning
profile payloads, passwords, or notarization credentials.

## Source

- Product version: `<version>`
- Git tag: `<tag>`
- Commit: `<full commit SHA>`
- Apple build number: `<UTC YYYYMMDDHHMM>`
- Xcode version: `<version>`
- NMP revision: `<exact pinned revision from core/Cargo.toml>`
- Source worktree was clean at archive time: `<yes/no>`

## iOS and TestFlight

- Bundle identifier: `com.pablof7z.tts29`
- Signing team: `456SHKPP26`
- Distribution identity label: `<label only>`
- Provisioning profile name and UUID: `<name / UUID>`
- App Store Connect key ID: `<key ID only>`
- Exported IPA SHA-256: `<digest>`
- Upload delivery UUID: `<UUID>`
- App Store Connect processing result: `<result and timestamp>`
- TestFlight installation evidence: `<device, OS, result>`

## macOS

- Bundle identifier: `com.pablof7z.tts29.macos`
- Signing team: `456SHKPP26`
- Developer ID identity label: `<label only>`
- Exported artifact SHA-256: `<digest>`
- Notarization request ID: `<ID>`
- Notarization result: `<result and timestamp>`
- Stapling and Gatekeeper verification: `<result>`
- Clean-machine launch evidence: `<Mac, macOS, result>`

## Product-boundary checks

- Both products came only from the tagged TTS29 repository: `<yes/no>`
- No skill installation cache was used: `<yes/no>`
- No NMP source or dependency revision was modified for the release: `<yes/no>`
- The NIP-29 group remains canonical truth after rollback testing: `<evidence>`
- Rollback or withdrawal action, if needed: `<action>`
