use std::collections::{BTreeMap, BTreeSet};

use crate::{AnswerBundle, Question, QuestionKind};

pub fn valid_answer(questions: &[Question], answer: &AnswerBundle) -> bool {
    if answer.answers.is_empty() || answer.answers.len() > 3 {
        return false;
    }
    let questions = questions
        .iter()
        .map(|question| (question.id.as_str(), question))
        .collect::<BTreeMap<_, _>>();
    let mut answered = BTreeSet::new();
    answer.answers.iter().all(|value| {
        if !answered.insert(value.question_id.as_str()) {
            return false;
        }
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

pub fn select_answer(
    questions: &[Question],
    candidates: impl IntoIterator<Item = AnswerBundle>,
) -> Option<AnswerBundle> {
    candidates
        .into_iter()
        .filter(|answer| valid_answer(questions, answer))
        .max_by(|left, right| answer_order(left).cmp(&answer_order(right)))
}

fn option_ids(question: &Question) -> BTreeSet<&String> {
    question.options.iter().map(|option| &option.id).collect()
}

fn answer_order(value: &AnswerBundle) -> (u64, &str) {
    (value.created_at, &value.event_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{QuestionAnswer, QuestionOption};

    #[test]
    fn selection_ignores_invalid_values_and_uses_the_conflict_order() {
        let questions = vec![question()];
        let invalid = answer("f", 30, &["unknown"]);
        let older = answer("a", 20, &["yes"]);
        let winner = answer("b", 20, &["no"]);

        let selected = select_answer(&questions, [invalid, older, winner.clone()]).unwrap();

        assert_eq!(selected, winner);
    }

    #[test]
    fn duplicate_values_fail_closed() {
        assert!(!valid_answer(
            &[question()],
            &answer("a", 20, &["yes", "yes"])
        ));
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

    fn answer(event_id: &str, created_at: u64, values: &[&str]) -> AnswerBundle {
        AnswerBundle {
            event_id: event_id.repeat(64),
            author: "a".repeat(64),
            created_at,
            answers: vec![QuestionAnswer {
                question_id: "ship".into(),
                values: values.iter().map(|value| (*value).to_string()).collect(),
            }],
        }
    }
}
