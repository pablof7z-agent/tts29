use serde::{Deserialize, Serialize};
use tts29_protocol::{DurableArtifact, FrozenSpokenItem, Question};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProducerRequest {
    pub request_id: String,
    pub group_id: String,
    pub agent_name: String,
    pub subject: String,
    pub summary: String,
    pub body: String,
    pub attachments: Vec<DurableArtifact>,
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocalAudioArtifact {
    pub path: String,
    pub sha256: String,
    pub media_type: String,
    pub byte_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum JobPhase {
    Admitted,
    Synthesized {
        audio: LocalAudioArtifact,
    },
    ArtifactsDurable {
        item: FrozenSpokenItem,
    },
    PublicationAccepted {
        item: FrozenSpokenItem,
        receipt_id: u64,
    },
    Published {
        item: FrozenSpokenItem,
        receipt_id: u64,
        event_id: String,
    },
}

impl JobPhase {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Synthesized { .. } => "synthesized",
            Self::ArtifactsDurable { .. } => "artifacts_durable",
            Self::PublicationAccepted { .. } => "publication_accepted",
            Self::Published { .. } => "published",
        }
    }

    pub fn is_published(&self) -> bool {
        matches!(self, Self::Published { .. })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct JobRecord {
    pub schema_version: u16,
    pub request_digest: String,
    pub author: String,
    pub created_at: u64,
    pub request: ProducerRequest,
    pub phase: JobPhase,
}
