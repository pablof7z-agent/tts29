use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tts29_protocol::{validate_spoken_item, DurableArtifact, FrozenSpokenItem};

use crate::{
    InsertOutcome, JobJournal, JobPhase, JobRecord, JournalError, LocalAudioArtifact,
    ProducerRequest,
};

pub trait Synthesizer {
    fn synthesize(
        &mut self,
        request: &ProducerRequest,
        output: &Path,
    ) -> Result<LocalAudioArtifact, String>;
}

pub trait ArtifactUploader {
    fn make_durable(&mut self, audio: &LocalAudioArtifact) -> Result<DurableArtifact, String>;
}

pub trait Publisher {
    fn accept(&mut self, item: &FrozenSpokenItem) -> Result<u64, String>;
    fn resume(&mut self, receipt_id: u64, item: &FrozenSpokenItem) -> Result<String, String>;
}

#[derive(Debug)]
pub enum ProducerError {
    InvalidRequest(&'static str),
    RequestConflict(String),
    JobNotFound(String),
    Journal(JournalError),
    Capability { stage: &'static str, reason: String },
}

impl std::fmt::Display for ProducerError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest(field) => write!(output, "invalid producer request: {field}"),
            Self::RequestConflict(id) => write!(output, "request id was reused: {id}"),
            Self::JobNotFound(id) => write!(output, "producer job was not found: {id}"),
            Self::Journal(error) => error.fmt(output),
            Self::Capability { stage, reason } => {
                write!(output, "producer stage {stage} failed: {reason}")
            }
        }
    }
}

impl std::error::Error for ProducerError {}

impl From<JournalError> for ProducerError {
    fn from(value: JournalError) -> Self {
        Self::Journal(value)
    }
}

pub struct ProducerRunner<J, S, U, P> {
    journal: J,
    synthesizer: S,
    uploader: U,
    publisher: P,
    work_root: PathBuf,
}

impl<J, S, U, P> ProducerRunner<J, S, U, P>
where
    J: JobJournal,
    S: Synthesizer,
    U: ArtifactUploader,
    P: Publisher,
{
    pub fn new(
        journal: J,
        synthesizer: S,
        uploader: U,
        publisher: P,
        work_root: impl Into<PathBuf>,
    ) -> Result<Self, ProducerError> {
        let work_root = work_root.into();
        fs::create_dir_all(&work_root).map_err(JournalError::Io)?;
        Ok(Self {
            journal,
            synthesizer,
            uploader,
            publisher,
            work_root,
        })
    }

    pub fn admit(
        &mut self,
        request: ProducerRequest,
        author: String,
        created_at: u64,
    ) -> Result<JobRecord, ProducerError> {
        validate_request_id(&request.request_id)?;
        let digest = request_digest(&request)?;
        if let Some(existing) = self.journal.load(&request.request_id)? {
            return same_request(existing, &digest, &author);
        }
        let job = JobRecord {
            schema_version: 1,
            request_digest: digest.clone(),
            author: author.clone(),
            created_at,
            request,
            phase: JobPhase::Admitted,
        };
        match self.journal.insert(&job)? {
            InsertOutcome::Inserted => Ok(job),
            InsertOutcome::AlreadyExists => {
                let existing = self
                    .journal
                    .load(&job.request.request_id)?
                    .ok_or_else(|| ProducerError::JobNotFound(job.request.request_id.clone()))?;
                same_request(existing, &digest, &author)
            }
        }
    }

    pub fn advance(&mut self, request_id: &str) -> Result<JobRecord, ProducerError> {
        validate_request_id(request_id)?;
        let mut job = self
            .journal
            .load(request_id)?
            .ok_or_else(|| ProducerError::JobNotFound(request_id.to_string()))?;
        job.phase = match &job.phase {
            JobPhase::Admitted => {
                let output = self.audio_path(request_id);
                if let Some(parent) = output.parent() {
                    fs::create_dir_all(parent).map_err(JournalError::Io)?;
                }
                let audio = self
                    .synthesizer
                    .synthesize(&job.request, &output)
                    .map_err(|reason| capability("synthesis", reason))?;
                validate_local_audio(&audio, &output)?;
                JobPhase::Synthesized { audio }
            }
            JobPhase::Synthesized { audio } => {
                let durable = self
                    .uploader
                    .make_durable(audio)
                    .map_err(|reason| capability("artifact_upload", reason))?;
                if durable.sha256 != audio.sha256
                    || durable.byte_count != audio.byte_count
                    || durable.media_type != audio.media_type
                {
                    return Err(capability(
                        "artifact_upload",
                        "durable metadata does not match synthesized bytes".into(),
                    ));
                }
                let item = frozen_item(&job, durable);
                validate_spoken_item(&item)
                    .map_err(|error| capability("contract_freeze", error.to_string()))?;
                JobPhase::ArtifactsDurable { item }
            }
            JobPhase::ArtifactsDurable { item } => {
                let receipt_id = self
                    .publisher
                    .accept(item)
                    .map_err(|reason| capability("publication_acceptance", reason))?;
                JobPhase::PublicationAccepted {
                    item: item.clone(),
                    receipt_id,
                }
            }
            JobPhase::PublicationAccepted { item, receipt_id } => {
                let event_id = self
                    .publisher
                    .resume(*receipt_id, item)
                    .map_err(|reason| capability("publication_receipt", reason))?;
                if !is_event_id(&event_id) {
                    return Err(capability(
                        "publication_receipt",
                        "publisher returned an invalid event id".into(),
                    ));
                }
                JobPhase::Published {
                    item: item.clone(),
                    receipt_id: *receipt_id,
                    event_id,
                }
            }
            JobPhase::Published { .. } => return Ok(job),
        };
        self.journal.save(&job)?;
        Ok(job)
    }

    pub fn into_parts(self) -> (J, S, U, P) {
        (
            self.journal,
            self.synthesizer,
            self.uploader,
            self.publisher,
        )
    }

    fn audio_path(&self, request_id: &str) -> PathBuf {
        self.work_root.join(request_id).join("speech.mp3")
    }
}

fn frozen_item(job: &JobRecord, audio: DurableArtifact) -> FrozenSpokenItem {
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
    }
}

fn same_request(
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

fn validate_request_id(value: &str) -> Result<(), ProducerError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    valid
        .then_some(())
        .ok_or(ProducerError::InvalidRequest("request_id"))
}

fn request_digest(request: &ProducerRequest) -> Result<String, ProducerError> {
    let bytes =
        serde_json::to_vec(request).map_err(|_| ProducerError::InvalidRequest("serialization"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_local_audio(
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn is_event_id(value: &str) -> bool {
    is_sha256(value)
}

fn capability(stage: &'static str, reason: String) -> ProducerError {
    ProducerError::Capability { stage, reason }
}
