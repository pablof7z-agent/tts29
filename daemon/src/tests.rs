use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tts29_protocol::{DurableArtifact, FrozenSpokenItem};

use crate::{
    ArtifactUploader, FileJobJournal, InsertOutcome, JobJournal, JobRecord, JournalError,
    LocalAudioArtifact, ProducerError, ProducerRequest, ProducerRunner, Publisher, Synthesizer,
};

#[test]
fn admission_is_idempotent_and_conflicting_reuse_fails_closed() {
    let temporary = TempDir::new().unwrap();
    let journal = FileJobJournal::open(temporary.path().join("jobs")).unwrap();
    let mut runner = runner(journal, temporary.path().join("work"));
    let first = runner.admit(request("one"), author(), 100).unwrap();
    let repeated = runner.admit(request("one"), author(), 999).unwrap();

    assert_eq!(first, repeated);
    let mut conflicting = request("one");
    conflicting.body = "Different immutable input".into();
    assert!(matches!(
        runner.admit(conflicting, author(), 100),
        Err(ProducerError::RequestConflict(id)) if id == "one"
    ));
}

#[test]
fn every_lost_stage_commit_recovers_without_duplicate_artifacts_or_events() {
    let temporary = TempDir::new().unwrap();
    let journal = FaultJournal::new([
        "synthesized",
        "artifacts_durable",
        "publication_accepted",
        "published",
    ]);
    let mut runner = runner(journal, temporary.path().join("work"));
    runner.admit(request("recover"), author(), 123).unwrap();

    for _ in 0..12 {
        let _ = runner.advance("recover");
    }
    let (mut journal, synthesizer, uploader, publisher) = runner.into_parts();
    let job = journal.load("recover").unwrap().unwrap();

    assert!(job.phase.is_published());
    assert_eq!(synthesizer.calls, 2);
    assert_eq!(synthesizer.generated_paths.len(), 1);
    assert_eq!(uploader.calls, 2);
    assert_eq!(uploader.blobs.len(), 1);
    assert_eq!(publisher.accept_calls, 2);
    assert_eq!(publisher.resume_calls, 2);
    assert_eq!(publisher.event_ids.len(), 1);
}

fn runner<J: JobJournal>(
    journal: J,
    work_root: PathBuf,
) -> ProducerRunner<J, FakeSynthesizer, FakeUploader, FakePublisher> {
    ProducerRunner::new(
        journal,
        FakeSynthesizer::default(),
        FakeUploader::default(),
        FakePublisher::default(),
        work_root,
    )
    .unwrap()
}

fn request(id: &str) -> ProducerRequest {
    ProducerRequest {
        request_id: id.into(),
        group_id: "tts".into(),
        voice: "af_heart".into(),
        agent_name: "Codex".into(),
        subject: "Recovery ready".into(),
        summary: "The producer can recover the request.".into(),
        body: "The producer can recover every completed stage.".into(),
        attachments: Vec::new(),
        questions: Vec::new(),
    }
}

fn author() -> String {
    "a".repeat(64)
}

#[derive(Default)]
struct FakeSynthesizer {
    calls: usize,
    generated_paths: BTreeSet<PathBuf>,
}

impl Synthesizer for FakeSynthesizer {
    fn synthesize(
        &mut self,
        _request: &ProducerRequest,
        output: &Path,
    ) -> Result<LocalAudioArtifact, String> {
        self.calls += 1;
        if self.generated_paths.insert(output.to_path_buf()) {
            fs::write(output, b"deterministic speech").map_err(|error| error.to_string())?;
        }
        let bytes = fs::read(output).map_err(|error| error.to_string())?;
        Ok(LocalAudioArtifact {
            path: output.to_string_lossy().into_owned(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            media_type: "audio/mpeg".into(),
            byte_count: bytes.len() as u64,
        })
    }
}

#[derive(Default)]
struct FakeUploader {
    calls: usize,
    blobs: BTreeSet<String>,
}

impl ArtifactUploader for FakeUploader {
    fn make_durable(&mut self, audio: &LocalAudioArtifact) -> Result<DurableArtifact, String> {
        self.calls += 1;
        self.blobs.insert(audio.sha256.clone());
        Ok(DurableArtifact {
            url: format!("https://cdn.example/{}.mp3", audio.sha256),
            sha256: audio.sha256.clone(),
            media_type: audio.media_type.clone(),
            byte_count: audio.byte_count,
            label: None,
        })
    }
}

#[derive(Default)]
struct FakePublisher {
    accept_calls: usize,
    resume_calls: usize,
    event_ids: BTreeSet<String>,
    receipts: BTreeMap<u64, String>,
}

impl Publisher for FakePublisher {
    fn accept(&mut self, item: &FrozenSpokenItem) -> Result<u64, String> {
        self.accept_calls += 1;
        let event_id = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(item).map_err(|error| error.to_string())?)
        );
        self.event_ids.insert(event_id.clone());
        let receipt_id = self.accept_calls as u64;
        self.receipts.insert(receipt_id, event_id);
        Ok(receipt_id)
    }

    fn resume(&mut self, receipt_id: u64, _item: &FrozenSpokenItem) -> Result<String, String> {
        self.resume_calls += 1;
        self.receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| "unknown receipt".into())
    }
}

struct FaultJournal {
    record: Option<JobRecord>,
    fail_once: BTreeSet<&'static str>,
}

impl FaultJournal {
    fn new(stages: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            record: None,
            fail_once: stages.into_iter().collect(),
        }
    }
}

impl JobJournal for FaultJournal {
    fn load(&mut self, request_id: &str) -> Result<Option<JobRecord>, JournalError> {
        Ok(self
            .record
            .as_ref()
            .filter(|job| job.request.request_id == request_id)
            .cloned())
    }

    fn insert(&mut self, job: &JobRecord) -> Result<InsertOutcome, JournalError> {
        if self.record.is_some() {
            Ok(InsertOutcome::AlreadyExists)
        } else {
            self.record = Some(job.clone());
            Ok(InsertOutcome::Inserted)
        }
    }

    fn save(&mut self, job: &JobRecord) -> Result<(), JournalError> {
        if self.fail_once.remove(job.phase.name()) {
            return Err(JournalError::Io(std::io::Error::other(
                "injected process loss",
            )));
        }
        self.record = Some(job.clone());
        Ok(())
    }
}
