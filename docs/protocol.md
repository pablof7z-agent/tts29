# TTS29 group event contract

Version 1 uses immutable kind `9` events in one NIP-29 group. Every event has
exactly one `h` tag for that group and exactly one product marker:

```text
["tts29", <type>, "1"]
```

Unknown types and versions are ignored. Tags described as singular must occur
exactly once; duplicates invalidate the event. Event content remains readable
to an ordinary group client, while the typed tags are the projection contract.

## Spoken item

An item uses type `item`. Its event ID is its durable item ID.

```text
["title", <short title>]
["summary", <non-spoken summary>]
["agent", <display attribution>]
["audio", <https URL>, <lowercase sha256>, <media type>, <byte count>]
["attachment", <https URL>, <lowercase sha256>, <media type>, <byte count>, <label>]
```

There is exactly one audio artifact and at most twelve attachments. An artifact
is portable only when the URL is HTTPS, the digest is 64 lowercase hexadecimal
characters, the media type is explicit, and its nonzero size is at most 250
MiB. The complete item is published only after those immutable bytes are
durable at their URLs.

An item may contain up to three immutable question definitions:

```text
["question", <id>, "single"|"multiple"|"freeform", <title>]
["label", <question id>, <short title>]
["description", <question id>, <description>]
["option", <question id>, <option id>, <title>, <optional description>]
```

Question and option IDs use 1–64 ASCII letters, digits, `_`, or `-`. Choice
questions require one to eight uniquely identified options; freeform questions
have none. A producer may omit `label`, in which case the title is the label.

## Related events

Every related event has one root reference with an empty relay hint:

```text
["e", <spoken item event id>, "", "root"]
```

An `answer` event atomically submits one answer bundle. It has one or more
`["answer", <question id>, <value>...]` tags. Choice values are option IDs;
single choice and freeform have exactly one value. A newer valid answer bundle
replaces the whole older bundle.

An `ack` event carries exactly one of:

```text
["state", "active"|"heard"|"dismissed"|"archived"]
["reason", <optional human-readable reason>]
```

Acknowledgement state is resolved per signing pubkey. `active` restores a
dismissed or archived item. A client applies only the configured viewer's
acknowledgement when selecting that viewer's queue.

A `reaction` event carries exactly one
`["reaction", <emoji>, "add"|"remove"]` tag. Reaction state is resolved per
item, author, and emoji.

## Deterministic projection

For answer bundles, per-author acknowledgements, and per-author reactions, the
winner is the greatest `(created_at, event_id)` tuple. This makes delivery
order irrelevant and gives equal timestamps a stable tie-breaker. Related
events with unknown roots, invalid question IDs, or invalid option values have
no effect.

Items sort by descending `(created_at, event_id)` and the screen projection is
bounded to forty. A viewer's `dismissed` and `archived` items are excluded;
`heard` remains visible. Inputs rejected by the contract are counted in the
snapshot evidence but never become queue items.

## Intentionally local facts

No event in this contract represents playback position, current item,
playing/paused state, audio output availability, autoplay barriers, or whether
a particular device completed playback. Those are device-local facts. Shared
`heard`, `dismissed`, and `archived` states exist only as explicit signed user
actions and are not inferred from playback telemetry.
