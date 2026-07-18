use std::collections::{BTreeMap, BTreeSet};

use nmp::{AcquisitionEvidence, Row};

use crate::model::{
    Acknowledgement, AcknowledgementState, AnswerBundle, KernelConfiguration, KernelPhase,
    QuestionKind, QueueEvidence, QueueSnapshot, ReactionSummary, SpokenItem,
};
use crate::protocol::{parse, ParsedEvent, Reaction};

const MAX_PROJECTED_ITEMS: usize = 40;

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
    let mut projected = items
        .into_values()
        .filter_map(|mut item| {
            let candidates = answers.remove(&item.id).unwrap_or_default();
            let (valid, invalid): (Vec<_>, Vec<_>) = candidates
                .into_iter()
                .partition(|answer| valid_answer(&item, answer));
            rejected_event_count += invalid.len();
            item.answer = valid
                .into_iter()
                .max_by(|left, right| answer_order(left).cmp(&answer_order(right)));
            item.acknowledgement = viewer
                .and_then(|author| acknowledgements.remove(&(item.id.clone(), author.to_string())));
            item.reactions = reaction_summaries(&item.id, &reactions);
            (!is_hidden(&item)).then_some(item)
        })
        .collect::<Vec<_>>();
    projected.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    projected.truncate(MAX_PROJECTED_ITEMS);

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

fn valid_answer(item: &SpokenItem, answer: &AnswerBundle) -> bool {
    let questions = item
        .questions
        .iter()
        .map(|question| (question.id.as_str(), question))
        .collect::<BTreeMap<_, _>>();
    answer.answers.iter().all(|value| {
        let Some(question) = questions.get(value.question_id.as_str()) else {
            return false;
        };
        let selected = value.values.iter().collect::<BTreeSet<_>>();
        if selected.len() != value.values.len() {
            return false;
        }
        match question.kind {
            QuestionKind::Freeform => value.values.len() == 1,
            QuestionKind::SingleChoice => {
                value.values.len() == 1 && option_ids(question).is_superset(&selected)
            }
            QuestionKind::MultipleChoice => option_ids(question).is_superset(&selected),
        }
    })
}

fn option_ids(question: &crate::model::Question) -> BTreeSet<&String> {
    question.options.iter().map(|option| &option.id).collect()
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
