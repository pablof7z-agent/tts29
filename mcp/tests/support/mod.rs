#![allow(dead_code)]

mod daemon;

#[allow(unused_imports)]
pub use daemon::FakeDaemon;

use std::collections::HashMap;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum_server::tls_rustls::RustlsConfig;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::SigningKey;
use http::{HeaderName, HeaderValue};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rand_core::OsRng;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use serde::Serialize;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use tts29_mcp::{build_router, AuthValidator, Clock, McpConfig};

pub const NOW: u64 = 2_000_000_000;
pub const ISSUER: &str = "https://auth.example.test";
pub const ORIGIN: &str = "https://assistant.example.test";
pub const SCOPE: &str = "tts29:publish";

pub struct FixedClock;

impl Clock for FixedClock {
    fn now_unix(&self) -> u64 {
        NOW
    }
}

pub struct TokenIssuer {
    signing: SigningKey,
    jwks: JwkSet,
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    aud: &'a str,
    sub: &'a str,
    scope: &'a str,
    exp: u64,
    nbf: u64,
}

impl TokenIssuer {
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        let x = URL_SAFE_NO_PAD.encode(signing.verifying_key().as_bytes());
        let jwks = serde_json::from_value(serde_json::json!({
            "keys": [{
                "kty": "OKP",
                "crv": "Ed25519",
                "alg": "EdDSA",
                "use": "sig",
                "kid": "runtime-test-key",
                "x": x
            }]
        }))
        .unwrap();
        Self { signing, jwks }
    }

    pub fn token(&self, audience: &str, scope: &str) -> String {
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some("runtime-test-key".into());
        let key = self.signing.to_pkcs8_der().unwrap();
        encode(
            &header,
            &Claims {
                iss: ISSUER,
                aud: audience,
                sub: "hosted-assistant",
                scope,
                exp: NOW + 300,
                nbf: NOW - 1,
            },
            &EncodingKey::from_ed_der(key.as_bytes()),
        )
        .unwrap()
    }
}

pub struct HttpsHarness {
    _temporary: TempDir,
    pub client: reqwest::Client,
    pub resource: String,
    pub config: McpConfig,
    pub issuer: TokenIssuer,
    handle: axum_server::Handle<std::net::SocketAddr>,
    cancellation: CancellationToken,
    task: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

impl HttpsHarness {
    pub async fn start(
        daemon_socket: PathBuf,
        max_concurrency: usize,
        execution_timeout: Duration,
    ) -> Self {
        let temporary = TempDir::new().unwrap();
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.pem();
        let key_pem = signing_key.serialize_pem();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let resource = format!("https://localhost:{port}/mcp");
        let config = config(
            temporary.path(),
            daemon_socket,
            &resource,
            max_concurrency,
            execution_timeout,
        );
        let issuer = TokenIssuer::generate();
        let auth = AuthValidator::new(
            issuer.jwks.clone(),
            ISSUER,
            resource.clone(),
            SCOPE,
            Arc::new(FixedClock),
        )
        .unwrap();
        let cancellation = CancellationToken::new();
        let router = build_router(&config, auth, cancellation.clone());
        let tls = RustlsConfig::from_pem(cert_pem.as_bytes().to_vec(), key_pem.into_bytes())
            .await
            .unwrap();
        let handle = axum_server::Handle::new();
        let task_handle = handle.clone();
        let task = tokio::spawn(async move {
            axum_server::from_tcp_rustls(listener, tls)
                .unwrap()
                .handle(task_handle)
                .serve(router.into_make_service())
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), handle.listening())
            .await
            .unwrap()
            .unwrap();
        let root = reqwest::Certificate::from_pem(cert_pem.as_bytes()).unwrap();
        let client = reqwest::Client::builder()
            .tls_backend_rustls()
            .tls_certs_only([root])
            .build()
            .unwrap();
        Self {
            _temporary: temporary,
            client,
            resource,
            config,
            issuer,
            handle,
            cancellation,
            task,
        }
    }

    pub fn token(&self) -> String {
        self.issuer.token(&self.resource, SCOPE)
    }

    pub fn client_headers(&self) -> HashMap<HeaderName, HeaderValue> {
        HashMap::from([(
            HeaderName::from_static("origin"),
            HeaderValue::from_static(ORIGIN),
        )])
    }

    pub async fn shutdown(self) {
        self.cancellation.cancel();
        self.handle.graceful_shutdown(Some(Duration::from_secs(2)));
        tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

fn config(
    base: &Path,
    daemon_socket_path: PathBuf,
    resource: &str,
    max_concurrency: usize,
    execution_timeout: Duration,
) -> McpConfig {
    let port = url::Url::parse(resource).unwrap().port().unwrap();
    McpConfig {
        bind: format!("127.0.0.1:{port}").parse().unwrap(),
        resource: resource.into(),
        mcp_path: "/mcp".into(),
        metadata_path: "/.well-known/oauth-protected-resource/mcp".into(),
        metadata_url: format!("https://localhost:{port}/.well-known/oauth-protected-resource/mcp"),
        authorization_servers: vec![ISSUER.into()],
        issuer: ISSUER.into(),
        audience: resource.into(),
        required_scope: SCOPE.into(),
        jwks_path: base.join("unused-jwks.json"),
        daemon_socket_path,
        tls_certificate_path: base.join("unused-cert.pem"),
        tls_private_key_path: base.join("unused-key.pem"),
        allowed_hosts: vec![format!("localhost:{port}")],
        allowed_origins: vec![ORIGIN.into()],
        max_request_bytes: 128 * 1024,
        max_response_bytes: 64 * 1024,
        max_concurrency,
        execution_timeout,
    }
}
