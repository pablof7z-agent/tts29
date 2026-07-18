use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tts29_contract::{DurableArtifact, Question};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerRequest {
    pub request_id: String,
    pub group_id: String,
    pub voice: String,
    pub agent_name: String,
    pub subject: String,
    pub summary: String,
    pub body: String,
    pub attachments: Vec<DurableArtifact>,
    pub questions: Vec<Question>,
}
