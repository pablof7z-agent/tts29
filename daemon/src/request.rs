use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};
use tts29_protocol::{DurableArtifact, FrozenSpokenItem};

use crate::{JobRecord, LocalAudioArtifact, ProducerError, ProducerRequest};

pub(crate) fn frozen_item(job: &JobRecord, audio: DurableArtifact) -> FrozenSpokenItem {
    FrozenSpokenItem {
        author: job.author.clone(),
        created_at: job.created_at,
        group_id: job.request.group_id.clone(),
        agent_name: job.request.agent_name.clone(),
        subject: job.request.subject.clone(),
        summary: job.request.summary.clone(),
        body: job.request.body.clone(),
        audio,
        attachments: job.request.attachments.clone(),
        questions: job.request.questions.clone(),
        attach: job.request.attach.clone(),
    }
}

pub(crate) fn same_request(
    existing: JobRecord,
    digest: &str,
    author: &str,
) -> Result<JobRecord, ProducerError> {
    if existing.request_digest == digest && existing.author == author {
        Ok(existing)
    } else {
        Err(ProducerError::RequestConflict(
            existing.request.request_id.clone(),
        ))
    }
}

pub(crate) fn validate_request_id(value: &str) -> Result<(), ProducerError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    valid
        .then_some(())
        .ok_or(ProducerError::InvalidRequest("request_id"))
}

pub(crate) fn request_digest(request: &ProducerRequest) -> Result<String, ProducerError> {
    let bytes =
        serde_json::to_vec(request).map_err(|_| ProducerError::InvalidRequest("serialization"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(crate) fn validate_local_audio(
    audio: &LocalAudioArtifact,
    expected_path: &Path,
) -> Result<(), ProducerError> {
    let bytes =
        fs::read(expected_path).map_err(|error| capability("synthesis", error.to_string()))?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let valid = Path::new(&audio.path) == expected_path
        && audio.byte_count > 0
        && audio.byte_count == bytes.len() as u64
        && audio.media_type.starts_with("audio/")
        && is_sha256(&audio.sha256)
        && audio.sha256 == digest;
    valid
        .then_some(())
        .ok_or(ProducerError::InvalidRequest("synthesized_audio"))
}

pub(crate) fn is_event_id(value: &str) -> bool {
    is_sha256(value)
}

pub(crate) fn capability(stage: &'static str, reason: String) -> ProducerError {
    ProducerError::Capability { stage, reason }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
