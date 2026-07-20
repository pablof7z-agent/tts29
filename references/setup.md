# TTS29 setup

Read this reference when the launcher reports missing configuration, binaries,
credentials, socket readiness, or daemon startup failures.

## Requirements

The installed skill contains the TTS29 source and launcher. First use requires:

- `jq`, `python3`, Rust, and Cargo;
- a private daemon configuration;
- a daemon Nostr identity with authority to add publishers to the selected
  NIP-29 group;
- a production HTTPS Kokoro endpoint; and
- a Blossom server.

The launcher uses `tts29d` and `tts29` from `PATH`, explicit `TTS29D_BIN` and
`TTS29_BIN` overrides, or release binaries built from the installed skill.

## Configuration

The default files are:

```text
~/.config/tts29/daemon.json
~/.config/tts29/env
```

Start from `<skill-dir>/daemon/example-config.json`. Set the socket, journal,
work, and optional NMP store paths; selected relay and group; Kokoro endpoint;
Blossom server; and bounded timeouts. Relative state paths resolve beside the
configuration file.

Override the locations with `TTS29_CONFIG` and `TTS29_ENV_FILE`.

## Secret boundary

Keep the env file private and outside the repository. It may define:

- `TTS29_DAEMON_NSEC`;
- `TTS29_KOKORO_BEARER`; or
- the mutually exclusive `TTS29_KOKORO_BASIC_USERNAME` and
  `TTS29_KOKORO_BASIC_PASSWORD`.

Use mode `0600`. Never print, commit, screenshot, or copy these values into a
request. `AGENT_NSEC` belongs to the caller environment for one agent-authored
submission; the daemon does not persist or return it.

## Runtime

`<skill-dir>/scripts/tts` reads the configured socket. If nothing is listening,
it starts `tts29d`, waits for readiness, and writes daemon output to
`TTS29_LOG` or `tts29d.log` beside the socket. It refuses startup without the
daemon identity.

The daemon identity must be able to manage membership on the configured host.
The daemon checks the caller author through NMP and adds a missing member before
publishing. A host rejection or ambiguous administrative delivery stops the
spoken item rather than bypassing authorization.
