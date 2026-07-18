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

Integration tests run real HTTP boundaries on explicitly admitted loopback
servers and exercise NMP signing, Blossom integrity validation, tracked receipt
acceptance, group refusal, and Kokoro restart reuse without modifying NMP.
