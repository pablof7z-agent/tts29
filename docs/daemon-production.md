# Production producer boundary

`ProductionProducer` wires the recovery runner to real capabilities while
keeping transport and signing inside NMP's supported public surface.

## Configuration

Startup requires:

- private journal and work directories;
- an optional persistent NMP store path;
- one producer secret key accepted by `Engine::add_account`;
- one NIP-29 host and group ID;
- a Kokoro speech endpoint and optional bearer or basic authentication;
- one Blossom server; and
- bounded request, upload-authorization, and receipt timeouts.

The secret key and Kokoro credentials are capability configuration. They have
no serialization or debug representation and are never copied into job JSON.
Invalid keys, URLs, group IDs, and zero-sized limits fail startup or the current
stage without exposing credentials.

## Kokoro

The synthesizer sends the immutable job body and voice as an OpenAI-compatible
`POST` requesting Kokoro MP3 output. Production endpoints require HTTPS. Plain
HTTP is available only for explicitly admitted loopback integration tests.
Redirects and ambient proxies are disabled, the response is size-bounded, and
non-MP3 response types fail closed.

Audio is flushed to a private staged file and linked into the deterministic job
path with create-if-absent semantics. A retry or competing process reuses the
winning complete file rather than overwriting it.

## Blossom

The uploader uses the pinned `nmp-blossom` module. It hashes the exact local
bytes, asks the active NMP account to sign the module's kind `24242` upload
draft, validates that signed authorization, and performs an integrity-checked
BUD-02 upload. The returned URL must be public HTTPS and its digest, size, and
media type must agree with the journaled audio evidence.

The content digest is the artifact identity, so a retry targets the same blob.

## Group publication

The same NMP engine owns the active account and tracked NIP-29 publication.
`NmpPublisher` rejects items outside the configured group. The recovery runner
freezes the complete item before acceptance, journals the receipt ID, and then
reattaches that receipt until the configured host acknowledges the signed event.

Before publication, the daemon observes current kind `39001` and `39002` group
state through a strict NMP demand pinned to the selected host. The read must
carry reconciled source evidence. If the frozen spoken-item author is absent,
the daemon composes kind `9000` through the public, kind-agnostic
`nmp-nip29` group composer. NMP owns the `h` tag, exact-host route, daemon
signing, durable acceptance, stable correlation, receipt stream, retry, and
rejection. TTS29 never opens a parallel Nostr client or modifies NMP.

Membership acceptance and authorization are separate journal stages. The
membership event ID and optional retained receipt ID remain in the final
published job. A retry after the NMP acceptance gap uses a stable correlation
derived from the request ID and member pubkey, so it reattaches the same NMP
obligation rather than creating untracked administrative work.

For a local request carrying `AGENT_NSEC`, the daemon temporarily registers the
key through `Engine::add_account`. The frozen item author is also the explicit
NMP per-write identity override, so publication uses that secondary signer
without changing the daemon's active identity. The request holds the opaque
NMP registration and removes that exact installation after the attempt. A
retry supplies the signer again and resumes the author-bound journal record;
the secret itself is never copied into the record.

The local CLI and Unix-socket contract are documented in
[local-producer.md](local-producer.md).

## Bounded answer waits

After a job reaches `published`, a caller may open one explicit answer wait with
its own timeout and cancellation token. The daemon observes the configured
group through `group_content_demand`; it creates no parallel Nostr client or
answer cache. Only answers related to the published event and valid for its
frozen questions qualify.

The shared protocol validator and `(created_at, event_id)` conflict order are
used by both this operation and the Apple queue projection. Cancellation,
timeout, engine closure, and a valid answer remain distinct results. The NMP
subscription is withdrawn when the operation ends, and the daemon records no
playback or item-ownership state.

Integration tests run real HTTP and WebSocket boundaries on explicitly admitted
loopback servers and exercise NMP membership observation and repair, existing
membership, host rejection, request signing, Blossom integrity validation,
tracked receipt acceptance, bounded answer observation, and Kokoro restart reuse
without modifying NMP.
