# TTS29

TTS29 is a durable cross-device spoken queue built on NIP-29 and NMP.

The NIP-29 group is canonical truth. Producers publish complete immutable
spoken items, clients reconstruct the same queue, and each device owns its own
playback position and autoplay policy. A local database is a reconstructible
projection, never the authority.

## Current slice

The first vertical slice proves the architecture on iOS:

- an app-specific Rust kernel consumes NMP's public Rust facade;
- NMP owns the pinned group query, canonical event store, relay work, and
  acquisition evidence;
- the Rust kernel validates the versioned spoken-item contract and bounds the
  convergent queue projection;
- SwiftUI renders that projection without implementing Nostr or product policy;
- lifecycle cancellation withdraws the query and shuts NMP down cleanly.

The live development defaults point at the existing public TTS group on
`wss://nip29.f7z.io`, group `tts`. They are bootstrap defaults, not a claim
that one public group is the finished account model.

## Build the iOS slice

Requirements: Rust with the `aarch64-apple-ios-sim` target, Xcode, and
XcodeBuildMCP CLI.

```bash
scripts/build-rust-ios.sh
xcodebuildmcp simulator test \
  --workspace-path apple/TTS29.xcworkspace \
  --scheme TTS29 \
  --simulator-name "iPhone 17 Pro"
xcodebuildmcp simulator build-and-run \
  --workspace-path apple/TTS29.xcworkspace \
  --scheme TTS29 \
  --simulator-name "iPhone 17 Pro"
```

The Rust dependency is pinned to an exact NMP revision in `core/Cargo.toml`.
Building TTS29 never edits the NMP checkout or repository.

The executable event contract is documented in [docs/protocol.md](docs/protocol.md).

## Work tracking

The public roadmap is [GitHub Project TTS29](https://github.com/users/pablof7z-agent/projects/1).
Product outcomes live in issues labeled `epic`; implementation starts from a
need-focused child issue and lands through a merged pull request.
