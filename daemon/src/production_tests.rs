use std::collections::BTreeSet;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use nmp::{Engine, EngineConfig, ReceiptReattachment, RelayUrl, WriteStatus};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tts29_protocol::{DurableArtifact, FrozenSpokenItem};

use crate::test_http::{TestResponse, TestServer};
use crate::{
    ArtifactUploader, BlossomArtifactUploader, BlossomUploadConfig, Clock, KokoroAuth,
    KokoroConfig, KokoroSynthesizer, LocalAudioArtifact, NmpPublisher, ProducerRequest, Publisher,
    Synthesizer,
};

static TEST_KEY_ID: AtomicU64 = AtomicU64::new(1);

#[test]
fn kokoro_commits_one_deterministic_file_and_reuses_it_after_restart() {
    let server = TestServer::serve(vec![TestResponse {
        status: "200 OK",
        content_type: "audio/mpeg",
        body: b"deterministic-kokoro-mp3".to_vec(),
    }]);
    let mut config = KokoroConfig::new(format!("{}/v1/audio/speech", server.origin));
    config.auth = KokoroAuth::Bearer("private-test-token".into());
    config.allow_insecure_loopback = true;
    let mut synthesizer = KokoroSynthesizer::new(config).unwrap();
    let temporary = TempDir::new().unwrap();
    let output = temporary.path().join("speech.mp3");

    let first = synthesizer.synthesize(&request(), &output).unwrap();
    let repeated = synthesizer.synthesize(&request(), &output).unwrap();
    let requests = server.finish();

    assert_eq!(first, repeated);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/audio/speech");
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("Bearer private-test-token")
    );
    let payload: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(payload["voice"], "af_heart");
    assert_eq!(payload["input"], request().body);
    assert_eq!(fs::read(output).unwrap(), b"deterministic-kokoro-mp3");
}

#[test]
fn blossom_upload_is_signed_by_nmp_and_integrity_checked() {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("speech.mp3");
    let bytes = b"blossom-audio";
    fs::write(&path, bytes).unwrap();
    let digest = format!("{:x}", Sha256::digest(bytes));
    let descriptor = format!(
        r#"{{"url":"https://cdn.example.com/{digest}.mp3","sha256":"{digest}","size":{},"type":"audio/mpeg","uploaded":1700000000}}"#,
        bytes.len()
    );
    let server = TestServer::serve(vec![TestResponse {
        status: "200 OK",
        content_type: "application/json",
        body: descriptor.into_bytes(),
    }]);
    let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
    let (account, secret) = add_test_account(&engine);
    engine
        .set_active_account(Some(account.public_key()))
        .unwrap();
    let mut config = BlossomUploadConfig::new(&server.origin);
    config.allowed_local_hosts = BTreeSet::from(["127.0.0.1".into()]);
    let mut uploader = BlossomArtifactUploader::new(
        Arc::clone(&engine),
        account.public_key(),
        config,
        Arc::new(FixedClock(1_700_000_000_000)),
    )
    .unwrap();

    let durable = uploader
        .make_durable(&LocalAudioArtifact {
            path: path.to_string_lossy().into_owned(),
            sha256: digest.clone(),
            media_type: "audio/mpeg".into(),
            byte_count: bytes.len() as u64,
        })
        .unwrap();
    let requests = server.finish();

    assert_eq!(durable.sha256, digest);
    assert_eq!(durable.url, format!("https://cdn.example.com/{digest}.mp3"));
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].path, "/upload");
    assert_eq!(requests[0].body, bytes);
    assert_eq!(requests[0].headers.get("x-sha-256"), Some(&digest));
    let authorization = requests[0].headers.get("authorization").unwrap();
    assert!(authorization.starts_with("Nostr "));
    assert!(!authorization.contains(&secret));
    engine.shutdown();
}

#[test]
fn nmp_publisher_accepts_only_the_configured_group_and_tracks_the_receipt() {
    let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
    let (account, _) = add_test_account(&engine);
    engine
        .set_active_account(Some(account.public_key()))
        .unwrap();
    let mut publisher = NmpPublisher::new(
        Arc::clone(&engine),
        RelayUrl::parse("wss://relay.example.com").unwrap(),
        "tts".into(),
        Duration::from_millis(10),
    );
    let item = spoken_item(account.public_key().to_hex());

    let receipt_id = publisher.accept(&item).unwrap();
    assert!(receipt_id > 0);
    assert!(matches!(
        engine.reattach_receipt(nmp::ReceiptId(receipt_id)).unwrap(),
        ReceiptReattachment::Attached(_, _)
    ));
    let mut wrong_group = item;
    wrong_group.group_id = "another-group".into();
    assert!(publisher.accept(&wrong_group).is_err());
    engine.shutdown();
}

#[test]
fn request_identity_signs_one_item_without_replacing_the_active_daemon() {
    let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
    let (daemon, _) = add_test_account(&engine);
    let (agent, _) = add_test_account(&engine);
    let daemon_author = daemon.public_key();
    let agent_author = agent.public_key();
    engine.set_active_account(Some(daemon_author)).unwrap();
    let mut publisher = NmpPublisher::new(
        Arc::clone(&engine),
        RelayUrl::parse("wss://relay.example.com").unwrap(),
        "tts".into(),
        Duration::from_millis(10),
    );

    let receipt_id = publisher
        .accept(&spoken_item(agent_author.to_hex()))
        .unwrap();
    let ReceiptReattachment::Attached(_, statuses) =
        engine.reattach_receipt(nmp::ReceiptId(receipt_id)).unwrap()
    else {
        panic!("tracked request receipt must be reattachable")
    };
    let signed = (0..16).find_map(
        |_| match statuses.recv_timeout(Duration::from_millis(250)) {
            Ok(WriteStatus::Signed(event_id)) => Some(event_id),
            Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => None,
        },
    );

    assert!(
        signed.is_some(),
        "request identity must sign the frozen item"
    );
    assert_eq!(engine.active_account().unwrap(), Some(daemon_author));
    assert!(engine.remove_account(&agent).unwrap());
    engine.shutdown();
}

struct FixedClock(u64);

impl Clock for FixedClock {
    fn unix_millis(&self) -> u64 {
        self.0
    }
}

fn request() -> ProducerRequest {
    ProducerRequest {
        request_id: "production".into(),
        group_id: "tts".into(),
        voice: "af_heart".into(),
        agent_name: "Codex".into(),
        subject: "Production".into(),
        summary: "Production capability boundary".into(),
        body: "Turn this request into durable speech.".into(),
        attachments: Vec::new(),
        questions: Vec::new(),
    }
}

fn spoken_item(author: String) -> FrozenSpokenItem {
    FrozenSpokenItem {
        author,
        created_at: 1_700_000_000,
        group_id: "tts".into(),
        agent_name: "Codex".into(),
        subject: "Publication".into(),
        summary: "NMP publication boundary".into(),
        body: "NMP accepts the frozen item.".into(),
        audio: DurableArtifact {
            url: "https://cdn.example.com/audio.mp3".into(),
            sha256: "a".repeat(64),
            media_type: "audio/mpeg".into(),
            byte_count: 10,
            label: None,
        },
        attachments: Vec::new(),
        questions: Vec::new(),
    }
}

fn add_test_account(engine: &Engine) -> (nmp::AccountRegistration, String) {
    loop {
        let id = TEST_KEY_ID.fetch_add(1, Ordering::Relaxed);
        let secret = format!(
            "{:x}",
            Sha256::digest(format!("tts29-production-test-{}-{id}", std::process::id()))
        );
        if let Ok(registration) = engine.add_account(&secret) {
            return (registration, secret);
        }
    }
}
