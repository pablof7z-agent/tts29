# Hosted producer over HTTPS MCP

`tts29-mcp` is the public ingress for hosted assistants. It is a separate
process from `tts29d` and depends only on the data-only TTS29 contract and the
private producer client. It does not contain or initialize NMP, Kokoro,
Blossom, journals, signers, receipt recovery, or answer observations.

```text
MCP client
  | HTTPS + OAuth bearer token
  v
tts29-mcp -- Host/Origin/auth/admission bounds
  | versioned JSON on a private Unix socket
  v
tts29d -- synthesis/artifacts/NMP/recovery/answer wait
```

The adapter implements stable MCP `2025-11-25` Streamable HTTP in stateless
JSON-response mode. It exposes one tool, `publish_speech`. The tool accepts a
`ProducerRequest` plus an optional bounded answer wait and returns the same
versioned `LocalPublishResponse` shape as the local CLI in both human-readable
content and `structuredContent`. Remote callers cannot supply `AGENT_NSEC`.

## Authorization boundary

The endpoint is an OAuth protected resource, not an authorization server. The
configured external authorization server issues JWT access tokens. TTS29:

- publishes RFC 9728 metadata at the path-specific
  `/.well-known/oauth-protected-resource/<mcp-path>` location and at the root
  compatibility location;
- accepts bearer tokens only in the `Authorization` header and rejects
  `access_token` query parameters;
- verifies an asymmetric JWK selected by a required `kid`;
- validates the exact issuer and resource audience, expiry/not-before, and the
  configured publish scope; and
- returns a protected-resource `WWW-Authenticate` challenge with `401` for an
  invalid token and `403` for insufficient scope.

The configured JWKS file contains public verification keys only. Symmetric JWKs
are refused because they would put authorization-server signing secrets on the
resource server. Replace the file atomically and restart the adapter when the
authorization server rotates keys. Tokens and claims are never logged or
returned.

## Run the endpoint

Start `tts29d` first using the private-socket instructions in
[local-producer.md](local-producer.md). Copy `mcp/example-config.json`, install
the authorization server's public JWKS, and point the configuration at a TLS
certificate and private key readable only by the adapter account.

```bash
cargo run --manifest-path mcp/Cargo.toml --bin tts29-mcp -- \
  --config mcp/example-config.json
```

The configured `resource` and `audience` must be the same HTTPS MCP endpoint.
The `issuer` must appear in `authorization_servers`. Hosts and browser origins
are explicit allowlists; a request with no browser `Origin` remains valid for
ordinary MCP clients, while a present but unlisted origin is rejected.

Request bytes, response bytes, concurrent work, and total execution time have
hard limits. Admission over capacity returns `503`; total execution timeout
returns `504`. The private socket also receives the same I/O deadline. A caller
that loses a response retries with the same stable `request_id`, allowing the
daemon journal to resume or return the existing publication evidence without
creating a second spoken item.

TLS terminates in `tts29-mcp`. If an operator places another proxy in front,
the adapter must remain on a private interface and the proxy-to-adapter hop must
preserve the configured Host and use TLS. The Unix socket and daemon state
directory must remain inaccessible to the public listener account except for
the exact socket access deliberately granted by the deployment.
