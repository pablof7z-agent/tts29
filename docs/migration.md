# Standalone cutover

TTS29 makes a hard cut from the former paired-device product. It does not
import the legacy queue or keep the paired laptop as a compatibility authority.

The old queue was local transport state rather than a canonical shared log.
Its audio could depend on a device-local path, its identity was not the TTS29
event ID, and playback/pairing records describe one device rather than a user
action. Importing those records would manufacture durable NIP-29 facts without
portable artifacts or a trustworthy shared author. That would weaken the new
contract more than it would preserve user data.

## State disposition

| Legacy state | Cutover treatment |
| --- | --- |
| Paired-device queue and delivery acknowledgements | Retired; no canonical identity to import |
| Device pairing, peer addresses, and transport credentials | Retired; not part of TTS29 |
| Local audio files and skill cache entries | Retired; not guaranteed durable or portable |
| Playback position, current item, autoplay, and device completion | Retired as device-local evidence |
| Reusable text or attachments chosen by a person | Submit as a new TTS29 request with new durable artifacts |

The legacy application may be kept temporarily only to review old local data.
It must not bridge, dual-write, or project state into the TTS29 group. There is
no mixed-authority transition period.

## Fresh standalone setup

1. Choose the user-controlled NIP-29 host and group and configure the human
   owner's public key. The daemon creates its own identity and, for a new group,
   grants that public key the `admin` role. An existing group must already trust
   the daemon identity.
2. Configure and start `tts29d` with its private journal, NMP store, Kokoro,
   Blossom, and group settings from [local-producer.md](local-producer.md).
3. Open **Connection** in each Apple client, enter the same host/group, and
   relaunch. Each client reconstructs the queue from NMP; no client copies a
   queue from another device.
4. Submit local speech through `tts29` or hosted speech through the authenticated
   HTTPS MCP adapter. A request-scoped agent key is authorized by the daemon
   before its spoken item is published.
5. Keep old paired processes stopped. Delete their local data only when the
   user no longer needs it for reference.

The migration is complete when a new spoken item published through either
producer surface appears independently on iPhone and macOS, and stopping either
player or the originating agent does not affect the other projection.
