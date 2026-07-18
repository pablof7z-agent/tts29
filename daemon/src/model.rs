use serde::{Deserialize, Serialize};
use tts29_producer_api::ProducerRequest;
use tts29_protocol::FrozenSpokenItem;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocalAudioArtifact {
    pub path: String,
    pub sha256: String,
    pub media_type: String,
    pub byte_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MembershipEvidence {
    pub event_id: String,
    pub receipt_id: Option<u64>,
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
    AuthorizationAccepted {
        item: FrozenSpokenItem,
        receipt_id: u64,
    },
    AuthorAuthorized {
        item: FrozenSpokenItem,
        membership: MembershipEvidence,
    },
    PublicationAccepted {
        item: FrozenSpokenItem,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        membership: Option<MembershipEvidence>,
        receipt_id: u64,
    },
    Published {
        item: FrozenSpokenItem,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        membership: Option<MembershipEvidence>,
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
            Self::AuthorizationAccepted { .. } => "authorization_accepted",
            Self::AuthorAuthorized { .. } => "author_authorized",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_signer_stops_before_the_durable_job_model() {
        let secret = "nsec1request-boundary";
        let local = crate::LocalPublishRequest {
            version: crate::LOCAL_PROTOCOL_VERSION,
            request: request(),
            wait_for_answer_seconds: None,
            agent_nsec: Some(secret.into()),
        };
        assert!(serde_json::to_string(&local).unwrap().contains(secret));
        let job = JobRecord {
            schema_version: 1,
            request_digest: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1,
            request: local.request,
            phase: JobPhase::Admitted,
        };

        let journal_record = serde_json::to_string(&job).unwrap();

        assert!(!journal_record.contains(secret));
        assert!(!journal_record.contains("agent_nsec"));
    }

    fn request() -> ProducerRequest {
        ProducerRequest {
            request_id: "secret-boundary".into(),
            group_id: "tts".into(),
            voice: "af_heart".into(),
            agent_name: "Codex".into(),
            subject: "Secret boundary".into(),
            summary: "Only the author survives admission.".into(),
            body: "Do not persist the request signer.".into(),
            attachments: Vec::new(),
            questions: Vec::new(),
        }
    }
}
