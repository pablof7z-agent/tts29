use std::collections::BTreeSet;

use nmp::{AcquisitionEvidence, Event, RelayUrl, Row};
use serde_json::json;

use crate::model::KernelConfiguration;
use crate::projection::project;
use tts29_protocol::AcknowledgementState;

const GROUP: &str = "tts";
const VIEWER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn accepts_complete_content_addressed_items() {
    let item = item_row(1, 10);
    let snapshot = project(
        &configuration(None),
        &[item],
        &AcquisitionEvidence::default(),
    );

    assert_eq!(snapshot.items.len(), 1);
    let item = &snapshot.items[0];
    assert_eq!(item.subject, "Build Ready");
    assert_eq!(item.audio.sha256, hex(9, 64));
    assert_eq!(item.attachments[0].label.as_deref(), Some("Report"));
    assert_eq!(item.questions[0].options.len(), 2);
    assert_eq!(snapshot.evidence.rejected_event_count, 0);
}

#[test]
fn ignores_unrelated_and_malformed_group_messages() {
    let unrelated = event_row(
        2,
        'a',
        11,
        vec![vec!["h", GROUP], vec!["title", "Ordinary chat"]],
        "Hello",
    );
    let malformed = event_row(
        3,
        'a',
        12,
        vec![
            vec!["h", GROUP],
            vec!["tts29", "item", "1"],
            vec!["title", "Missing artifact"],
            vec!["summary", "This item has no audio metadata."],
        ],
        "No audio",
    );
    let snapshot = project(
        &configuration(None),
        &[unrelated, malformed],
        &AcquisitionEvidence::default(),
    );

    assert!(snapshot.items.is_empty());
    assert_eq!(snapshot.evidence.rejected_event_count, 2);
}

#[test]
fn conflicting_related_events_converge_independent_of_arrival_order() {
    let rows = vec![
        item_row(1, 10),
        answer_row(2, 20, "no"),
        answer_row(3, 20, "yes"),
        answer_row(9, 30, "unknown-option"),
        acknowledgement_row(4, 21, "archived"),
        acknowledgement_row(5, 22, "active"),
        reaction_row(6, 'c', 23, "add"),
        reaction_row(7, 'c', 24, "remove"),
        reaction_row(8, 'd', 23, "add"),
    ];
    let mut reversed = rows.clone();
    reversed.reverse();

    let first = project(
        &configuration(Some(VIEWER)),
        &rows,
        &AcquisitionEvidence::default(),
    );
    let second = project(
        &configuration(Some(VIEWER)),
        &reversed,
        &AcquisitionEvidence::default(),
    );

    assert_eq!(first.items, second.items);
    let item = &first.items[0];
    assert_eq!(item.answer.as_ref().unwrap().answers[0].values, ["yes"]);
    assert_eq!(
        item.acknowledgement.as_ref().unwrap().state,
        AcknowledgementState::Active
    );
    assert_eq!(item.reactions[0].count, 1);
    assert_eq!(item.reactions[0].authors, [hex_char('d', 64)]);
}

#[test]
fn viewer_archive_hides_item_without_publishing_playback_state() {
    let rows = vec![item_row(1, 10), acknowledgement_row(5, 22, "archived")];
    let snapshot = project(
        &configuration(Some(VIEWER)),
        &rows,
        &AcquisitionEvidence::default(),
    );

    assert!(snapshot.items.is_empty());
}

#[test]
fn nests_narrated_child_and_excludes_it_from_top_level() {
    let rows = vec![item_row(1, 10), child_row(2, 9, 1, "Details")];
    let snapshot = project(&configuration(None), &rows, &AcquisitionEvidence::default());

    assert_eq!(snapshot.items.len(), 1);
    let parent = &snapshot.items[0];
    assert_eq!(parent.children.len(), 1);
    let child = &parent.children[0];
    let link = child.attach.as_ref().unwrap();
    assert_eq!(link.label, "Details");
    assert_eq!(link.parent_id, hex(1, 64));
    assert_eq!(snapshot.evidence.rejected_event_count, 0);
}

#[test]
fn orphan_narrated_child_is_rejected() {
    let rows = vec![child_row(2, 9, 1, "Details")];
    let snapshot = project(&configuration(None), &rows, &AcquisitionEvidence::default());

    assert!(snapshot.items.is_empty());
    assert_eq!(snapshot.evidence.rejected_event_count, 1);
}

#[test]
fn nests_grandchildren_and_bounds_depth() {
    let rows = vec![
        item_row(1, 10),
        child_row(2, 9, 1, "L1"),
        child_row(3, 8, 2, "L2"),
        child_row(4, 7, 3, "L3"),
        child_row(5, 6, 4, "L4"),
    ];
    let snapshot = project(&configuration(None), &rows, &AcquisitionEvidence::default());

    assert_eq!(snapshot.items.len(), 1);
    let l1 = &snapshot.items[0].children[0];
    let l2 = &l1.children[0];
    let l3 = &l2.children[0];
    assert!(l3.children.is_empty());
    assert_eq!(snapshot.evidence.rejected_event_count, 1);
}

fn child_row(id: u8, created_at: u64, parent: u8, label: &str) -> Row {
    event_row(
        id,
        'a',
        created_at,
        vec![
            vec!["h", GROUP],
            vec!["tts29", "item", "1"],
            vec!["title", "Detailed explanation"],
            vec!["agent", "Codex"],
            vec![
                "audio",
                "https://cdn.example/child.mp3",
                &hex(5, 64),
                "audio/mpeg",
                "900",
            ],
            vec!["e", &hex(parent, 64), "", "attach", label],
        ],
        "This is the narrated branch body.",
    )
}

fn item_row(id: u8, created_at: u64) -> Row {
    event_row(
        id,
        'a',
        created_at,
        vec![
            vec!["h", GROUP],
            vec!["tts29", "item", "1"],
            vec!["title", "Build Ready"],
            vec!["summary", "The build is ready."],
            vec!["agent", "Codex"],
            vec![
                "audio",
                "https://cdn.example/audio.mp3",
                &hex(9, 64),
                "audio/mpeg",
                "1200",
            ],
            vec![
                "attachment",
                "https://cdn.example/report.md",
                &hex(8, 64),
                "text/markdown",
                "300",
                "Report",
            ],
            vec!["question", "ship", "single", "Should I ship it?"],
            vec!["label", "ship", "Ship?"],
            vec!["option", "ship", "yes", "Yes"],
            vec!["option", "ship", "no", "No"],
        ],
        "The simulator build is ready.",
    )
}

fn answer_row(id: u8, created_at: u64, value: &'static str) -> Row {
    event_row(
        id,
        'b',
        created_at,
        vec![
            vec!["h", GROUP],
            vec!["tts29", "answer", "1"],
            vec!["e", &hex(1, 64), "", "root"],
            vec!["answer", "ship", value],
        ],
        "Answer submitted.",
    )
}

fn acknowledgement_row(id: u8, created_at: u64, state: &'static str) -> Row {
    event_row(
        id,
        'b',
        created_at,
        vec![
            vec!["h", GROUP],
            vec!["tts29", "ack", "1"],
            vec!["e", &hex(1, 64), "", "root"],
            vec!["state", state],
        ],
        state,
    )
}

fn reaction_row(id: u8, author: char, created_at: u64, action: &'static str) -> Row {
    event_row(
        id,
        author,
        created_at,
        vec![
            vec!["h", GROUP],
            vec!["tts29", "reaction", "1"],
            vec!["e", &hex(1, 64), "", "root"],
            vec!["reaction", "👍", action],
        ],
        "👍",
    )
}

fn event_row(id: u8, author: char, created_at: u64, tags: Vec<Vec<&str>>, content: &str) -> Row {
    let event: Event = serde_json::from_value(json!({
        "id": hex(id, 64),
        "pubkey": hex_char(author, 64),
        "created_at": created_at,
        "kind": 9,
        "tags": tags,
        "content": content,
        "sig": hex(7, 128)
    }))
    .expect("fixture event");
    Row {
        event,
        sources: BTreeSet::from([RelayUrl::parse("wss://relay.example.com").unwrap()]),
    }
}

fn configuration(viewer_pubkey: Option<&str>) -> KernelConfiguration {
    KernelConfiguration {
        relay: "wss://relay.example.com".into(),
        group_id: GROUP.into(),
        store_path: None,
        viewer_pubkey: viewer_pubkey.map(str::to_string),
    }
}

fn hex(value: u8, length: usize) -> String {
    format!("{value:x}").repeat(length)
}

fn hex_char(value: char, length: usize) -> String {
    value.to_string().repeat(length)
}
