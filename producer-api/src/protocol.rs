use serde::{Deserialize, Serialize};
use tts29_contract::AnswerBundle;

use crate::{ProducerRequest, SpokenTree};

pub const LOCAL_PROTOCOL_VERSION: u16 = 1;
pub const MAX_LOCAL_FRAME_BYTES: usize = 128 * 1024;
pub const MAX_ANSWER_WAIT_SECONDS: u64 = 300;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalPublishRequest {
    pub version: u16,
    pub request: ProducerRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_for_answer_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_nsec: Option<String>,
}

impl LocalPublishRequest {
    pub fn validate(&self) -> Result<(), LocalRequestError> {
        if self.version != LOCAL_PROTOCOL_VERSION {
            return Err(LocalRequestError::new(
                "unsupported_version",
                format!(
                    "local protocol version {} is unsupported; expected {}",
                    self.version, LOCAL_PROTOCOL_VERSION
                ),
            ));
        }
        if let Some(seconds) = self.wait_for_answer_seconds {
            if seconds == 0 || seconds > MAX_ANSWER_WAIT_SECONDS {
                return Err(LocalRequestError::new(
                    "invalid_answer_wait",
                    format!("answer wait must be between 1 and {MAX_ANSWER_WAIT_SECONDS} seconds"),
                ));
            }
            if self.request.questions.is_empty() {
                return Err(LocalRequestError::new(
                    "invalid_answer_wait",
                    "answer wait requires at least one question",
                ));
            }
        }
        if self
            .agent_nsec
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 128)
        {
            return Err(LocalRequestError::new(
                "invalid_agent_identity",
                "request identity has an invalid encoded length",
            ));
        }
        Ok(())
    }
}

/// Submits a whole spoken tree of file paths. Attribution comes from
/// `agent_id` (optional; absent means the signer's pubkey is the identity) and
/// signing from `agent_nsec` (optional; absent means the daemon signs).
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalTreeRequest {
    pub version: u16,
    pub tree: SpokenTree,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_nsec: Option<String>,
}

impl LocalTreeRequest {
    pub fn validate(&self) -> Result<(), LocalRequestError> {
        if self.version != LOCAL_PROTOCOL_VERSION {
            return Err(LocalRequestError::new(
                "unsupported_version",
                format!(
                    "local protocol version {} is unsupported; expected {}",
                    self.version, LOCAL_PROTOCOL_VERSION
                ),
            ));
        }
        if self
            .agent_id
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 80)
        {
            return Err(LocalRequestError::new(
                "invalid_agent_identity",
                "agent id has an invalid length",
            ));
        }
        if self
            .agent_nsec
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 128)
        {
            return Err(LocalRequestError::new(
                "invalid_agent_identity",
                "request identity has an invalid encoded length",
            ));
        }
        Ok(())
    }
}

pub struct LocalRequestError {
    pub code: &'static str,
    pub message: String,
}

impl LocalRequestError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LocalPublishResponse {
    Published {
        version: u16,
        request_id: String,
        receipt_id: u64,
        event_id: String,
        answer_wait: AnswerWaitResult,
    },
    PublishedTree {
        version: u16,
        request_id: String,
        root_event_id: String,
        child_event_ids: Vec<String>,
    },
    Error {
        version: u16,
        code: String,
        message: String,
    },
}

impl LocalPublishResponse {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            version: LOCAL_PROTOCOL_VERSION,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AnswerWaitResult {
    NotRequested,
    Answered { answer: AnswerBundle },
    TimedOut,
    Unavailable { code: String, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_model_has_no_request_identity_field() {
        let response = LocalPublishResponse::Published {
            version: LOCAL_PROTOCOL_VERSION,
            request_id: "request-1".into(),
            receipt_id: 7,
            event_id: "a".repeat(64),
            answer_wait: AnswerWaitResult::NotRequested,
        };

        let encoded = serde_json::to_string(&response).unwrap();

        assert!(!encoded.contains("nsec"));
        assert!(!encoded.contains("agent_identity"));
    }
}
