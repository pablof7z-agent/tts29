use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct DurableArtifact {
    pub url: String,
    pub sha256: String,
    pub media_type: String,
    pub byte_count: u64,
    pub label: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    SingleChoice,
    MultipleChoice,
    Freeform,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct QuestionOption {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct Question {
    pub id: String,
    pub kind: QuestionKind,
    pub short_title: String,
    pub title: String,
    pub description: Option<String>,
    pub options: Vec<QuestionOption>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct QuestionAnswer {
    pub question_id: String,
    pub values: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct AnswerBundle {
    pub event_id: String,
    pub author: String,
    pub created_at: u64,
    pub answers: Vec<QuestionAnswer>,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcknowledgementState {
    Active,
    Heard,
    Dismissed,
    Archived,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct Acknowledgement {
    pub event_id: String,
    pub author: String,
    pub created_at: u64,
    pub state: AcknowledgementState,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct ReactionSummary {
    pub emoji: String,
    pub count: usize,
    pub authors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct SpokenItem {
    pub id: String,
    pub author: String,
    pub created_at: u64,
    pub agent_name: String,
    pub subject: String,
    pub summary: String,
    pub body: String,
    pub audio_url: Option<String>,
    pub audio: DurableArtifact,
    pub attachments: Vec<DurableArtifact>,
    pub questions: Vec<Question>,
    pub answer: Option<AnswerBundle>,
    pub acknowledgement: Option<Acknowledgement>,
    pub reactions: Vec<ReactionSummary>,
}
