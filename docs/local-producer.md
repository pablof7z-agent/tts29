# Local producer surface

TTS29 ships two separate executable roles from the `daemon` crate:

- `tts29d` owns NMP, Kokoro, Blossom, recovery, and optional answer waits;
- `tts29` is a thin local client that sends one producer request to the daemon.

The client never initializes NMP or receives daemon credentials. The boundary
is versioned JSON over a Unix socket, limited to 128 KiB and one request per
connection. The socket is mode `0600` inside a directory inaccessible to group
and other users. Startup refuses a live listener, a non-socket collision, or an
unsafe parent; it removes only a demonstrably stale socket with the same
filesystem identity. SIGINT and SIGTERM stop admission after the current
bounded request, shut NMP down, and remove the owned socket.

## Configure and start the daemon

Copy `daemon/example-config.json` and replace the Kokoro and Blossom endpoints.
Relative state paths resolve beside the config file. Secrets are intentionally
not valid config fields:

```bash
export TTS29_DAEMON_NSEC='nsec1...'
export TTS29_KOKORO_BEARER='...'
cargo run --manifest-path daemon/Cargo.toml --bin tts29d -- \
  --config daemon/example-config.json
```

Kokoro can instead use `TTS29_KOKORO_BASIC_USERNAME` and
`TTS29_KOKORO_BASIC_PASSWORD`. Authentication modes are mutually exclusive.
The daemon key, Kokoro credentials, and request-only agent key have no config,
journal, response, or debug representation.

The configured daemon identity must have authority to add publishers to the
NIP-29 group. Before spoken publication, the daemon reads current admin/member
state from the selected host through NMP. An existing member proceeds without
an administrative write; a missing request author is added through the
daemon-owned identity and the public NMP NIP-29 group composer. Host refusal or
ambiguous delivery stops publication with retained receipt evidence.

## Submit speech

The CLI accepts a `ProducerRequest` JSON document from standard input or
`--request <file>`. `TTS29_SOCKET` can replace `--socket`:

```bash
cargo run --manifest-path daemon/Cargo.toml --bin tts29 -- \
  --socket daemon/state/runtime/daemon.sock <<'JSON'
{
  "request_id": "agent-build-42",
  "group_id": "tts",
  "voice": "af_heart",
  "agent_name": "Codex",
  "subject": "Build complete",
  "summary": "The verified build is ready.",
  "body": "The verified build is ready for review.",
  "attachments": [],
  "questions": []
}
JSON
```

A successful response contains the stable request ID, NMP receipt ID, signed
event ID, and answer-wait result. A retry with the same request and author
resumes the journaled job. Reusing the ID with changed content or author returns
`request_conflict` instead of creating a second item.

Set `AGENT_NSEC` on the CLI process to publish the spoken item as that agent.
The key exists only in the local request frame. The daemon registers it through
`Engine::add_account`, the protocol sets that frozen author as NMP's explicit
per-write identity override, and the exact registration is removed after the
attempt. Blossom authorization remains a daemon-owned artifact operation.

For a request containing questions, `--wait-seconds <1-300>` asks the daemon to
make one bounded NMP answer observation after publication. Timeout is returned
alongside the durable publication evidence; it does not undo publication or
make the caller own the spoken item.

This request/response model is also the contract boundary used by the HTTPS
MCP ingress. That adapter adds remote authentication and admission, then calls
the same daemon service rather than reproducing producer capabilities. See
[remote-producer.md](remote-producer.md).
