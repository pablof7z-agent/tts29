use nmp::{AcquisitionEvidence, Row};

use crate::model::{KernelConfiguration, KernelPhase, QueueEvidence, QueueSnapshot, SpokenItem};

const MAX_PROJECTED_ITEMS: usize = 40;

pub fn project(
    configuration: &KernelConfiguration,
    rows: &[Row],
    evidence: &AcquisitionEvidence,
) -> QueueSnapshot {
    let mut items = rows.iter().filter_map(spoken_item).collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    items.truncate(MAX_PROJECTED_ITEMS);

    QueueSnapshot {
        phase: KernelPhase::Listening,
        relay: configuration.relay.clone(),
        group_id: configuration.group_id.clone(),
        items,
        evidence: QueueEvidence {
            source_count: evidence.sources.len(),
            shortfall_count: evidence.shortfall.len(),
        },
        error: None,
    }
}

fn spoken_item(row: &Row) -> Option<SpokenItem> {
    let event = &row.event;
    if event.kind.as_u16() != 9 {
        return None;
    }

    let title = tag_value(row, "title").unwrap_or_else(|| first_line(&event.content));
    let summary = tag_value(row, "summary").unwrap_or_else(|| preview(&event.content));
    Some(SpokenItem {
        id: event.id.to_hex(),
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs(),
        subject: title,
        summary,
        body: event.content.clone(),
        audio_url: tag_value(row, "audio"),
    })
}

fn tag_value(row: &Row, name: &str) -> Option<String> {
    row.event.tags.iter().find_map(|tag| {
        let values = tag.as_slice();
        (values.first().map(String::as_str) == Some(name))
            .then(|| values.get(1).cloned())
            .flatten()
            .filter(|value| !value.trim().is_empty())
    })
}

fn first_line(value: &str) -> String {
    let line = value
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Spoken update");
    truncate(line.trim(), 80)
}

fn preview(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&collapsed, 140)
}

fn truncate(value: &str, max_characters: usize) -> String {
    let mut characters = value.chars();
    let prefix = characters.by_ref().take(max_characters).collect::<String>();
    if characters.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use nmp::{AcquisitionEvidence, Event, RelayUrl, Row};

    use super::project;
    use crate::model::KernelConfiguration;

    #[test]
    fn projects_kind_nine_tags_without_native_policy() {
        let event: Event = serde_json::from_value(serde_json::json!({
            "id": "0000000000000000000000000000000000000000000000000000000000000000",
            "pubkey": "1111111111111111111111111111111111111111111111111111111111111111",
            "created_at": 42,
            "kind": 9,
            "tags": [["h", "tts"], ["title", "Build Ready"], ["summary", "The build is ready."], ["audio", "https://example.com/audio.mp3"]],
            "content": "The simulator build is ready.",
            "sig": "22222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222"
        }))
        .expect("fixture event");
        let row = Row {
            event,
            sources: BTreeSet::from([RelayUrl::parse("wss://relay.example.com").unwrap()]),
        };
        let configuration = KernelConfiguration {
            relay: "wss://relay.example.com".into(),
            group_id: "tts".into(),
            store_path: None,
        };

        let snapshot = project(&configuration, &[row], &AcquisitionEvidence::default());

        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].subject, "Build Ready");
        assert_eq!(snapshot.items[0].summary, "The build is ready.");
        assert_eq!(
            snapshot.items[0].audio_url.as_deref(),
            Some("https://example.com/audio.mp3")
        );
    }
}
