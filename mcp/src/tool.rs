use std::path::PathBuf;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tts29_producer_api::{
    submit_local_with_timeout, LocalPublishRequest, LocalPublishResponse, ProducerRequest,
    LOCAL_PROTOCOL_VERSION,
};

#[derive(Clone)]
pub struct SpeechTool {
    socket_path: PathBuf,
    timeout: Duration,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishSpeechInput {
    pub request: ProducerRequest,
    #[serde(default)]
    pub wait_for_answer_seconds: Option<u64>,
}

impl SpeechTool {
    pub fn new(socket_path: PathBuf, timeout: Duration) -> Self {
        Self {
            socket_path,
            timeout,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl SpeechTool {
    #[tool(
        name = "publish_speech",
        description = "Generate and durably publish one complete spoken item to the TTS29 queue"
    )]
    async fn publish_speech(
        &self,
        Parameters(input): Parameters<PublishSpeechInput>,
    ) -> CallToolResult {
        let local = LocalPublishRequest {
            version: LOCAL_PROTOCOL_VERSION,
            request: input.request,
            wait_for_answer_seconds: input.wait_for_answer_seconds,
            agent_nsec: None,
        };
        if let Err(error) = local.validate() {
            return tool_error(LocalPublishResponse::error(error.code, error.message));
        }
        let socket = self.socket_path.clone();
        let timeout = self.timeout;
        let response = match tokio::task::spawn_blocking(move || {
            submit_local_with_timeout(socket, &local, timeout)
        })
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(_)) | Err(_) => LocalPublishResponse::error(
                "daemon_unavailable",
                "the private TTS29 producer is unavailable",
            ),
        };
        match response {
            response @ LocalPublishResponse::Published { .. } => tool_success(response),
            LocalPublishResponse::Error { version, code, .. } => {
                tool_error(LocalPublishResponse::Error {
                    version,
                    message: public_error_message(&code).into(),
                    code,
                })
            }
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SpeechTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Publish complete spoken items; playback remains client-owned.")
    }
}

fn tool_success(response: LocalPublishResponse) -> CallToolResult {
    let text = match &response {
        LocalPublishResponse::Published {
            request_id,
            receipt_id,
            event_id,
            ..
        } => format!(
            "Published TTS29 request {request_id} as event {event_id} with receipt {receipt_id}."
        ),
        LocalPublishResponse::Error { .. } => unreachable!(),
    };
    structured(response, text, false)
}

fn tool_error(response: LocalPublishResponse) -> CallToolResult {
    let text = match &response {
        LocalPublishResponse::Error { message, .. } => message.clone(),
        LocalPublishResponse::Published { .. } => unreachable!(),
    };
    structured(response, text, true)
}

fn structured(response: LocalPublishResponse, text: String, error: bool) -> CallToolResult {
    let value = serde_json::to_value(response).unwrap_or(Value::Null);
    let mut result = if error {
        CallToolResult::structured_error(value)
    } else {
        CallToolResult::structured(value)
    };
    result.content = vec![ContentBlock::text(text)];
    result
}

fn public_error_message(code: &str) -> &'static str {
    match code {
        "invalid_request" | "malformed_request" | "unsupported_version" | "invalid_answer_wait" => {
            "the speech publication request is invalid"
        }
        "request_conflict" => "the request ID was already used for different content",
        "request_too_large" => "the speech publication request is too large",
        "daemon_unavailable" => "the private TTS29 producer is unavailable",
        _ => "the private TTS29 producer could not complete publication",
    }
}
