use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tts29_contract::{AttachLink, DurableArtifact, Question};

/// A single spoken item to synthesize and publish. When `attach` is set the
/// item is published as a narrated child of another item. This is the per-node
/// unit the runner journals; the agent-facing tree is `SpokenTree`.
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
    #[serde(default)]
    pub attach: Option<AttachLink>,
}

/// The agent-facing spoken tree: a root message plus file and narrated
/// attachments, every payload a local file path. The daemon reads the files,
/// synthesizes, uploads, and publishes each node — the agent supplies no voice
/// and no signing identity.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenTree {
    pub request_id: String,
    pub group_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    /// Path to the spoken message file (its `[label](attachment:)` links
    /// reference this node's own attachments).
    pub message: String,
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub attachments: Vec<TreeAttachment>,
}

/// An attachment on a tree node: a binary file uploaded as-is, or a narrated
/// child that is itself synthesized and published as a linked item.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
pub enum TreeAttachment {
    File {
        label: String,
        file: String,
    },
    Narrated {
        /// Human-readable label; it is also the child's title.
        label: String,
        message: String,
        #[serde(default)]
        questions: Vec<Question>,
        #[serde(default)]
        attachments: Vec<TreeAttachment>,
    },
}
