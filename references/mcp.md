# Hosted TTS29 MCP

Read this reference only for hosted assistants or deployment of the public
HTTPS ingress.

## Consumer contract

The MCP server exposes one tool, `publish_speech`. Pass a complete
`ProducerRequest` and an optional answer wait from 1 to 300 seconds. The result
is the same versioned response used by the local CLI in both readable content
and `structuredContent`.

Use a stable request ID and retry the identical request after a lost response.
Remote callers cannot provide `AGENT_NSEC`; never add signer material to tool
arguments.

## Boundary

`tts29-mcp` authenticates and bounds the HTTPS request, then forwards it to the
private daemon socket. It does not implement NMP, synthesis, Blossom upload,
journaling, signing, retry, or answer observation. Do not recreate those
capabilities in an MCP client.

The resource server requires OAuth bearer tokens in the `Authorization` header,
an exact issuer and audience, the configured publish scope, and an asymmetric
JWK selected by `kid`. It rejects query-string access tokens and symmetric
JWKs. Tokens and claims must never enter logs or responses.

## Deployment

Start `tts29d` first. Configure `tts29-mcp` with its private socket, HTTPS
certificate and key, allowed hosts and browser origins, authorization server,
public JWKS, exact resource/audience, publish scope, and hard request bounds.

Run the repository binary through its supported config:

```bash
cargo run --manifest-path <skill-dir>/mcp/Cargo.toml --bin tts29-mcp -- \
  --config <private-mcp-config.json>
```

Keep TLS through the public hop and any proxy-to-adapter hop. Keep the daemon
socket and state inaccessible to the public listener account except for the
deliberately granted socket access. For the full protocol and OAuth shape, read
`<skill-dir>/docs/remote-producer.md`.
