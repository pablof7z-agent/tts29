# Architecture boundary

## Ownership

| Fact or behavior | Single owner |
| --- | --- |
| Canonical Nostr events, relay query, evidence, routing, signing, receipts | NMP |
| Spoken-item interpretation, queue ordering, answer policy, bounded screen projection | TTS29 Rust kernel |
| Navigation state and product actions | TTS29 Rust kernel |
| SwiftUI rendering and accessibility | Apple shell |
| Raw device-local relay/group bootstrap preferences | Apple storage capability |
| User-secret persistence and deletion | Apple Keychain capability |
| Audio session and playback execution | Apple capability bridge |
| Current device position, pause state, autoplay barrier | Device-local state |

The app does not create a second event cache. It receives NMP rows through one
windowed observation and emits a bounded screen-shaped projection. The NMP
subscription is cancelled through its public cancellation handle; the owning
thread then shuts down the engine deterministically.

The Apple shell may persist the two raw values a user enters for the next
launch. They are bootstrap capability input, not a mirror of queue or Nostr
state. Swift does not interpret the values or use them to contact a relay; the
Rust kernel parses them, starts NMP, and reports invalid configuration through
the bounded lifecycle snapshot.

The human user's `nsec` crosses a dedicated secret FFI function and is never
serialized into an action or snapshot. Rust registers and activates that exact
NMP account. Swift executes only kernel-requested Keychain store/delete
operations and returns raw capability success or failure; Rust owns the login
state transition. A submitted answer is a semantic action containing bounded
question IDs and values. Rust validates it against the current projection,
composes the kind:9 answer, and NMP signs and publishes it as the active user's
pubkey. Logout deletes the Keychain credential before Rust removes the NMP
account. The daemon identity is separate and retains group-administration
responsibility.

## First slice data flow

```text
NIP-29 host
    |
    v
NMP windowed group query -- rows + scoped evidence
    |
    v
TTS29 Rust projection -- at most 40 spoken items
    |
    v
C callback carrying one bounded JSON snapshot
    |
    v
Swift AsyncStream -- main-actor snapshot replacement -- SwiftUI
```

The callback is event-driven. There is no timer, sleep-check loop, or native
event mirror. Swift cancellation causes the Rust observation cancellation
handle to fire, unblocking the query owner before NMP engine shutdown.

## Nondeterministic inputs

- Relay events enter only through the NMP subscription.
- Relay acquisition state enters only through NMP scoped evidence.
- Application lifecycle cancellation enters through the FFI stop action.
- User credentials enter through the dedicated login capability boundary;
  Keychain results return as raw capability input to the Rust state machine.
- Apple audio callbacks enter only the device-local playback controller and are
  never projected as shared queue facts.

## Bounds

- NMP query window: 40 initial rows, 100 maximum.
- Cross-FFI queue projection: 40 spoken items.
- One observation owner and one callback stream per running app kernel.
- UI applies snapshots on the main actor and performs no event parsing.

## Architecture scan note

The NMP architecture scanner reports `D6/no-ffi-errors` warnings on the three
Rust C-ABI declarations because its heuristic matches `extern "C"`. The ABI
does not transport Rust `Result` values, throw native exceptions, or expose an
error object. Runtime failures are serialized into `QueueSnapshot` state;
invalid startup input is represented by a null handle and immediately mapped
to failed snapshot state by Swift.

The scanner also reports `D4/native-cache-smell` for the `UserDefaults` calls
in `ConnectionSettings.swift`. Those keys are the source of raw device-local
bootstrap input for the next kernel launch. They do not cache or mirror any
Rust-owned projection, event, routing decision, or protocol fact.

It reports `D6/no-ffi-errors` on `KeychainCredentialVault` because the native
capability uses Swift errors internally. Those errors do not become app policy
or cross FFI as thrown values: Swift reduces them to raw capability results and
the Rust kernel projects the resulting account state and user-facing error.
