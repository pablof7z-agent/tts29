# Real-relay verification record

## Scope

- Claim under test: TTS29 can publish and reacquire a spoken item and related
  answer through a real NIP-29 host, then project and play that exact item in
  the iOS simulator.
- Platform/tier: macOS daemon plus iPhone 17 Pro simulator.
- NMP revision: `5135a36558786d857958347ce30314e39472d5ce`.
- Environment: `wss://nip29.f7z.io`, disposable group
  `tts29-live-20260718221116`, 2026-07-18 GMT.

## Evidence matrix

| Claim | Method | Result | Artifact |
|---|---|---|---|
| Real host accepted group creation | tracked NMP receipt | ACKed | receipt `1`, event `0f3e132bad598679470c6b0612c5707ab8dc20a0a1d68595037f41177a822510` |
| Daemon published one spoken item | private socket plus tracked NMP receipt | ACKed | receipt `2`, event `99b1bc7f4272addcad1a8079846e784956609cf0e7cf5ee839bf355f11653f3f` |
| Independent item reacquisition | fresh NMP engine, exact-ID pinned demand | row returned with the selected host as its sole source | event above; no shortfall |
| Durable audio integrity | fresh HTTPS download with HTTPS-only redirects | 109,484 bytes matched the descriptor | SHA-256 `b73dc334136d53910aa17427d6b7312c9b9bef3e986df57f8f08c465aa0d6472` |
| Independent answer publication | second NMP engine and tracked receipt | ACKed | receipt `1`, event `854af224a503b63ececdf1e52f83a000e6463fb9a90d9745e157002e11a07fa0` |
| Answer reacquisition and relation | third fresh NMP engine, exact-ID pinned demand | root and `live-e2e=confirmed` answer matched | root is the spoken-item event above |
| iOS app projection | real Rust kernel and SwiftUI shell | exact live subject rendered, one queued | XcodeBuildMCP simulator screenshot, 2026-07-18 21:57 GMT |
| iOS playback | DEBUG-only exact-event autoplay hook using the ordinary `PlaybackController` and `AVPlayerBackend` | full progress and visible `Finished` state | XcodeBuildMCP simulator screenshot, 2026-07-18 21:59 GMT |
| Native regression | XcodeBuildMCP simulator test | 8 passed, 0 failed | result bundle `test_sim_2026-07-18T21-58-33-551Z_pid76103_2347329f.xcresult` |

## Failure-path coverage

- [x] The configured `tts` group rejected unauthorized membership repair; the
  runner surfaced `restricted: insufficient permissions` and published no item.
- [x] Exact-event acquisition records the selected host on the returned row and
  keeps historical reconciliation separate.
- [x] Receipt ACK is required independently from event reacquisition.
- [x] HTTP synthesis is admitted only on literal loopback; the configured public
  HTTP endpoint remained rejected.
- [x] Audio redirects are bounded to three HTTPS-only hops.
- [x] Secrets are absent from output and artifacts.
- [x] The group admin and request/response author were generated in memory for
  this run; the public item/answer author was
  `5e130f7a2faf7bf92cc5040f39f58b536faf63fd009002c6e5ea70db6a385644`.
- [x] Each NMP engine, subscription, Unix socket, temporary directory, and
  loopback fixture has deterministic bounded teardown.

## Classification

- Proved: real host ACKs, independent exact-ID readback with host provenance,
  Blossom integrity, related-answer semantics, iOS projection, and completed
  AVPlayer playback.
- Observed but not deterministic: the host returned exact rows while its source
  status remained `Requesting` without a historical reconciliation watermark.
  This is not represented as complete group history.
- Deliberate test seam: the disposable MP3 was generated beforehand with the
  configured TTS client and returned to the daemon by a literal-loopback
  Kokoro-compatible fixture because the configured public synthesis endpoint
  is HTTP-only. The daemon's synthesis request, artifact commit, Blossom upload,
  NMP publication, readback, and app playback paths were real.
- Not proved: the preconfigured `tts` group is writable by the current agent
  identity, and the public Kokoro deployment is transport-safe for production.

## Reproduction

```sh
TTS29_LIVE_AUDIO_FILE=/path/to/disposable.mp3 \
  scripts/run-live-relay-e2e.sh

scripts/build-rust-ios.sh
xcodebuildmcp simulator test \
  --workspace-path apple/TTS29.xcworkspace \
  --scheme TTS29 \
  --simulator-name "iPhone 17 Pro"
```

Launch the Debug app with these public values to reproduce playback through the
same visible controller path:

- launch arguments: `-tts29.relay wss://nip29.f7z.io -tts29.group tts29-live-20260718221116`
- environment: `TTS29_UI_AUTOPLAY_ID=99b1bc7f4272addcad1a8079846e784956609cf0e7cf5ee839bf355f11653f3f`
