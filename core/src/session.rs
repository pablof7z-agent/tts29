use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use nmp::{AcquisitionEvidence, Engine, ReceiptStream, RelayUrl, Row, WriteStatus};
use tts29_protocol::{
    compose_answer, valid_answer, AnswerBundle, FrozenAnswer, QuestionAnswer, SpokenItem,
};

use crate::clock::Clock;
use crate::identity::IdentityController;
use crate::model::{
    AnswerSubmissionPhase, AnswerSubmissionSnapshot, KernelConfiguration, KernelPhase,
    QueueSnapshot,
};
use crate::projection::project;

pub struct Session {
    configuration: KernelConfiguration,
    host: RelayUrl,
    engine: Arc<Engine>,
    clock: Arc<dyn Clock>,
    identity: IdentityController,
    rows: Vec<Row>,
    evidence: AcquisitionEvidence,
    queue: QueueSnapshot,
    submissions: BTreeMap<String, AnswerSubmissionSnapshot>,
}

impl Session {
    pub fn new(
        configuration: KernelConfiguration,
        host: RelayUrl,
        engine: Arc<Engine>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let queue = QueueSnapshot::lifecycle(&configuration, KernelPhase::Listening);
        Self {
            configuration,
            host,
            engine,
            clock,
            identity: IdentityController::new(),
            rows: Vec::new(),
            evidence: AcquisitionEvidence::default(),
            queue,
            submissions: BTreeMap::new(),
        }
    }

    pub fn update_rows(&mut self, rows: Vec<Row>, evidence: AcquisitionEvidence) {
        self.rows = rows;
        self.evidence = evidence;
        self.reproject();
        let visible = visible_item_ids(&self.queue.items);
        self.submissions
            .retain(|item_id, _| visible.contains(item_id));
    }

    pub fn login(&mut self, secret: &str, persist: bool) {
        self.identity.login(&self.engine, secret, persist);
        self.reproject();
    }

    pub fn logout(&mut self) {
        self.identity.logout();
    }

    pub fn credential_load_failed(&mut self, error: String) {
        self.identity.restore_failed(error);
    }

    pub fn credential_result(&mut self, request_id: u64, succeeded: bool, error: Option<String>) {
        self.identity
            .credential_result(&self.engine, request_id, succeeded, error);
        self.reproject();
    }

    pub fn submit_answer(
        &mut self,
        item_id: &str,
        answers: Vec<QuestionAnswer>,
    ) -> Result<ReceiptStream, String> {
        let author = self
            .identity
            .active_pubkey()
            .ok_or_else(|| "Log in before answering.".to_string())?
            .to_string();
        if self
            .submissions
            .get(item_id)
            .is_some_and(|state| state.phase == AnswerSubmissionPhase::Sending)
        {
            return Err("That answer is already being sent.".into());
        }
        let item = find_item(&self.queue.items, item_id)
            .ok_or_else(|| "The question is no longer in the bounded queue.".to_string())?;
        if item.questions.is_empty() {
            return Err("That update has no questions.".into());
        }
        if item.answer.is_some() {
            return Err("That question already has an answer.".into());
        }
        let candidate = AnswerBundle {
            event_id: String::new(),
            author: author.clone(),
            created_at: self.clock.unix_seconds(),
            answers: answers.clone(),
        };
        if !valid_answer(&item.questions, &candidate) {
            return Err("The selected answer is not valid for this question.".into());
        }
        let frozen = FrozenAnswer {
            author,
            created_at: candidate.created_at,
            group_id: self.configuration.group_id.clone(),
            root_event_id: item.id.clone(),
            content: answer_content(item, &answers),
            answers,
        };
        let intent =
            compose_answer(self.host.clone(), &frozen).map_err(|error| error.to_string())?;
        let receipt = self
            .engine
            .publish_tracked(intent)
            .map_err(|error| format!("NMP refused the answer: {error}"))?;
        self.submissions.insert(
            item_id.to_string(),
            AnswerSubmissionSnapshot {
                item_id: item_id.to_string(),
                phase: AnswerSubmissionPhase::Sending,
                receipt_id: Some(receipt.id.0),
                event_id: None,
                error: None,
            },
        );
        Ok(receipt)
    }

    pub fn receipt_status(&mut self, item_id: &str, receipt_id: u64, status: WriteStatus) {
        let Some(submission) = self.submissions.get_mut(item_id) else {
            return;
        };
        if submission.receipt_id != Some(receipt_id) {
            return;
        }
        match status {
            WriteStatus::Signed(event_id) => submission.event_id = Some(event_id.to_hex()),
            WriteStatus::Acked(relay) if relay == self.host => {
                submission.phase = AnswerSubmissionPhase::Published;
                submission.error = None;
            }
            WriteStatus::Rejected(relay, reason) if relay == self.host => {
                fail(
                    submission,
                    format!("The group rejected the answer: {reason}"),
                );
            }
            WriteStatus::GaveUp(relay) if relay == self.host => {
                fail(submission, "The answer could not reach the group.".into());
            }
            WriteStatus::OutcomeUnknown(relay) if relay == self.host => {
                fail(
                    submission,
                    "The answer delivery outcome is unknown; do not send it again blindly.".into(),
                );
            }
            WriteStatus::Cancelled => fail(submission, "The answer was cancelled.".into()),
            WriteStatus::ReplaceableConflict { .. } => {
                fail(
                    submission,
                    "The answer conflicted before publication.".into(),
                );
            }
            WriteStatus::Failed(reason) => fail(submission, reason),
            _ => {}
        }
    }

    pub fn receipt_closed(&mut self, item_id: &str, receipt_id: u64) {
        let Some(submission) = self.submissions.get_mut(item_id) else {
            return;
        };
        if submission.receipt_id == Some(receipt_id)
            && submission.phase == AnswerSubmissionPhase::Sending
        {
            fail(
                submission,
                "Answer evidence closed before the group acknowledged it.".into(),
            );
        }
    }

    pub fn action_error(&mut self, item_id: Option<&str>, error: String) {
        if let Some(item_id) = item_id {
            self.submissions.insert(
                item_id.to_string(),
                AnswerSubmissionSnapshot {
                    item_id: item_id.to_string(),
                    phase: AnswerSubmissionPhase::Failed,
                    receipt_id: None,
                    event_id: None,
                    error: Some(error),
                },
            );
        } else {
            self.queue.error = Some(error);
        }
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        let mut snapshot = self.queue.clone();
        snapshot.identity = self.identity.snapshot();
        snapshot.credential_request = self.identity.credential_request();
        snapshot.answer_submissions = self.submissions.values().cloned().collect();
        snapshot
    }

    pub fn shutdown(&mut self) {
        self.identity.shutdown(&self.engine);
    }

    fn reproject(&mut self) {
        self.configuration.viewer_pubkey = self.identity.active_pubkey().map(str::to_string);
        if !self.rows.is_empty() {
            self.queue = project(&self.configuration, &self.rows, &self.evidence);
        }
    }
}

fn find_item<'a>(items: &'a [SpokenItem], item_id: &str) -> Option<&'a SpokenItem> {
    items.iter().find_map(|item| {
        (item.id == item_id)
            .then_some(item)
            .or_else(|| find_item(&item.children, item_id))
    })
}

fn visible_item_ids(items: &[SpokenItem]) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for item in items {
        result.insert(item.id.clone());
        result.extend(visible_item_ids(&item.children));
    }
    result
}

fn answer_content(item: &SpokenItem, answers: &[QuestionAnswer]) -> String {
    answers
        .iter()
        .filter_map(|answer| {
            let question = item
                .questions
                .iter()
                .find(|value| value.id == answer.question_id)?;
            let values = answer
                .values
                .iter()
                .map(|value| {
                    question
                        .options
                        .iter()
                        .find(|option| option.id == *value)
                        .map(|option| option.title.as_str())
                        .unwrap_or(value)
                })
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!("{}: {values}", question.title))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fail(submission: &mut AnswerSubmissionSnapshot, error: String) {
    submission.phase = AnswerSubmissionPhase::Failed;
    submission.error = Some(error);
}
