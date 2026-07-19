use std::collections::{BTreeMap, BTreeSet};

use nmp::{AcquisitionEvidence, Row};

use crate::model::{KernelConfiguration, KernelPhase, QueueEvidence, QueueSnapshot};
use tts29_protocol::{
    parse, valid_answer, Acknowledgement, AcknowledgementState, AnswerBundle, ParsedEvent,
    Reaction, ReactionSummary, SpokenItem,
};

const MAX_PROJECTED_ITEMS: usize = 40;
const MAX_ATTACH_DEPTH: usize = 3;
const MAX_CHILDREN: usize = 12;

pub fn project(
    configuration: &KernelConfiguration,
    rows: &[Row],
    evidence: &AcquisitionEvidence,
) -> QueueSnapshot {
    let mut items = BTreeMap::new();
    let mut answers = BTreeMap::new();
    let mut acknowledgements = BTreeMap::new();
    let mut reactions = BTreeMap::new();
    let mut rejected_event_count = 0;

    for row in rows {
        match parse(row, &configuration.group_id) {
            Some(ParsedEvent::Item(item)) => {
                items.insert(item.id.clone(), *item);
            }
            Some(ParsedEvent::Answer(event)) => {
                answers
                    .entry(event.root_id)
                    .or_insert_with(Vec::new)
                    .push(event.value);
            }
            Some(ParsedEvent::Acknowledgement(event)) => {
                let key = (event.root_id, event.value.author.clone());
                replace_latest(
                    &mut acknowledgements,
                    key,
                    event.value,
                    acknowledgement_order,
                );
            }
            Some(ParsedEvent::Reaction(event)) => {
                let key = (
                    event.root_id,
                    event.value.author.clone(),
                    event.value.emoji.clone(),
                );
                replace_latest(&mut reactions, key, event.value, reaction_order);
            }
            None => rejected_event_count += 1,
        }
    }

    let known_item_ids = items.keys().cloned().collect::<BTreeSet<_>>();
    rejected_event_count += answers
        .iter()
        .filter(|(root, _)| !known_item_ids.contains(*root))
        .map(|(_, values)| values.len())
        .sum::<usize>();
    rejected_event_count += acknowledgements
        .keys()
        .filter(|(root, _)| !known_item_ids.contains(root))
        .count();
    rejected_event_count += reactions
        .keys()
        .filter(|(root, _, _)| !known_item_ids.contains(root))
        .count();

    let viewer = configuration.viewer_pubkey.as_deref();
    // Resolve per-item related state for every item, parents and children alike.
    let mut resolved: BTreeMap<String, SpokenItem> = BTreeMap::new();
    for (id, mut item) in items {
        let candidates = answers.remove(&item.id).unwrap_or_default();
        let (valid, invalid): (Vec<_>, Vec<_>) = candidates
            .into_iter()
            .partition(|answer| valid_answer(&item.questions, answer));
        rejected_event_count += invalid.len();
        item.answer = valid
            .into_iter()
            .max_by(|left, right| answer_order(left).cmp(&answer_order(right)));
        item.acknowledgement = viewer
            .and_then(|author| acknowledgements.remove(&(item.id.clone(), author.to_string())));
        item.reactions = reaction_summaries(&item.id, &reactions);
        resolved.insert(id, item);
    }

    // Index narrated children by parent; top-level items keep no attach link.
    let mut children_by_parent: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut top_level: Vec<String> = Vec::new();
    for (id, item) in &resolved {
        match item.attach.as_ref() {
            Some(link) if resolved.contains_key(&link.parent_id) => children_by_parent
                .entry(link.parent_id.clone())
                .or_default()
                .push(id.clone()),
            Some(_) => rejected_event_count += 1, // orphan: parent absent
            None => top_level.push(id.clone()),
        }
    }
    for ids in children_by_parent.values_mut() {
        ids.sort_by(|left, right| {
            resolved[left]
                .created_at
                .cmp(&resolved[right].created_at)
                .then_with(|| left.cmp(right))
        });
    }

    top_level.retain(|id| !is_hidden(&resolved[id]));
    top_level.sort_by(|left, right| {
        resolved[right]
            .created_at
            .cmp(&resolved[left].created_at)
            .then_with(|| left.cmp(right))
    });
    top_level.truncate(MAX_PROJECTED_ITEMS);

    let mut visited = BTreeSet::new();
    let projected = top_level
        .iter()
        .filter_map(|id| {
            assemble_subtree(
                id,
                &resolved,
                &children_by_parent,
                0,
                &mut visited,
                &mut rejected_event_count,
            )
        })
        .collect::<Vec<_>>();

    QueueSnapshot {
        phase: KernelPhase::Listening,
        relay: configuration.relay.clone(),
        group_id: configuration.group_id.clone(),
        items: projected,
        evidence: QueueEvidence {
            source_count: evidence.sources.len(),
            shortfall_count: evidence.shortfall.len(),
            rejected_event_count,
        },
        error: None,
    }
}

/// Clones an item with its narrated children nested beneath it, bounded by
/// depth and child count. Cycles and overflow are dropped and counted.
fn assemble_subtree(
    id: &str,
    resolved: &BTreeMap<String, SpokenItem>,
    children_by_parent: &BTreeMap<String, Vec<String>>,
    depth: usize,
    visited: &mut BTreeSet<String>,
    rejected: &mut usize,
) -> Option<SpokenItem> {
    if !visited.insert(id.to_string()) {
        return None;
    }
    let mut item = resolved.get(id)?.clone();
    let child_ids = children_by_parent
        .get(id)
        .cloned()
        .unwrap_or_default();
    if depth >= MAX_ATTACH_DEPTH {
        *rejected += child_ids.len();
        item.children = Vec::new();
        return Some(item);
    }
    let mut children = Vec::new();
    for child_id in child_ids {
        if children.len() >= MAX_CHILDREN {
            *rejected += 1;
            continue;
        }
        match assemble_subtree(&child_id, resolved, children_by_parent, depth + 1, visited, rejected)
        {
            Some(child) => children.push(child),
            None => *rejected += 1,
        }
    }
    item.children = children;
    Some(item)
}

fn replace_latest<K: Ord, V>(
    values: &mut BTreeMap<K, V>,
    key: K,
    candidate: V,
    order: fn(&V) -> (u64, &str),
) {
    let should_replace = values
        .get(&key)
        .map(|current| order(&candidate) > order(current))
        .unwrap_or(true);
    if should_replace {
        values.insert(key, candidate);
    }
}

fn reaction_summaries(
    item_id: &str,
    reactions: &BTreeMap<(String, String, String), Reaction>,
) -> Vec<ReactionSummary> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for ((root, author, emoji), reaction) in reactions {
        if root == item_id && reaction.active {
            grouped
                .entry(emoji.clone())
                .or_default()
                .push(author.clone());
        }
    }
    grouped
        .into_iter()
        .map(|(emoji, mut authors)| {
            authors.sort();
            ReactionSummary {
                emoji,
                count: authors.len(),
                authors,
            }
        })
        .collect()
}

fn is_hidden(item: &SpokenItem) -> bool {
    matches!(
        item.acknowledgement.as_ref().map(|value| &value.state),
        Some(AcknowledgementState::Dismissed | AcknowledgementState::Archived)
    )
}

fn answer_order(value: &AnswerBundle) -> (u64, &str) {
    (value.created_at, &value.event_id)
}

fn acknowledgement_order(value: &Acknowledgement) -> (u64, &str) {
    (value.created_at, &value.event_id)
}

fn reaction_order(value: &Reaction) -> (u64, &str) {
    (value.created_at, &value.event_id)
}
