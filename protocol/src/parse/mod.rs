pub(crate) mod tags;

use std::collections::BTreeSet;

use nmp::Row;

use crate::{Acknowledgement, AcknowledgementState, AnswerBundle, QuestionAnswer, SpokenItem};
use tags::{
    artifact, bounded, group_matches, identifier, marker, optional, root_event, rows, unique,
};

pub const VERSION: &str = "1";

#[derive(Clone, Debug)]
pub enum ParsedEvent {
    Item(Box<SpokenItem>),
    Answer(Related<AnswerBundle>),
    Acknowledgement(Related<Acknowledgement>),
    Reaction(Related<Reaction>),
}

#[derive(Clone, Debug)]
pub struct Related<T> {
    pub root_id: String,
    pub value: T,
}

#[derive(Clone, Debug)]
pub struct Reaction {
    pub event_id: String,
    pub author: String,
    pub created_at: u64,
    pub emoji: String,
    pub active: bool,
}

pub fn parse(row: &Row, group_id: &str) -> Option<ParsedEvent> {
    if row.event.kind.as_u16() != 9 || !group_matches(row, group_id) {
        return None;
    }
    let marker = marker(row)?;
    match (marker.0.as_str(), marker.1.as_str()) {
        ("item", VERSION) => parse_item(row).map(Box::new).map(ParsedEvent::Item),
        ("answer", VERSION) => parse_answer(row).map(ParsedEvent::Answer),
        ("ack", VERSION) => parse_acknowledgement(row).map(ParsedEvent::Acknowledgement),
        ("reaction", VERSION) => parse_reaction(row).map(ParsedEvent::Reaction),
        _ => None,
    }
}

fn parse_item(row: &Row) -> Option<SpokenItem> {
    let audio_rows = rows(row, "audio");
    if audio_rows.len() != 1 {
        return None;
    }
    let audio = artifact(&audio_rows[0], "audio", false)?;
    if !audio.media_type.starts_with("audio/") {
        return None;
    }
    let attachment_rows = rows(row, "attachment");
    if attachment_rows.len() > 12 {
        return None;
    }
    let attachments = attachment_rows
        .iter()
        .map(|value| artifact(value, "attachment", true))
        .collect::<Option<Vec<_>>>()?;
    if attachments.iter().any(|value| value.label.is_none()) {
        return None;
    }

    Some(SpokenItem {
        id: row.event.id.to_hex(),
        author: row.event.pubkey.to_hex(),
        created_at: row.event.created_at.as_secs(),
        agent_name: unique(row, "agent", 80)?,
        subject: unique(row, "title", 80)?,
        summary: unique(row, "summary", 280)?,
        body: bounded(&row.event.content, 40_000)?,
        audio_url: Some(audio.url.clone()),
        audio,
        attachments,
        questions: tags::questions(row)?,
        answer: None,
        acknowledgement: None,
        reactions: Vec::new(),
    })
}

fn parse_answer(row: &Row) -> Option<Related<AnswerBundle>> {
    let answer_rows = rows(row, "answer");
    if answer_rows.is_empty() || answer_rows.len() > 3 {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut answers = Vec::new();
    for value in answer_rows {
        if value.len() < 3 || value.len() > 10 {
            return None;
        }
        let question_id = identifier(&value[1])?;
        if !seen.insert(question_id.clone()) {
            return None;
        }
        let values = value[2..]
            .iter()
            .map(|item| bounded(item, 4_000))
            .collect::<Option<Vec<_>>>()?;
        answers.push(QuestionAnswer {
            question_id,
            values,
        });
    }
    Some(Related {
        root_id: root_event(row)?,
        value: AnswerBundle {
            event_id: row.event.id.to_hex(),
            author: row.event.pubkey.to_hex(),
            created_at: row.event.created_at.as_secs(),
            answers,
        },
    })
}

fn parse_acknowledgement(row: &Row) -> Option<Related<Acknowledgement>> {
    let state = match unique(row, "state", 16)?.as_str() {
        "active" => AcknowledgementState::Active,
        "heard" => AcknowledgementState::Heard,
        "dismissed" => AcknowledgementState::Dismissed,
        "archived" => AcknowledgementState::Archived,
        _ => return None,
    };
    Some(Related {
        root_id: root_event(row)?,
        value: Acknowledgement {
            event_id: row.event.id.to_hex(),
            author: row.event.pubkey.to_hex(),
            created_at: row.event.created_at.as_secs(),
            state,
            reason: optional(row, "reason", 500)?,
        },
    })
}

fn parse_reaction(row: &Row) -> Option<Related<Reaction>> {
    let reaction_rows = rows(row, "reaction");
    if reaction_rows.len() != 1 || reaction_rows[0].len() != 3 {
        return None;
    }
    let active = match reaction_rows[0][2].as_str() {
        "add" => true,
        "remove" => false,
        _ => return None,
    };
    Some(Related {
        root_id: root_event(row)?,
        value: Reaction {
            event_id: row.event.id.to_hex(),
            author: row.event.pubkey.to_hex(),
            created_at: row.event.created_at.as_secs(),
            emoji: bounded(&reaction_rows[0][1], 16)?,
            active,
        },
    })
}
