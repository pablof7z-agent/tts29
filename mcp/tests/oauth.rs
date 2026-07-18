#![cfg(unix)]

mod support;

use std::time::Duration;

use serde_json::json;
use tempfile::TempDir;

use support::HttpsHarness;

#[tokio::test(flavor = "multi_thread")]
async fn oauth_failures_return_bounded_protected_resource_challenges() {
    let temporary = TempDir::new().unwrap();
    let harness = HttpsHarness::start(
        temporary.path().join("missing.sock"),
        2,
        Duration::from_secs(2),
    )
    .await;

    let missing = post(&harness, None, None).await;
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_challenge(&missing, "Bearer resource_metadata=");

    let invalid_value = "secret-token-marker";
    let invalid = post(&harness, Some(invalid_value.into()), None).await;
    assert_eq!(invalid.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_challenge(&invalid, "error=\"invalid_token\"");
    assert!(!invalid.text().await.unwrap().contains(invalid_value));

    let wrong_audience = harness
        .issuer
        .token("https://other.example.test/mcp", "tts29:publish");
    let response = post(&harness, Some(wrong_audience), None).await;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_challenge(&response, "error=\"invalid_token\"");

    let insufficient = harness.issuer.token(&harness.resource, "tts29:read");
    let response = post(&harness, Some(insufficient), None).await;
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    assert_challenge(&response, "error=\"insufficient_scope\"");
    assert_challenge(&response, "scope=\"tts29:publish\"");

    let query_token = harness
        .client
        .post(format!(
            "{}?access_token={}",
            harness.resource,
            harness.token()
        ))
        .bearer_auth(harness.token())
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&initialize())
        .send()
        .await
        .unwrap();
    assert_eq!(query_token.status(), reqwest::StatusCode::BAD_REQUEST);

    let bad_host = post(&harness, Some(harness.token()), Some("evil.example.test")).await;
    assert_eq!(bad_host.status(), reqwest::StatusCode::FORBIDDEN);

    harness.shutdown().await;
}

async fn post(
    harness: &HttpsHarness,
    token: Option<String>,
    host: Option<&str>,
) -> reqwest::Response {
    let mut request = harness
        .client
        .post(&harness.resource)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&initialize());
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(host) = host {
        request = request.header("host", host);
    }
    request.send().await.unwrap()
}

fn assert_challenge(response: &reqwest::Response, expected: &str) {
    let value = response
        .headers()
        .get("www-authenticate")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(value.contains(expected), "challenge was {value}");
}

fn initialize() -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "tts29-test", "version": "1" }
        }
    })
}
