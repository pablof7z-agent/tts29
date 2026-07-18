use std::sync::Arc;

use axum::middleware;
use axum::routing::get;
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::ingress::{protect, IngressState};
use crate::{AuthValidator, McpConfig, SpeechTool};

#[derive(Clone, Serialize)]
struct ProtectedResourceMetadata {
    resource: String,
    authorization_servers: Vec<String>,
    scopes_supported: Vec<String>,
    bearer_methods_supported: Vec<&'static str>,
    resource_name: &'static str,
}

pub fn build_router(
    config: &McpConfig,
    auth: AuthValidator,
    cancellation: CancellationToken,
) -> Router {
    let speech = SpeechTool::new(config.daemon_socket_path.clone(), config.execution_timeout);
    let mcp: StreamableHttpService<SpeechTool, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(speech.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_stateful_mode(false)
            .with_json_response(true)
            .with_sse_keep_alive(None)
            .with_allowed_hosts(config.allowed_hosts.clone())
            .with_allowed_origins(config.allowed_origins.clone())
            .with_cancellation_token(cancellation.child_token()),
    );
    let ingress = IngressState::new(
        auth,
        config.max_concurrency,
        config.execution_timeout,
        config.max_request_bytes,
        config.max_response_bytes,
        config.metadata_url.clone(),
        config.required_scope.clone(),
    );
    let protected = Router::new()
        .nest_service(&config.mcp_path, mcp)
        .route_layer(middleware::from_fn_with_state(ingress, protect));
    let metadata = ProtectedResourceMetadata {
        resource: config.resource.clone(),
        authorization_servers: config.authorization_servers.clone(),
        scopes_supported: vec![config.required_scope.clone()],
        bearer_methods_supported: vec!["header"],
        resource_name: "TTS29 speech publisher",
    };
    let metadata_handler = move || {
        let value = metadata.clone();
        async move { Json(value) }
    };
    let mut router = Router::new().route(&config.metadata_path, get(metadata_handler.clone()));
    if config.metadata_path != "/.well-known/oauth-protected-resource" {
        router = router.route(
            "/.well-known/oauth-protected-resource",
            get(metadata_handler),
        );
    }
    router.merge(protected)
}

pub async fn run_server(config: McpConfig) -> Result<(), String> {
    let auth = AuthValidator::from_jwks_file(
        &config.jwks_path,
        config.issuer.clone(),
        config.audience.clone(),
        config.required_scope.clone(),
    )?;
    let cancellation = CancellationToken::new();
    let router = build_router(&config, auth, cancellation.clone());
    let tls =
        RustlsConfig::from_pem_file(&config.tls_certificate_path, &config.tls_private_key_path)
            .await
            .map_err(|error| format!("TLS configuration could not be loaded: {error}"))?;
    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    let shutdown_token = cancellation.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_token.cancel();
        shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
    });
    axum_server::bind_rustls(config.bind, tls)
        .handle(handle)
        .serve(router.into_make_service())
        .await
        .map_err(|error| format!("HTTPS MCP endpoint failed: {error}"))
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = async {
                if let Some(signal) = terminate.as_mut() {
                    signal.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
