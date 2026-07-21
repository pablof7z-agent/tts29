use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nmp::{AcquisitionEvidence, Engine, EngineConfig, Event, RelayUrl, Row, WriteStatus};
use serde_json::json;
use tts29_protocol::QuestionAnswer;

use crate::clock::FixedClock;
use crate::model::{IdentityPhase, KernelConfiguration};
use crate::session::Session;

#[test]
fn active_user_account_signs_a_bounded_kind_nine_answer() {
    let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
    let host = RelayUrl::parse("ws://127.0.0.1:9").unwrap();
    let configuration = KernelConfiguration {
        relay: host.to_string(),
        group_id: "tts".into(),
        store_path: None,
        viewer_pubkey: None,
    };
    let mut session = Session::new(
        configuration,
        host,
        Arc::clone(&engine),
        Arc::new(FixedClock(42)),
    );
    session.update_rows(vec![question_row()], AcquisitionEvidence::default());
    session.login(&"3".repeat(64), false);

    let pubkey = session.snapshot().identity.pubkey.unwrap();
    assert_eq!(session.snapshot().identity.phase, IdentityPhase::SignedIn);
    let receipt = session
        .submit_answer(
            &"1".repeat(64),
            vec![QuestionAnswer {
                question_id: "ship".into(),
                values: vec!["yes".into()],
            }],
        )
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    let signed_id = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receipt.statuses.recv_timeout(remaining) {
            Ok(WriteStatus::Signed(event_id)) => break event_id.to_hex(),
            Ok(_) => {}
            Err(error) => panic!("answer was not signed by {pubkey}: {error}"),
        }
    };
    assert_eq!(signed_id.len(), 64);

    session.shutdown();
    engine.shutdown();
}

#[test]
fn signed_out_session_rejects_answers_before_nmp() {
    let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
    let host = RelayUrl::parse("ws://127.0.0.1:9").unwrap();
    let mut session = Session::new(
        KernelConfiguration {
            relay: host.to_string(),
            group_id: "tts".into(),
            store_path: None,
            viewer_pubkey: None,
        },
        host,
        Arc::clone(&engine),
        Arc::new(FixedClock(42)),
    );
    session.update_rows(vec![question_row()], AcquisitionEvidence::default());

    let error = match session.submit_answer(
        &"1".repeat(64),
        vec![QuestionAnswer {
            question_id: "ship".into(),
            values: vec!["yes".into()],
        }],
    ) {
        Ok(_) => panic!("signed-out answer unexpectedly reached NMP"),
        Err(error) => error,
    };

    assert_eq!(error, "Log in before answering.");
    session.shutdown();
    engine.shutdown();
}

fn question_row() -> Row {
    let event: Event = serde_json::from_value(json!({
        "id": "1".repeat(64),
        "pubkey": "a".repeat(64),
        "created_at": 10,
        "kind": 9,
        "tags": [
            ["h", "tts"],
            ["tts29", "item", "1"],
            ["title", "Build Ready"],
            ["summary", "The build is ready."],
            ["audio", "https://cdn.example/audio.mp3", "9".repeat(64), "audio/mpeg", "1200"],
            ["question", "ship", "single", "Should I ship it?"],
            ["label", "ship", "Ship?"],
            ["option", "ship", "yes", "Yes"],
            ["option", "ship", "no", "No"]
        ],
        "content": "The simulator build is ready.",
        "sig": "7".repeat(128)
    }))
    .unwrap();
    Row {
        event,
        sources: BTreeSet::from([RelayUrl::parse("ws://127.0.0.1:9").unwrap()]),
    }
}
