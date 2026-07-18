# Producer recovery contract

The producer daemon owns request admission and preparation until one immutable
spoken item is durably accepted by NMP. It never owns queue playback.

## Stable admission

A caller supplies a request ID made from 1–64 ASCII letters, digits, `_`, or
`-`. Admission stores a SHA-256 digest of the complete structured request,
the selected author, and one frozen event timestamp.

Reusing the ID with the same request and author returns the existing job.
Reusing it with different immutable input fails closed. New records use an
atomic create-if-absent hard link, so concurrent processes cannot each admit a
different job under the same ID.

Journal updates are written to a private temporary file, flushed, atomically
renamed, and followed by a directory sync. A process loss therefore exposes
the previous complete stage or the next complete stage, never partial JSON.

## Stages

| Stage | Durable evidence | Safe recovery action |
| --- | --- | --- |
| `admitted` | Request digest, author, event time | Synthesize at the deterministic job path |
| `synthesized` | Path, digest, media type, byte count | Reuse verified bytes and upload by digest |
| `artifacts_durable` | Exact frozen protocol item | Submit the identical NMP write intent |
| `publication_accepted` | Frozen item and NMP receipt ID | Reattach to the receipt through NMP |
| `published` | Receipt ID and event ID | Return existing publication evidence |

The runner hashes the synthesized file itself and rejects claimed metadata that
does not match its bytes. Durable audio metadata must retain that digest, media
type, and size. The shared protocol crate validates the complete frozen item
before publication.

## Acceptance gap

NMP durably accepts a write before the daemon can durably record its receipt
ID; these are separate stores and cannot share an atomic transaction. The
daemon closes the user-visible duplication gap by freezing author, timestamp,
content, group, and every tag before acceptance. Retrying that exact item
computes the same Nostr event ID. It may create another receipt obligation, but
it cannot create a second spoken item identity.

Once the receipt ID is journaled, recovery uses NMP receipt reattachment rather
than submitting again. Only an acknowledged signed event advances the job to
`published`.

## Fault-injection gate

Tests discard the journal update once at every stage boundary. Recovery proves:

- synthesis may be invoked again but reuses one deterministic local file;
- upload may be invoked again but addresses one digest;
- lost acceptance evidence may allocate another receipt but yields one event
  ID;
- lost terminal evidence reattaches the saved receipt;
- the final journal reaches `published` without changing immutable input.
