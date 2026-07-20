# Results and troubleshooting

Read this reference when interpreting a response, recovering a lost call, or
diagnosing a producer failure.

## Successful publication

A single-item success returns:

```json
{
  "status": "published",
  "version": 1,
  "request_id": "stable-request-id",
  "receipt_id": 2,
  "event_id": "<64-hex-event-id>",
  "answer_wait": {"status": "not_requested"}
}
```

Preserve the request, receipt, and event IDs. The daemon returns publication
only after it has durable job state and selected-host delivery evidence. This
does not establish device playback, completion, or hearing.

A tree success returns `status: published_tree`, its stable request ID, the
root event ID, and child event IDs in publication order.

## Retry discipline

If the caller loses the response, retry with the exact same request ID,
content, group, and author. The daemon resumes or returns the journaled result.
Changing content or author while reusing an ID returns `request_conflict`.

Never blindly create a new request ID after an unknown outcome; that can create
a distinct spoken item.

## Diagnose failures

- Missing config: create the private files described in [setup.md](setup.md).
- Daemon startup or socket failure: inspect the non-secret daemon log at
  `TTS29_LOG` or beside the configured socket.
- `request_conflict`: compare the complete request and author with the original;
  do not overwrite the journaled operation.
- Membership failure: report the selected host rejection or unresolved receipt;
  do not bypass the daemon or publish directly.
- Kokoro failure: verify HTTPS endpoint reachability, authentication mode,
  response type, size bound, and timeout without exposing credentials.
- Blossom failure: preserve the integrity or upload error; do not substitute a
  local audio path.
- Publication failure: preserve receipt and relay evidence. A durable accept is
  distinct from a relay acknowledgement.
- Answer timeout or unavailability: report it separately from the already
  published item.

Job journals and work files live at the private paths in daemon configuration.
Do not edit them to force a retry. Use the same supported request boundary.
