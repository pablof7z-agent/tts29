---
name: tts29
description: Publish durable cross-device spoken updates, answerable questions, file attachments, and narrated attachment trees through the TTS29 daemon, local CLI, or hosted MCP boundary. Use when an agent should send speech to a TTS29 NIP-29 group, wait for a related answer, inspect publication evidence, configure the supported producer, or diagnose TTS29 delivery without implementing Nostr, synthesis, upload, or retry logic itself.
---

# TTS29

## Script location

Resolve `<skill-dir>` to the directory containing this `SKILL.md`. Run
`<skill-dir>/scripts/tts`; do not assume the current working directory is the
skill directory or that TTS29 is on `PATH`.

The launcher starts the resident `tts29d` when necessary and submits through
the private `tts29` client. Do not call NMP, a relay, Kokoro, or Blossom
directly. The daemon owns synthesis, durable artifacts, membership repair,
tracked publication, recovery, and answer observation.

## Publish an update

Use a stable agent display name, concise title, factual one-line preview, and
spoken message. Supply a stable request ID whenever a retry may be needed:

```bash
<skill-dir>/scripts/tts \
  --agent-id "<agent-name>" \
  --subject "<concise title>" \
  --summary "<one-line preview>" \
  --message "<spoken update>" \
  --request-id "<stable-id>"
```

Keep the title and agent name stable within one work stream. Make the summary
useful in the queue without repeating the title. Write the message for
listening: put the outcome first, use short sentences, and omit raw logs,
secret values, and terminal noise.

`--message` accepts literal text, `@path`, or `-` for standard input. The
command stays in the foreground until durable publication succeeds or fails.
If the result is lost, retry the identical content and author with the same
request ID. Never create a new ID merely because the first response was lost.

Treat a `published` response and relay event ID as durable publication
evidence, not proof that a device played the audio or that the user heard it.
Only claim audible playback from explicit user confirmation or independent
device evidence.

## Identity and secrets

Use `AGENT_NSEC` from the caller environment when the item must be signed as
the agent. Never put it in arguments, JSON, source, logs, screenshots, or tool
output, and never print or inspect its value. Without it, the daemon identity
signs the item.

Keep the daemon identity and Kokoro credentials in the private TTS29 env file.
Do not add secret fields to daemon configuration. Read
[setup.md](references/setup.md) before configuring or repairing a runtime.

## Conditional guidance

- **Questions and bounded answer waits**: Read
  [asking-questions.md](references/asking-questions.md) before publishing any
  question. Questions use a raw `ProducerRequest`; waits are optional and
  bounded to 300 seconds.
- **Files, Markdown, or narrated attachment branches**: Read
  [rich-content.md](references/rich-content.md). Use a `SpokenTree` for local
  files and narrated children.
- **Response interpretation, retries, or failures**: Read
  [results-and-troubleshooting.md](references/results-and-troubleshooting.md).
- **Hosted assistants or public HTTPS ingress**: Read
  [mcp.md](references/mcp.md). Remote callers use the authenticated
  `publish_speech` tool and cannot supply `AGENT_NSEC`.

## Completion gate

Before reporting success:

1. Preserve the exact request ID, receipt ID, and item event ID returned.
2. Distinguish publication, answer observation, and device playback evidence.
3. Report timeouts or unavailable answer observation without undoing or
   misreporting the already durable item.
4. Confirm that no secret value entered output or a persistent request file.
