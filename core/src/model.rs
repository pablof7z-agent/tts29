use serde::{Deserialize, Serialize};
use tts29_protocol::SpokenItem;

#[derive(Clone, Debug, Deserialize)]
pub struct KernelConfiguration {
    pub relay: String,
    pub group_id: String,
    pub store_path: Option<String>,
    pub viewer_pubkey: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelPhase {
    Starting,
    Listening,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityPhase {
    SignedOut,
    Saving,
    SignedIn,
    LoggingOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IdentitySnapshot {
    pub phase: IdentityPhase,
    pub pubkey: Option<String>,
    pub error: Option<String>,
}

impl IdentitySnapshot {
    pub fn signed_out() -> Self {
        Self {
            phase: IdentityPhase::SignedOut,
            pubkey: None,
            error: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialOperation {
    Store,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CredentialRequest {
    pub id: u64,
    pub operation: CredentialOperation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerSubmissionPhase {
    Sending,
    Published,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnswerSubmissionSnapshot {
    pub item_id: String,
    pub phase: AnswerSubmissionPhase,
    pub receipt_id: Option<u64>,
    pub event_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueueEvidence {
    pub source_count: usize,
    pub shortfall_count: usize,
    pub rejected_event_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueueSnapshot {
    pub phase: KernelPhase,
    pub relay: String,
    pub group_id: String,
    pub items: Vec<SpokenItem>,
    pub evidence: QueueEvidence,
    pub error: Option<String>,
    pub identity: IdentitySnapshot,
    pub credential_request: Option<CredentialRequest>,
    pub answer_submissions: Vec<AnswerSubmissionSnapshot>,
}

impl QueueSnapshot {
    pub fn lifecycle(configuration: &KernelConfiguration, phase: KernelPhase) -> Self {
        Self {
            phase,
            relay: configuration.relay.clone(),
            group_id: configuration.group_id.clone(),
            items: Vec::new(),
            evidence: QueueEvidence {
                source_count: 0,
                shortfall_count: 0,
                rejected_event_count: 0,
            },
            error: None,
            identity: IdentitySnapshot::signed_out(),
            credential_request: None,
            answer_submissions: Vec::new(),
        }
    }

    pub fn failed(configuration: &KernelConfiguration, error: impl Into<String>) -> Self {
        let mut snapshot = Self::lifecycle(configuration, KernelPhase::Failed);
        snapshot.error = Some(error.into());
        snapshot
    }
}
