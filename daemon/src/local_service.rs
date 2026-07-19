use std::time::Duration;

use crate::{
    AnswerWaitCancel, AnswerWaitError, AnswerWaitResult, JobPhase, LocalPublishRequest,
    LocalPublishResponse, LocalTreeRequest, ProducerError, ProductionProducer,
    LOCAL_PROTOCOL_VERSION,
};

pub trait LocalPublishService {
    fn publish_local(&mut self, request: LocalPublishRequest) -> LocalPublishResponse;
    fn publish_tree_local(&mut self, request: LocalTreeRequest) -> LocalPublishResponse;
}

impl LocalPublishService for ProductionProducer {
    fn publish_local(&mut self, input: LocalPublishRequest) -> LocalPublishResponse {
        if let Err(error) = input.validate() {
            return LocalPublishResponse::error(error.code, error.message);
        }
        let LocalPublishRequest {
            request,
            wait_for_answer_seconds,
            agent_nsec,
            ..
        } = input;
        let job = match self.publish_now(request, agent_nsec.as_deref()) {
            Ok(job) => job,
            Err(error) => return producer_error(error),
        };
        let JobPhase::Published {
            receipt_id,
            event_id,
            ..
        } = &job.phase
        else {
            return LocalPublishResponse::error(
                "publication_incomplete",
                "producer returned without durable publication evidence",
            );
        };
        let answer_wait = match wait_for_answer_seconds {
            None => AnswerWaitResult::NotRequested,
            Some(seconds) => {
                let cancel = AnswerWaitCancel::new();
                match self.wait_for_answer(&job, Duration::from_secs(seconds), &cancel) {
                    Ok(answer) => AnswerWaitResult::Answered { answer },
                    Err(AnswerWaitError::TimedOut) => AnswerWaitResult::TimedOut,
                    Err(error) => AnswerWaitResult::Unavailable {
                        code: answer_error_code(&error).into(),
                        message: error.to_string(),
                    },
                }
            }
        };
        LocalPublishResponse::Published {
            version: LOCAL_PROTOCOL_VERSION,
            request_id: job.request.request_id,
            receipt_id: *receipt_id,
            event_id: event_id.clone(),
            answer_wait,
        }
    }

    fn publish_tree_local(&mut self, input: LocalTreeRequest) -> LocalPublishResponse {
        if let Err(error) = input.validate() {
            return LocalPublishResponse::error(error.code, error.message);
        }
        let LocalTreeRequest {
            tree,
            agent_id,
            agent_nsec,
            ..
        } = input;
        let request_id = tree.request_id.clone();
        let agent_name = agent_id.unwrap_or_default();
        match self.publish_tree(tree, &agent_name, agent_nsec.as_deref()) {
            Ok(publication) => LocalPublishResponse::PublishedTree {
                version: LOCAL_PROTOCOL_VERSION,
                request_id,
                root_event_id: publication.root_event_id,
                child_event_ids: publication.child_event_ids,
            },
            Err(error) => producer_error(error),
        }
    }
}

fn producer_error(error: ProducerError) -> LocalPublishResponse {
    let code = match &error {
        ProducerError::InvalidRequest(_) => "invalid_request",
        ProducerError::RequestConflict(_) => "request_conflict",
        ProducerError::JobNotFound(_) => "job_not_found",
        ProducerError::Journal(_) => "journal_failed",
        ProducerError::Capability { .. } => "capability_failed",
    };
    LocalPublishResponse::error(code, error.to_string())
}

fn answer_error_code(error: &AnswerWaitError) -> &'static str {
    match error {
        AnswerWaitError::NotPublished => "not_published",
        AnswerWaitError::NoQuestions => "no_questions",
        AnswerWaitError::InvalidTimeout => "invalid_timeout",
        AnswerWaitError::Cancelled => "cancelled",
        AnswerWaitError::TimedOut => "timed_out",
        AnswerWaitError::EngineClosed => "engine_closed",
        AnswerWaitError::FrameLimit => "frame_limit",
        AnswerWaitError::Observation(_) => "observation_failed",
    }
}
