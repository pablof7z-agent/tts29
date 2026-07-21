# TTS29 setup

Read this reference when the launcher reports missing configuration, binaries,
credentials, socket readiness, or daemon startup failures. The goal is to lead
the user through setup, not merely repeat the list of requirements.

## Agent-led setup contract

1. Resolve `<skill-dir>` to the directory containing `SKILL.md`.
2. Run `<skill-dir>/scripts/setup --check`. Report its non-secret findings.
3. If config is absent and the user wants this machine configured, run
   `<skill-dir>/scripts/setup --init`. It creates only private skeleton files;
   it never invents infrastructure or secret values and never overwrites an
   existing file.
4. Discover safe values from the current machine and the user's stated target.
   Ask the user only for the missing host/group, service endpoints, and public
   owner key. Explain why each missing value is needed.
5. Configure public values directly. For secrets, ask the user to edit the
   private env file locally. Never ask them to paste a secret into chat.
6. Re-run `scripts/setup --check`, resolve every actionable failure, and tell
   the user before running a first publish because it creates a real durable
   event in the selected group.

Do not inspect, print, summarize, copy, or screenshot secret values. Presence
checks must reveal only `present` or `missing`. Do not silently reuse a legacy
TTS identity, group, or service credential; those are explicit user choices.

## What setup needs

The installed skill contains the TTS29 source and launcher. First use requires:

- `jq`, `python3`, Rust, and Cargo;
- a private daemon configuration;
- a daemon-owned Nostr identity generated privately on first start;
- the human owner's public npub or hex key;
- a production HTTPS Kokoro endpoint; and
- a public HTTPS Blossom server.

The launcher uses `tts29d` and `tts29` from `PATH`, explicit `TTS29D_BIN` and
`TTS29_BIN` overrides, or release binaries it builds from the installed skill.
Missing binaries therefore do not block setup when Rust and Cargo are present.

## Scaffold the private files

The default files are:

```text
~/.config/tts29/daemon.json
~/.config/tts29/env
~/.config/tts29/daemon.key
```

`scripts/setup --init` creates their parent directory with mode `0700`, copies
`daemon/example-config.json` to the config path, creates an empty env file, and
sets both files to mode `0600`. Override the locations with `TTS29_CONFIG` and
`TTS29_ENV_FILE` when a non-default runtime is intentional.

Edit the public config values:

- `host`: the exact `wss://` NIP-29 host;
- `group_id`: the group shared by producers and players;
- `owner_pubkey`: the human owner's public npub or 64-character hex key;
- `daemon_identity_path`: where the daemon generates and reuses its private
  identity (normally `daemon.key` beside the config);
- `kokoro.endpoint`: the production `https://` OpenAI-compatible speech
  endpoint;
- `blossom.server`: the public `https://` upload server; and
- state paths and bounded timeouts, only when the defaults are unsuitable.

Relative socket, journal, work, and NMP store paths resolve beside the config
file. The checked-in example endpoints are placeholders and are intentionally
reported as incomplete.

Literal `localhost`, `127.0.0.1`, and `::1` Kokoro HTTP endpoints are supported
only when `kokoro.allow_insecure_loopback` is explicitly `true`. This exception
is intended for a local service or an encrypted tunnel terminating on loopback;
non-loopback HTTP remains invalid.

## Enter secrets locally

Have the user open the env file in a local editor whose contents are not sent
to the agent transcript when Kokoro authentication is required. It may define:

```bash
TTS29_KOKORO_BEARER='...'
```

Kokoro may instead use the mutually dependent
`TTS29_KOKORO_BASIC_USERNAME` and `TTS29_KOKORO_BASIC_PASSWORD`. Bearer and
basic authentication are mutually exclusive. A service that needs no auth may
omit both. Keep the file at mode `0600`.

`TTS29_DAEMON_NSEC` is an optional deployment override for the daemon-owned
identity. It is never the human owner's key and ordinary setup does not need it.
For a missing group, the daemon creates the group and adds `owner_pubkey` with
the `admin` role. For an existing group, the daemon identity must already be an
administrator. It checks each request author through NMP and adds a missing
member before publishing. Host rejection or ambiguous administrative delivery
stops the item instead of bypassing authorization.

`AGENT_NSEC` is different: it belongs only to the caller environment for one
agent-authored submission. It must not enter daemon config, the env file,
journals, request JSON, logs, or responses.

## Verify without publishing

Re-run:

```bash
<skill-dir>/scripts/setup --check
```

A ready result proves local prerequisites, non-placeholder public config,
private file modes, and the owner public key. It does not prove that service
credentials are valid, the daemon identity has host authority on an existing group, the
services are reachable, or a device can play audio.

## First real verification

Tell the user that this step publishes a durable group event, agree on a stable
request ID, then use the normal launcher:

```bash
<skill-dir>/scripts/tts \
  --agent-id "<agent-name>" \
  --subject "TTS29 setup complete" \
  --summary "The producer completed its first publication." \
  --message "TTS29 setup is complete on this machine." \
  --request-id "<stable-setup-verification-id>"
```

The launcher starts the resident daemon when necessary. It writes daemon output
to `TTS29_LOG` or `tts29d.log` beside the socket. Preserve the returned request,
receipt, and event IDs. A `published` response proves durable publication, not
device playback; confirm playback separately in an Apple client using the same
host and group.

## If setup still fails

- Re-run `scripts/setup --check` after each config change.
- For daemon startup or socket failure, inspect only the non-secret daemon log.
- For Kokoro or Blossom failure, verify the configured public URL, auth mode,
  response contract, and reachability without exposing credentials.
- For membership failure, confirm the daemon identity is an administrator of
  the exact configured host/group; do not publish around the daemon.
- For publication or answer failures, continue with
  [results-and-troubleshooting.md](results-and-troubleshooting.md).
