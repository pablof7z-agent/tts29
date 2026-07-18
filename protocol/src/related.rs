use std::collections::BTreeSet;

use nmp::{EventId, PublicKey, RelayUrl, Timestamp, WriteIntent};
use nmp_nip29::{compose_group_send, GroupTimelineEvidence};

use crate::parse::tags::{bounded, identifier};
use crate::{QuestionAnswer, VERSION};

#[derive(Clone, Debug, PartialEq)]
pub struct FrozenAnswer {
    pub author: String,
    pub created_at: u64,
    pub group_id: String,
    pub root_event_id: String,
    pub content: String,
    pub answers: Vec<QuestionAnswer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelatedComposeError {
    InvalidAuthor,
    InvalidGroup,
    InvalidRoot,
    InvalidContent,
    InvalidAnswer,
    InvalidTag,
}

impl std::fmt::Display for RelatedComposeError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidAuthor => "the answer author is not a public key",
            Self::InvalidGroup => "the answer group is invalid",
            Self::InvalidRoot => "the answer root is not an event id",
            Self::InvalidContent => "the answer content is invalid",
            Self::InvalidAnswer => "the answer values are invalid",
            Self::InvalidTag => "NMP refused a generated answer tag",
        };
        output.write_str(message)
    }
}

impl std::error::Error for RelatedComposeError {}

pub fn compose_answer(
    host: RelayUrl,
    answer: &FrozenAnswer,
) -> Result<WriteIntent, RelatedComposeError> {
    let author =
        PublicKey::parse(&answer.author).map_err(|_| RelatedComposeError::InvalidAuthor)?;
    bounded(&answer.group_id, 128).ok_or(RelatedComposeError::InvalidGroup)?;
    EventId::from_hex(&answer.root_event_id).map_err(|_| RelatedComposeError::InvalidRoot)?;
    bounded(&answer.content, 4_000).ok_or(RelatedComposeError::InvalidContent)?;
    validate_answers(&answer.answers)?;

    let mut tags = vec![
        row(["tts29", "answer", VERSION]),
        row(["e", &answer.root_event_id, "", "root"]),
    ];
    tags.extend(answer.answers.iter().map(|value| {
        let mut tag = vec!["answer".to_string(), value.question_id.clone()];
        tag.extend(value.values.clone());
        tag
    }));
    let mut intent = compose_group_send(
        host,
        &answer.group_id,
        author,
        Timestamp::from(answer.created_at),
        9,
        answer.content.clone(),
        tags,
        &GroupTimelineEvidence::none(),
    )
    .map_err(|_| RelatedComposeError::InvalidTag)?;
    intent.identity_override = Some(author);
    Ok(intent)
}

fn validate_answers(answers: &[QuestionAnswer]) -> Result<(), RelatedComposeError> {
    if answers.is_empty() || answers.len() > 3 {
        return Err(RelatedComposeError::InvalidAnswer);
    }
    let mut questions = BTreeSet::new();
    for answer in answers {
        if identifier(&answer.question_id).is_none()
            || !questions.insert(&answer.question_id)
            || answer.values.is_empty()
            || answer.values.len() > 8
            || answer
                .values
                .iter()
                .any(|value| bounded(value, 4_000).is_none())
        {
            return Err(RelatedComposeError::InvalidAnswer);
        }
    }
    Ok(())
}

fn row<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use nmp::{Kind, WritePayload, WriteRouting};

    use super::*;

    #[test]
    fn answer_uses_the_root_group_and_explicit_responder_identity() {
        let answer = fixture();
        let author = PublicKey::parse(&answer.author).unwrap();

        let intent = compose_answer(host(), &answer).unwrap();
        assert_eq!(intent.identity_override, Some(author));
        assert!(matches!(intent.routing, WriteRouting::PinnedHost(_)));
        let WritePayload::Unsigned(event) = intent.payload else {
            panic!("answers must be unsigned")
        };
        assert_eq!(event.kind, Kind::from(9u16));
        assert!(event.tags.iter().any(|tag| tag.as_slice() == ["h", "tts"]));
        assert!(event
            .tags
            .iter()
            .any(|tag| { tag.as_slice() == ["e", &"a".repeat(64), "", "root"] }));
        assert!(event
            .tags
            .iter()
            .any(|tag| tag.as_slice() == ["answer", "live-e2e", "confirmed"]));
    }

    #[test]
    fn duplicate_question_answers_fail_before_nmp_acceptance() {
        let mut answer = fixture();
        answer.answers.push(answer.answers[0].clone());

        assert!(matches!(
            compose_answer(host(), &answer),
            Err(RelatedComposeError::InvalidAnswer)
        ));
    }

    fn fixture() -> FrozenAnswer {
        FrozenAnswer {
            author: "1".repeat(64),
            created_at: 100,
            group_id: "tts".into(),
            root_event_id: "a".repeat(64),
            content: "Live E2E confirmed.".into(),
            answers: vec![QuestionAnswer {
                question_id: "live-e2e".into(),
                values: vec!["confirmed".into()],
            }],
        }
    }

    fn host() -> RelayUrl {
        RelayUrl::parse("wss://relay.example.com").unwrap()
    }
}
