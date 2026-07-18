#![cfg(unix)]

mod support;

use std::time::Duration;

use rmcp::model::{CallToolRequestParams, ClientInfo};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use serde_json::json;
use tempfile::TempDir;
use tts29_producer_api::{AnswerWaitResult, LocalPublishResponse, LOCAL_PROTOCOL_VERSION};

use support::{FakeDaemon, HttpsHarness, ORIGIN};

#[tokio::test(flavor = "multi_thread")]
async fn official_client_discovers_and_publishes_over_authenticated_https() {
    let temporary = TempDir::new().unwrap();
    let socket = temporary.path().join("daemon.sock");
    let daemon = FakeDaemon::start(socket.clone());
    let harness = HttpsHarness::start(socket, 4, Duration::from_secs(5)).await;

    let metadata: serde_json::Value = harness
        .client
        .get(&harness.config.metadata_url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(metadata["resource"], harness.resource);
    assert_eq!(metadata["scopes_supported"], json!(["tts29:publish"]));

    let transport = StreamableHttpClientTransport::with_client(
        harness.client.clone(),
        StreamableHttpClientTransportConfig::with_uri(harness.resource.clone())
            .auth_header(harness.token())
            .custom_headers(harness.client_headers()),
    );
    let client = ClientInfo::default().serve(transport).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "publish_speech");

    let malformed = client
        .call_tool(
            CallToolRequestParams::new("publish_speech")
                .with_arguments(serde_json::from_value(json!({})).unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(malformed.is_error, Some(true));

    let call = client.call_tool(
        CallToolRequestParams::new("publish_speech").with_arguments(
            serde_json::from_value(json!({
                "request": {
                    "request_id": "hosted-request-1",
                    "group_id": "tts",
                    "voice": "af_heart",
                    "agent_name": "Hosted Assistant",
                    "subject": "Deployment ready",
                    "summary": "The remote producer boundary is ready.",
                    "body": "The remote producer boundary is ready for review.",
                    "attachments": [],
                    "questions": [{
                        "id": "ship",
                        "kind": "single_choice",
                        "short_title": "Ship?",
                        "title": "Should this ship?",
                        "description": null,
                        "options": [{
                            "id": "yes",
                            "title": "Yes",
                            "description": null
                        }]
                    }]
                },
                "wait_for_answer_seconds": 1
            }))
            .unwrap(),
        ),
    );
    let daemon_flow = async {
        let request = daemon.received().await;
        assert_eq!(request.request.request_id, "hosted-request-1");
        assert_eq!(request.wait_for_answer_seconds, Some(1));
        assert!(request.agent_nsec.is_none());
        daemon
            .respond(LocalPublishResponse::Published {
                version: LOCAL_PROTOCOL_VERSION,
                request_id: request.request.request_id,
                receipt_id: 41,
                event_id: "a".repeat(64),
                answer_wait: AnswerWaitResult::TimedOut,
            })
            .await;
    };
    let (result, ()) = tokio::join!(call, daemon_flow);
    let result = result.unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result.structured_content.as_ref().unwrap()["receipt_id"],
        41
    );
    assert_eq!(
        result.structured_content.as_ref().unwrap()["answer_wait"]["status"],
        "timed_out"
    );
    assert!(format!("{:?}", result.content).contains("Published TTS29 request"));

    client.cancel().await.unwrap();
    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_origin_is_rejected_before_mcp_dispatch() {
    let temporary = TempDir::new().unwrap();
    let harness = HttpsHarness::start(
        temporary.path().join("missing.sock"),
        2,
        Duration::from_secs(2),
    )
    .await;
    let response = harness
        .client
        .post(&harness.resource)
        .bearer_auth(harness.token())
        .header("origin", format!("{ORIGIN}.invalid"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&initialize())
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    harness.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn request_bytes_and_concurrent_work_are_refused_at_the_ingress() {
    let temporary = TempDir::new().unwrap();
    let socket = temporary.path().join("daemon.sock");
    let daemon = FakeDaemon::start(socket.clone());
    let harness = HttpsHarness::start(socket, 1, Duration::from_secs(5)).await;

    let oversized = harness
        .client
        .post(&harness.resource)
        .bearer_auth(harness.token())
        .header("content-type", "application/json")
        .body(vec![b'x'; harness.config.max_request_bytes + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(oversized.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);

    let transport = StreamableHttpClientTransport::with_client(
        harness.client.clone(),
        StreamableHttpClientTransportConfig::with_uri(harness.resource.clone())
            .auth_header(harness.token())
            .custom_headers(harness.client_headers()),
    );
    let client = ClientInfo::default().serve(transport).await.unwrap();
    let call = client.call_tool(publish_call("concurrent-request"));
    let daemon_flow = async {
        let request = daemon.received().await;
        let busy = harness
            .client
            .post(&harness.resource)
            .bearer_auth(harness.token())
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&initialize())
            .send()
            .await
            .unwrap();
        assert_eq!(busy.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
        daemon
            .respond(LocalPublishResponse::Published {
                version: LOCAL_PROTOCOL_VERSION,
                request_id: request.request.request_id,
                receipt_id: 42,
                event_id: "b".repeat(64),
                answer_wait: AnswerWaitResult::NotRequested,
            })
            .await;
    };
    let (result, ()) = tokio::join!(call, daemon_flow);
    assert_eq!(result.unwrap().is_error, Some(false));

    client.cancel().await.unwrap();
    harness.shutdown().await;
}

fn publish_call(request_id: &str) -> CallToolRequestParams {
    CallToolRequestParams::new("publish_speech").with_arguments(
        serde_json::from_value(json!({
            "request": {
                "request_id": request_id,
                "group_id": "tts",
                "voice": "af_heart",
                "agent_name": "Hosted Assistant",
                "subject": "Admission proof",
                "summary": "Admission remains bounded.",
                "body": "Admission remains bounded.",
                "attachments": [],
                "questions": []
            }
        }))
        .unwrap(),
    )
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
