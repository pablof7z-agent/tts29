use std::collections::BTreeSet;

use nmp::{PublicKey, RelayUrl, Timestamp, WriteIntent};
use nmp_nip29::{compose_group_send, GroupTimelineEvidence};
use serde::{Deserialize, Serialize};

use crate::model::{DurableArtifact, Question, QuestionKind};
use crate::parse::tags::{artifact, bounded, identifier};
use crate::parse::VERSION;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FrozenSpokenItem {
    pub author: String,
    pub created_at: u64,
    pub group_id: String,
    pub agent_name: String,
    pub subject: String,
    pub summary: String,
    pub body: String,
    pub audio: DurableArtifact,
    pub attachments: Vec<DurableArtifact>,
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeError {
    InvalidAuthor,
    InvalidField(&'static str),
    InvalidTag,
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAuthor => write!(output, "the frozen author is not a public key"),
            Self::InvalidField(field) => write!(output, "invalid frozen field: {field}"),
            Self::InvalidTag => write!(output, "NMP refused a generated protocol tag"),
        }
    }
}

impl std::error::Error for ComposeError {}

pub fn compose_spoken_item(
    host: RelayUrl,
    item: &FrozenSpokenItem,
) -> Result<WriteIntent, ComposeError> {
    validate_spoken_item(item)?;
    let author = PublicKey::parse(&item.author).map_err(|_| ComposeError::InvalidAuthor)?;
    let mut intent = compose_group_send(
        host,
        &item.group_id,
        author,
        Timestamp::from(item.created_at),
        9,
        item.body.clone(),
        tags(item),
        &GroupTimelineEvidence::none(),
    )
    .map_err(|_| ComposeError::InvalidTag)?;
    intent.identity_override = Some(author);
    Ok(intent)
}

pub fn validate_spoken_item(item: &FrozenSpokenItem) -> Result<(), ComposeError> {
    PublicKey::parse(&item.author).map_err(|_| ComposeError::InvalidAuthor)?;
    if bounded(&item.group_id, 128).is_none() {
        return Err(ComposeError::InvalidField("group_id"));
    }
    for (name, value, max) in [
        ("agent_name", item.agent_name.as_str(), 80),
        ("subject", item.subject.as_str(), 80),
        ("summary", item.summary.as_str(), 280),
        ("body", item.body.as_str(), 40_000),
    ] {
        if bounded(value, max).is_none() {
            return Err(ComposeError::InvalidField(name));
        }
    }
    if !valid_artifact(&item.audio, false) || !item.audio.media_type.starts_with("audio/") {
        return Err(ComposeError::InvalidField("audio"));
    }
    if item.attachments.len() > 12
        || item
            .attachments
            .iter()
            .any(|value| !valid_artifact(value, true))
    {
        return Err(ComposeError::InvalidField("attachments"));
    }
    validate_questions(&item.questions)
}

fn validate_questions(questions: &[Question]) -> Result<(), ComposeError> {
    if questions.len() > 3 {
        return Err(ComposeError::InvalidField("questions"));
    }
    let mut question_ids = BTreeSet::new();
    for question in questions {
        if identifier(&question.id).is_none()
            || !question_ids.insert(&question.id)
            || bounded(&question.title, 240).is_none()
            || bounded(&question.short_title, 40).is_none()
            || question
                .description
                .as_deref()
                .is_some_and(|value| bounded(value, 500).is_none())
        {
            return Err(ComposeError::InvalidField("question"));
        }
        let choices = matches!(
            question.kind,
            QuestionKind::SingleChoice | QuestionKind::MultipleChoice
        );
        if choices == question.options.is_empty() || question.options.len() > 8 {
            return Err(ComposeError::InvalidField("question_options"));
        }
        let mut option_ids = BTreeSet::new();
        for option in &question.options {
            if identifier(&option.id).is_none()
                || !option_ids.insert(&option.id)
                || bounded(&option.title, 120).is_none()
                || option
                    .description
                    .as_deref()
                    .is_some_and(|value| bounded(value, 300).is_none())
            {
                return Err(ComposeError::InvalidField("question_option"));
            }
        }
    }
    Ok(())
}

fn valid_artifact(value: &DurableArtifact, with_label: bool) -> bool {
    artifact(
        &artifact_row(value, if with_label { "attachment" } else { "audio" }),
        if with_label { "attachment" } else { "audio" },
        with_label,
    )
    .is_some()
}

fn tags(item: &FrozenSpokenItem) -> Vec<Vec<String>> {
    let mut result = vec![
        row(["tts29", "item", VERSION]),
        row(["title", &item.subject]),
        row(["summary", &item.summary]),
        row(["agent", &item.agent_name]),
        artifact_row(&item.audio, "audio"),
    ];
    result.extend(
        item.attachments
            .iter()
            .map(|value| artifact_row(value, "attachment")),
    );
    for question in &item.questions {
        let kind = match question.kind {
            QuestionKind::SingleChoice => "single",
            QuestionKind::MultipleChoice => "multiple",
            QuestionKind::Freeform => "freeform",
        };
        result.push(row(["question", &question.id, kind, &question.title]));
        result.push(row(["label", &question.id, &question.short_title]));
        if let Some(description) = &question.description {
            result.push(row(["description", &question.id, description]));
        }
        for option in &question.options {
            let mut value = row(["option", &question.id, &option.id, &option.title]);
            if let Some(description) = &option.description {
                value.push(description.clone());
            }
            result.push(value);
        }
    }
    result
}

fn artifact_row(value: &DurableArtifact, name: &str) -> Vec<String> {
    let mut result = vec![
        name.to_string(),
        value.url.clone(),
        value.sha256.clone(),
        value.media_type.clone(),
        value.byte_count.to_string(),
    ];
    if let Some(label) = &value.label {
        result.push(label.clone());
    }
    result
}

fn row<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use nmp::{RelayUrl, WritePayload};

    use super::*;

    #[test]
    fn frozen_item_retries_have_the_same_event_id() {
        let item = fixture();
        let first = compose_spoken_item(host(), &item).unwrap();
        let second = compose_spoken_item(host(), &item).unwrap();
        let mut first = unsigned(first).clone();
        let mut second = unsigned(second).clone();

        assert_eq!(first.id(), second.id());
        assert!(first.tags.iter().any(|tag| tag.as_slice() == ["h", "tts"]));
        assert!(first
            .tags
            .iter()
            .any(|tag| tag.as_slice() == ["tts29", "item", "1"]));

        let mut later = item;
        later.created_at += 1;
        let mut later = unsigned(compose_spoken_item(host(), &later).unwrap()).clone();
        assert_ne!(first.id(), later.id());
    }

    #[test]
    fn frozen_author_is_the_explicit_nmp_write_identity() {
        let item = fixture();
        let author = PublicKey::parse(&item.author).unwrap();

        let intent = compose_spoken_item(host(), &item).unwrap();

        assert_eq!(intent.identity_override, Some(author));
    }

    #[test]
    fn incomplete_artifact_fails_before_nmp_acceptance() {
        let mut item = fixture();
        item.audio.sha256 = "not-a-digest".into();

        let error = match compose_spoken_item(host(), &item) {
            Err(error) => error,
            Ok(_) => panic!("an invalid artifact must not compose"),
        };
        assert_eq!(error, ComposeError::InvalidField("audio"));
    }

    fn fixture() -> FrozenSpokenItem {
        FrozenSpokenItem {
            author: "1".repeat(64),
            created_at: 100,
            group_id: "tts".into(),
            agent_name: "Codex".into(),
            subject: "Build ready".into(),
            summary: "The build is ready.".into(),
            body: "The simulator build is ready.".into(),
            audio: DurableArtifact {
                url: "https://cdn.example/audio.mp3".into(),
                sha256: "a".repeat(64),
                media_type: "audio/mpeg".into(),
                byte_count: 1200,
                label: None,
            },
            attachments: Vec::new(),
            questions: Vec::new(),
        }
    }

    fn host() -> RelayUrl {
        RelayUrl::parse("wss://relay.example.com").unwrap()
    }

    fn unsigned(intent: WriteIntent) -> nmp::UnsignedEvent {
        let WritePayload::Unsigned(unsigned) = intent.payload else {
            panic!("spoken items must be unsigned")
        };
        unsigned
    }
}
