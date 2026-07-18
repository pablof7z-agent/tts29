use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nmp::{Engine, LiveQuery, ObservationCancel, RelayUrl, Row, Window};
use nmp_nip29::group_content_demand;
use tts29_protocol::{parse, select_answer, AnswerBundle, ParsedEvent, Question};

use crate::{Clock, JobPhase, JobRecord};

#[derive(Debug, PartialEq, Eq)]
pub enum AnswerWaitError {
    NotPublished,
    NoQuestions,
    InvalidTimeout,
    Cancelled,
    TimedOut,
    EngineClosed,
    FrameLimit,
    Observation(String),
}

impl std::fmt::Display for AnswerWaitError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPublished => output.write_str("answer wait requires a published job"),
            Self::NoQuestions => output.write_str("published item has no questions"),
            Self::InvalidTimeout => output.write_str("answer wait timeout is invalid"),
            Self::Cancelled => output.write_str("answer wait was cancelled"),
            Self::TimedOut => output.write_str("answer wait timed out"),
            Self::EngineClosed => output.write_str("NMP answer stream closed"),
            Self::FrameLimit => output.write_str("NMP answer stream exceeded its frame bound"),
            Self::Observation(reason) => write!(output, "NMP refused answer observation: {reason}"),
        }
    }
}

impl std::error::Error for AnswerWaitError {}

#[derive(Default)]
pub struct AnswerWaitCancel {
    cancelled: AtomicBool,
    observation: Mutex<Option<ObservationCancel>>,
}

impl AnswerWaitCancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let observation = self
            .observation
            .lock()
            .unwrap_or_else(|value| value.into_inner());
        if let Some(observation) = observation.as_ref() {
            observation.cancel();
        }
    }

    fn install(&self, observation: ObservationCancel) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            observation.cancel();
            return false;
        }
        let mut stored = self
            .observation
            .lock()
            .unwrap_or_else(|value| value.into_inner());
        if self.cancelled.load(Ordering::Acquire) {
            observation.cancel();
            return false;
        }
        *stored = Some(observation);
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub struct AnswerWaiter {
    engine: Arc<Engine>,
    host: RelayUrl,
    group_id: String,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl AnswerWaiter {
    pub fn new(
        engine: Arc<Engine>,
        host: RelayUrl,
        group_id: String,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            engine,
            host,
            group_id,
            clock,
        }
    }

    pub fn wait(
        &self,
        job: &JobRecord,
        timeout: Duration,
        cancel: &AnswerWaitCancel,
    ) -> Result<AnswerBundle, AnswerWaitError> {
        let JobPhase::Published { item, event_id, .. } = &job.phase else {
            return Err(AnswerWaitError::NotPublished);
        };
        if item.questions.is_empty() {
            return Err(AnswerWaitError::NoQuestions);
        }
        let timeout_millis = u64::try_from(timeout.as_millis())
            .ok()
            .filter(|value| *value > 0)
            .ok_or(AnswerWaitError::InvalidTimeout)?;
        let deadline = self
            .clock
            .unix_millis()
            .checked_add(timeout_millis)
            .ok_or(AnswerWaitError::InvalidTimeout)?;
        let demand = group_content_demand(self.host.clone(), &self.group_id);
        let rows = NonZeroUsize::new(100).expect("answer window is nonzero");
        let subscription = self
            .engine
            .observe(
                LiveQuery(demand),
                Some(Window::Expandable {
                    initial: rows,
                    max: rows,
                }),
            )
            .map_err(|error| AnswerWaitError::Observation(error.to_string()))?;
        if !cancel.install(subscription.cancel_handle()) {
            return Err(AnswerWaitError::Cancelled);
        }

        for _ in 0..128 {
            if cancel.is_cancelled() {
                return Err(AnswerWaitError::Cancelled);
            }
            let remaining = deadline.saturating_sub(self.clock.unix_millis());
            if remaining == 0 {
                return Err(AnswerWaitError::TimedOut);
            }
            match subscription.recv_timeout(Duration::from_millis(remaining)) {
                Ok(frame) => {
                    if cancel.is_cancelled() {
                        return Err(AnswerWaitError::Cancelled);
                    }
                    if self.clock.unix_millis() >= deadline {
                        return Err(AnswerWaitError::TimedOut);
                    }
                    if let Some(window) = frame.window {
                        if let Some(answer) = answer_from_rows(
                            &window.rows,
                            &self.group_id,
                            event_id,
                            &item.questions,
                        ) {
                            return Ok(answer);
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    return Err(AnswerWaitError::TimedOut);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(if cancel.is_cancelled() {
                        AnswerWaitError::Cancelled
                    } else {
                        AnswerWaitError::EngineClosed
                    });
                }
            }
        }
        Err(AnswerWaitError::FrameLimit)
    }
}

pub(crate) fn answer_from_rows(
    rows: &[Row],
    group_id: &str,
    event_id: &str,
    questions: &[Question],
) -> Option<AnswerBundle> {
    let candidates = rows.iter().filter_map(|row| match parse(row, group_id) {
        Some(ParsedEvent::Answer(answer)) if answer.root_id == event_id => Some(answer.value),
        _ => None,
    });
    select_answer(questions, candidates)
}
