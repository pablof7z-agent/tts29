use std::collections::{BTreeMap, BTreeSet};

use nmp::Row;

use crate::{DurableArtifact, Question, QuestionKind, QuestionOption};

const MAX_ARTIFACT_BYTES: u64 = 250 * 1024 * 1024;

pub fn rows(row: &Row, name: &str) -> Vec<Vec<String>> {
    row.event
        .tags
        .iter()
        .filter_map(|tag| {
            let values = tag.as_slice();
            (values.first().map(String::as_str) == Some(name)).then(|| values.to_vec())
        })
        .collect()
}

pub fn unique(row: &Row, name: &str, max: usize) -> Option<String> {
    let matches = rows(row, name);
    if matches.len() != 1 || matches[0].len() != 2 {
        return None;
    }
    bounded(&matches[0][1], max)
}

pub fn optional(row: &Row, name: &str, max: usize) -> Option<Option<String>> {
    let matches = rows(row, name);
    match matches.as_slice() {
        [] => Some(None),
        [value] if value.len() == 2 => bounded(&value[1], max).map(Some),
        _ => None,
    }
}

pub fn marker(row: &Row) -> Option<(String, String)> {
    let matches = rows(row, "tts29");
    let value = matches.first()?;
    (matches.len() == 1 && value.len() == 3).then(|| (value[1].clone(), value[2].clone()))
}

pub fn group_matches(row: &Row, group_id: &str) -> bool {
    rows(row, "h") == [vec!["h".to_string(), group_id.to_string()]]
}

pub fn root_event(row: &Row) -> Option<String> {
    let matches = rows(row, "e");
    if matches.len() != 1 || matches[0].len() != 4 || matches[0][3] != "root" {
        return None;
    }
    let id = &matches[0][1];
    (is_lower_hex(id, 64) && matches[0][2].is_empty()).then(|| id.clone())
}

pub fn artifact(row: &[String], tag_name: &str, with_label: bool) -> Option<DurableArtifact> {
    let expected = if with_label { 6 } else { 5 };
    if row.len() != expected || row.first().map(String::as_str) != Some(tag_name) {
        return None;
    }
    let url = bounded(&row[1], 2048)?;
    let sha256 = bounded(&row[2], 64)?;
    let media_type = bounded(&row[3], 128)?;
    let byte_count = row[4].parse::<u64>().ok()?;
    if !url.starts_with("https://")
        || !is_lower_hex(&sha256, 64)
        || !media_type.contains('/')
        || byte_count == 0
        || byte_count > MAX_ARTIFACT_BYTES
    {
        return None;
    }
    Some(DurableArtifact {
        url,
        sha256,
        media_type,
        byte_count,
        label: with_label.then(|| bounded(&row[5], 120)).flatten(),
    })
}

pub fn questions(row: &Row) -> Option<Vec<Question>> {
    let question_rows = rows(row, "question");
    if question_rows.len() > 3 {
        return None;
    }
    let labels = keyed_text(row, "label", 40)?;
    let descriptions = keyed_text(row, "description", 500)?;
    let mut options = keyed_options(row)?;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in question_rows {
        if value.len() != 4 || !seen.insert(value[1].clone()) {
            return None;
        }
        let id = identifier(&value[1])?;
        let kind = match value[2].as_str() {
            "single" => QuestionKind::SingleChoice,
            "multiple" => QuestionKind::MultipleChoice,
            "freeform" => QuestionKind::Freeform,
            _ => return None,
        };
        let title = bounded(&value[3], 240)?;
        let question_options = options.remove(&id).unwrap_or_default();
        let choices_required = matches!(
            kind,
            QuestionKind::SingleChoice | QuestionKind::MultipleChoice
        );
        if choices_required == question_options.is_empty() {
            return None;
        }
        result.push(Question {
            short_title: labels.get(&id).cloned().unwrap_or_else(|| title.clone()),
            description: descriptions.get(&id).cloned(),
            id,
            kind,
            title,
            options: question_options,
        });
    }
    let known = seen;
    if labels.keys().any(|id| !known.contains(id))
        || descriptions.keys().any(|id| !known.contains(id))
        || !options.is_empty()
    {
        return None;
    }
    Some(result)
}

fn keyed_text(row: &Row, name: &str, max: usize) -> Option<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for value in rows(row, name) {
        if value.len() != 3 {
            return None;
        }
        let id = identifier(&value[1])?;
        if result.insert(id, bounded(&value[2], max)?).is_some() {
            return None;
        }
    }
    Some(result)
}

fn keyed_options(row: &Row) -> Option<BTreeMap<String, Vec<QuestionOption>>> {
    let mut result: BTreeMap<String, Vec<QuestionOption>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for value in rows(row, "option") {
        if !matches!(value.len(), 4 | 5) {
            return None;
        }
        let question_id = identifier(&value[1])?;
        let id = identifier(&value[2])?;
        if !seen.insert((question_id.clone(), id.clone())) {
            return None;
        }
        let values = result.entry(question_id).or_default();
        if values.len() == 8 {
            return None;
        }
        let description = match value.get(4) {
            Some(description) if !description.is_empty() => Some(bounded(description, 300)?),
            _ => None,
        };
        values.push(QuestionOption {
            id,
            title: bounded(&value[3], 120)?,
            description,
        });
    }
    Some(result)
}

pub fn identifier(value: &str) -> Option<String> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    valid.then(|| value.to_string())
}

pub fn bounded(value: &str, max: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().count() <= max).then(|| value.to_string())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
