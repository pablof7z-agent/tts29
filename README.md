# TTS29

TTS29 is a durable cross-device spoken queue built on NIP-29 and NMP.

The NIP-29 group is canonical truth. Producers publish complete immutable
spoken items, clients reconstruct the same queue, and each device owns its own
playback position and autoplay policy. A local database is a reconstructible
projection, never the authority.

## Current product slices

The Apple slice proves the architecture on iOS and macOS:

- an app-specific Rust kernel consumes NMP's public Rust facade;
- NMP owns the pinned group query, canonical event store, relay work, and
  acquisition evidence;
- the Rust kernel validates the versioned spoken-item contract and bounds the
  convergent queue projection;
- shared SwiftUI renders that projection without implementing Nostr or product
  policy, while each platform executes playback locally through AVPlayer;
- lifecycle cancellation withdraws the query and shuts NMP down cleanly.

The producer slice provides a crash-recoverable daemon plus thin local and
hosted ingress surfaces:

- the daemon owns Kokoro, NMP-backed Blossom upload, tracked NIP-29
  publication, journals, and bounded answer waits;
- a private versioned Unix-socket request returns durable receipt and event
  evidence;
- `AGENT_NSEC` can authorize one request through NMP's per-write identity
  override without entering config, journals, logs, or responses; and
- retrying the same request ID resumes the author-bound job instead of creating
  another spoken item; and
- the daemon verifies request-author membership and repairs missing membership
  through its authorized identity before spoken publication; and
- a separate HTTPS MCP process validates OAuth issuer, audience, and publish
  scope before forwarding the same bounded request over the private socket.

The live development defaults point at the existing public TTS group on
`wss://nip29.f7z.io`, group `tts`. They are bootstrap defaults, not a claim
that one public group is the finished account model.

## Build the Apple clients

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

scripts/build-rust-macos.sh
xcodebuildmcp swift-package test \
  --package-path apple/TTS29Package \
  --configuration debug
xcodebuildmcp macos build-and-run \
  --workspace-path apple/TTS29Mac/TTS29Mac.xcworkspace \
  --scheme TTS29Mac
```

The Rust dependency is pinned to an exact NMP revision in `core/Cargo.toml`.
Building TTS29 never edits the NMP checkout or repository.

The executable event contract is documented in [docs/protocol.md](docs/protocol.md).
The intentional hard cut from the paired-device product is documented in
[docs/migration.md](docs/migration.md).

## Product crates

- `protocol`: shared frozen event model, validation, parsing, and NMP NIP-29
  composition;
- `core`: app kernel that observes NMP and projects the queue for native shells;
- `daemon`: durable producer admission and crash-recovery stage runner;
- `contract`: data-only spoken-item values shared without importing NMP;
- `producer-api`: versioned private request/response contract and Unix client;
- `mcp`: HTTPS/OAuth MCP ingress that has no daemon or NMP dependency.

The daemon recovery contract is documented in
[docs/daemon-recovery.md](docs/daemon-recovery.md).
Its production Kokoro, Blossom, identity, and group boundary is documented in
[docs/daemon-production.md](docs/daemon-production.md).
The local daemon and CLI contract is documented in
[docs/local-producer.md](docs/local-producer.md).
The hosted assistant boundary is documented in
[docs/remote-producer.md](docs/remote-producer.md).
Standalone release ownership and signing prerequisites are documented in
[docs/releases.md](docs/releases.md), with a non-secret delivery record in
[docs/release-evidence.md](docs/release-evidence.md).

## Real-relay verification

Issue-focused live verification creates a unique disposable group, drives the
real daemon through its private socket, requires host ACKs, reacquires the
exact item and answer through fresh NMP engines, and verifies the uploaded
audio by hash. Supply a non-sensitive MP3 when the configured Kokoro service
is not available through HTTPS or literal loopback:

```bash
TTS29_LIVE_AUDIO_FILE=/path/to/disposable.mp3 \
  scripts/run-live-relay-e2e.sh
```

The runner generates a disposable daemon identity, emits only public JSON
evidence, bounds every network wait, and removes its private local state and
loopback synthesis fixture on exit. The recorded real run is in
[docs/live-relay-verification.md](docs/live-relay-verification.md).

## Work tracking

The public roadmap is [GitHub Project TTS29](https://github.com/users/pablof7z-agent/projects/1).
Product outcomes live in issues labeled `epic`; implementation starts from a
need-focused child issue and lands through a merged pull request.
