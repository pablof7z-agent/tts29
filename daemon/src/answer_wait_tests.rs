use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use nmp::{Engine, EngineConfig, Event, RelayUrl, Row};
use serde_json::json;
use tts29_protocol::{DurableArtifact, FrozenSpokenItem, Question, QuestionKind, QuestionOption};

use crate::answer_wait::answer_from_rows;
use crate::{
    AnswerWaitCancel, AnswerWaitError, AnswerWaiter, JobPhase, JobRecord, ProducerRequest,
    SystemClock,
};

const GROUP: &str = "tts";

#[test]
fn visible_answers_ignore_other_roots_and_choose_deterministically() {
    let rows = vec![
        answer_row(2, 20, &hex(9, 64), "yes"),
        answer_row(3, 20, &hex(1, 64), "yes"),
        answer_row(4, 20, &hex(1, 64), "no"),
        answer_row(5, 30, &hex(1, 64), "unknown"),
    ];

    let answer = answer_from_rows(&rows, GROUP, &hex(1, 64), &[question()]).unwrap();

    assert_eq!(answer.event_id, hex(4, 64));
    assert_eq!(answer.answers[0].values, ["no"]);
}

#[test]
fn a_pre_cancelled_wait_is_distinct_from_timeout() {
    let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
    let waiter = AnswerWaiter::new(
        Arc::clone(&engine),
        RelayUrl::parse("wss://relay.example.com").unwrap(),
        GROUP.into(),
        Arc::new(SystemClock),
    );
    let cancel = AnswerWaitCancel::new();
    cancel.cancel();

    let result = waiter.wait(&published_job(), Duration::from_secs(1), &cancel);

    assert_eq!(result, Err(AnswerWaitError::Cancelled));
    engine.shutdown();
}

#[test]
fn no_answer_before_the_bound_is_a_timeout() {
    let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
    let waiter = AnswerWaiter::new(
        Arc::clone(&engine),
        RelayUrl::parse("wss://relay.example.com").unwrap(),
        GROUP.into(),
        Arc::new(SystemClock),
    );

    let result = waiter.wait(
        &published_job(),
        Duration::from_millis(10),
        &AnswerWaitCancel::new(),
    );

    assert_eq!(result, Err(AnswerWaitError::TimedOut));
    engine.shutdown();
}

fn published_job() -> JobRecord {
    let request = ProducerRequest {
        request_id: "answer-wait".into(),
        group_id: GROUP.into(),
        voice: "af_heart".into(),
        agent_name: "Codex".into(),
        subject: "Answer".into(),
        summary: "Wait for a bounded answer".into(),
        body: "Should I ship?".into(),
        attachments: Vec::new(),
        questions: vec![question()],
    };
    let item = FrozenSpokenItem {
        author: hex_char('a', 64),
        created_at: 10,
        group_id: GROUP.into(),
        agent_name: request.agent_name.clone(),
        subject: request.subject.clone(),
        summary: request.summary.clone(),
        body: request.body.clone(),
        audio: DurableArtifact {
            url: "https://cdn.example/audio.mp3".into(),
            sha256: hex(9, 64),
            media_type: "audio/mpeg".into(),
            byte_count: 10,
            label: None,
        },
        attachments: Vec::new(),
        questions: request.questions.clone(),
    };
    JobRecord {
        schema_version: 1,
        request_digest: hex(8, 64),
        author: hex_char('a', 64),
        created_at: 10,
        request,
        phase: JobPhase::Published {
            item,
            membership: None,
            receipt_id: 1,
            event_id: hex(1, 64),
        },
    }
}

fn question() -> Question {
    Question {
        id: "ship".into(),
        kind: QuestionKind::SingleChoice,
        short_title: "Ship?".into(),
        title: "Should I ship?".into(),
        description: None,
        options: vec![
            QuestionOption {
                id: "yes".into(),
                title: "Yes".into(),
                description: None,
            },
            QuestionOption {
                id: "no".into(),
                title: "No".into(),
                description: None,
            },
        ],
    }
}

fn answer_row(id: u8, created_at: u64, root: &str, value: &str) -> Row {
    let event: Event = serde_json::from_value(json!({
        "id": hex(id, 64),
        "pubkey": hex_char('b', 64),
        "created_at": created_at,
        "kind": 9,
        "tags": [
            ["h", GROUP],
            ["tts29", "answer", "1"],
            ["e", root, "", "root"],
            ["answer", "ship", value]
        ],
        "content": "Answer submitted.",
        "sig": hex(7, 128)
    }))
    .unwrap();
    Row {
        event,
        sources: BTreeSet::from([RelayUrl::parse("wss://relay.example.com").unwrap()]),
    }
}

fn hex(value: u8, length: usize) -> String {
    format!("{value:x}").repeat(length)
}

fn hex_char(value: char, length: usize) -> String {
    value.to_string().repeat(length)
}
