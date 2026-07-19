use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tts29_protocol::DurableArtifact;

use crate::test_publisher::FakePublisher;
use crate::{
    ArtifactUploader, FileJobJournal, InsertOutcome, JobJournal, JobRecord, JournalError,
    LocalAudioArtifact, ProducerError, ProducerRequest, ProducerRunner, Synthesizer,
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
        "authorization_accepted",
        "author_authorized",
        "publication_accepted",
        "published",
    ]);
    let mut runner = runner(journal, temporary.path().join("work"));
    runner.admit(request("recover"), author(), 123).unwrap();

    for _ in 0..16 {
        let _ = runner.advance("recover");
    }
    let (mut journal, synthesizer, uploader, publisher) = runner.into_parts();
    let job = journal.load("recover").unwrap().unwrap();

    assert!(job.phase.is_published());
    let crate::JobPhase::Published {
        membership: Some(membership),
        ..
    } = &job.phase
    else {
        panic!("published jobs must retain membership evidence")
    };
    assert_eq!(membership.receipt_id, Some(7));
    assert!(publisher
        .membership_event_ids
        .contains(&membership.event_id));
    assert_eq!(synthesizer.calls, 2);
    assert_eq!(synthesizer.generated_paths.len(), 1);
    assert_eq!(uploader.calls, 2);
    assert_eq!(uploader.blobs.len(), 1);
    assert_eq!(publisher.authorize_calls, 2);
    assert_eq!(publisher.authorization_resume_calls, 2);
    assert_eq!(publisher.membership_event_ids.len(), 1);
    assert_eq!(publisher.accept_calls, 2);
    assert_eq!(publisher.resume_calls, 2);
    assert_eq!(publisher.event_ids.len(), 1);
}

#[test]
fn existing_membership_skips_the_admin_write() {
    let temporary = TempDir::new().unwrap();
    let journal = FileJobJournal::open(temporary.path().join("jobs")).unwrap();
    let mut publisher = FakePublisher::default();
    publisher.already_authorized = true;
    let mut runner = ProducerRunner::new(
        journal,
        FakeSynthesizer::default(),
        FakeUploader::default(),
        publisher,
        temporary.path().join("work"),
    )
    .unwrap();
    runner.admit(request("member"), author(), 123).unwrap();

    for _ in 0..5 {
        runner.advance("member").unwrap();
    }
    let (_, _, _, publisher) = runner.into_parts();

    assert_eq!(publisher.authorize_calls, 1);
    assert_eq!(publisher.authorization_resume_calls, 0);
    assert_eq!(publisher.accept_calls, 1);
}

#[test]
fn membership_rejection_prevents_spoken_publication() {
    let temporary = TempDir::new().unwrap();
    let journal = FileJobJournal::open(temporary.path().join("jobs")).unwrap();
    let mut publisher = FakePublisher::default();
    publisher.reject_authorization = true;
    let mut runner = ProducerRunner::new(
        journal,
        FakeSynthesizer::default(),
        FakeUploader::default(),
        publisher,
        temporary.path().join("work"),
    )
    .unwrap();
    runner.admit(request("rejected"), author(), 123).unwrap();
    runner.advance("rejected").unwrap();
    runner.advance("rejected").unwrap();

    let error = runner.advance("rejected").unwrap_err();
    let (_, _, _, publisher) = runner.into_parts();

    assert!(matches!(
        error,
        ProducerError::Capability {
            stage: "membership_authorization",
            ..
        }
    ));
    assert_eq!(publisher.accept_calls, 0);
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
    attach: None,
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
