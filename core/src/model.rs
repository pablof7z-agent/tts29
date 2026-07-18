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
        }
    }

    pub fn failed(configuration: &KernelConfiguration, error: impl Into<String>) -> Self {
        let mut snapshot = Self::lifecycle(configuration, KernelPhase::Failed);
        snapshot.error = Some(error.into());
        snapshot
    }
}
