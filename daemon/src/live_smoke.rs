use std::path::Path;
use std::time::Duration;

use nmp::RelayUrl;
use rand_core::{OsRng, RngCore};
use serde::Serialize;
use tts29_protocol::{ParsedEvent, Question, QuestionAnswer, QuestionKind, QuestionOption};

use crate::live_network::{
    observe_exact, publish_answer, unix_seconds, verify_audio, AudioEvidence, SourceEvidence,
};
use crate::{
    load_daemon_config, serve_one, submit_local_tree, submit_local_with_timeout, AnswerWaitResult,
    LocalPublishRequest, LocalPublishResponse, LocalTreeRequest, PrivateUnixListener,
    ProducerRequest, ProductionProducer, LOCAL_PROTOCOL_VERSION,
};

const PRODUCER_TIMEOUT: Duration = Duration::from_secs(240);

#[derive(Debug, Serialize)]
pub struct LiveRelayEvidence {
    pub relay: String,
    pub group_id: String,
    pub request_id: String,
    pub group_creation_receipt_id: Option<u64>,
    pub group_creation_event_id: Option<String>,
    pub item_receipt_id: u64,
    pub item_event_id: String,
    pub item_author: String,
    pub item_source: SourceEvidence,
    pub audio: AudioEvidence,
    pub answer_receipt_id: u64,
    pub answer_event_id: String,
    pub answer_root_id: String,
    pub answer_author: String,
    pub answer_source: SourceEvidence,
}

pub fn run_live_relay_smoke(path: impl AsRef<Path>) -> Result<LiveRelayEvidence, String> {
    let mut loaded = load_daemon_config(path)?;
    let relay = RelayUrl::parse(&loaded.production.host)
        .map_err(|_| "live relay URL is invalid".to_string())?;
    let relay_text = relay.to_string();
    let group_id = loaded.production.group_id.clone();
    let creates_group = std::env::var("TTS29_LIVE_CREATE_GROUP").as_deref() == Ok("1");
    if creates_group {
        let daemon_secret = disposable_secret();
        loaded.production.owner_pubkey = public_key_for(&daemon_secret)?;
        loaded.production.secret_key_override = Some(daemon_secret);
    }
    let request_secret = disposable_secret();
    let request_id = format!("live-e2e-{}", unix_seconds()?);
    let request = live_request(&request_id, &group_id);
    let local = LocalPublishRequest {
        version: LOCAL_PROTOCOL_VERSION,
        request,
        wait_for_answer_seconds: None,
        agent_nsec: Some(request_secret.clone()),
    };

    let mut producer = ProductionProducer::open(loaded.production)?;
    let group_creation_receipt_id = producer.bootstrap_evidence().group_creation_receipt_id;
    let group_creation_event_id = producer.bootstrap_evidence().group_created_event_id.clone();
    let listener = PrivateUnixListener::bind(&loaded.socket_path)
        .map_err(|error| format!("live daemon socket could not bind: {error}"))?;
    let socket_path = listener.path().to_path_buf();
    let client = std::thread::spawn(move || {
        submit_local_with_timeout(socket_path, &local, PRODUCER_TIMEOUT)
    });
    let server_result = serve_one(&listener, &mut producer);
    producer.shutdown();
    server_result.map_err(|error| format!("live daemon request failed: {error}"))?;
    let response = client
        .join()
        .map_err(|_| "live daemon client panicked".to_string())?
        .map_err(|error| format!("live daemon client failed: {error}"))?;
    let (item_receipt_id, item_event_id) = published(response)?;

    let item_observation = observe_exact(&relay, &group_id, &item_event_id)?;
    let ParsedEvent::Item(item) = item_observation.parsed else {
        return Err("live relay returned the item id with the wrong TTS29 type".into());
    };
    let audio = verify_audio(&item)?;
    let (answer_receipt_id, answer_event_id, answer_author) =
        publish_answer(&relay, &group_id, &item_event_id, &request_secret)?;
    let answer_observation = observe_exact(&relay, &group_id, &answer_event_id)?;
    let ParsedEvent::Answer(answer) = answer_observation.parsed else {
        return Err("live relay returned the answer id with the wrong TTS29 type".into());
    };
    if answer.root_id != item_event_id
        || answer.value.answers
            != [QuestionAnswer {
                question_id: "live-e2e".into(),
                values: vec!["confirmed".into()],
            }]
    {
        return Err("live answer did not preserve the expected item relationship".into());
    }

    Ok(LiveRelayEvidence {
        relay: relay_text,
        group_id,
        request_id,
        group_creation_receipt_id,
        group_creation_event_id,
        item_receipt_id,
        item_event_id,
        item_author: item.author.clone(),
        item_source: item_observation.source,
        audio,
        answer_receipt_id,
        answer_event_id,
        answer_root_id: answer.root_id,
        answer_author,
        answer_source: answer_observation.source,
    })
}

#[derive(Debug, Serialize)]
pub struct TreeSmokeEvidence {
    pub relay: String,
    pub group_id: String,
    pub root_event_id: String,
    pub child_event_ids: Vec<String>,
}

/// Publishes a real narrated-attachment tree through the daemon: a root message
/// with a nested child and grandchild, each synthesized, uploaded, and
/// published parent-first. Points at the configured (already-existing) group.
pub fn run_tree_relay_smoke(path: impl AsRef<Path>) -> Result<TreeSmokeEvidence, String> {
    use tts29_producer_api::{SpokenTree, TreeAttachment};

    let mut loaded = load_daemon_config(path)?;
    let relay = loaded.production.host.clone();
    let stamp = unix_seconds()?;
    // Create a fresh group owned by the daemon identity so it can authorize
    // membership and publish every node.
    let group_id =
        std::env::var("TTS29_TREE_GROUP").unwrap_or_else(|_| format!("tts29-tree-{stamp}"));
    loaded.production.group_id = group_id.clone();
    let daemon_secret = disposable_secret();
    loaded.production.owner_pubkey = public_key_for(&daemon_secret)?;
    loaded.production.secret_key_override = Some(daemon_secret);
    let dir = std::env::temp_dir().join(format!("tts29-tree-{stamp}"));
    std::fs::create_dir_all(&dir).map_err(|error| format!("tree temp dir failed: {error}"))?;
    let write = |name: &str, body: &str| -> Result<String, String> {
        let file = dir.join(name);
        std::fs::write(&file, body)
            .map_err(|error| format!("tree message write failed: {error}"))?;
        file.to_str()
            .map(str::to_string)
            .ok_or_else(|| "tree message path is not UTF-8".to_string())
    };
    let root = write(
        "root.md",
        "Daemon-published proposal. Open the [Detailed explanation](attachment:).",
    )?;
    let child = write(
        "child.md",
        "Here is the detailed explanation. See the [Further note](attachment:).",
    )?;
    let grandchild = write("gc.md", "This is the further note, one level deeper.")?;

    let tree = SpokenTree {
        request_id: format!("tree-{stamp}"),
        group_id: group_id.clone(),
        title: "Daemon proposal".into(),
        summary: Some("Published as a tree by the daemon.".into()),
        message: root,
        questions: Vec::new(),
        attachments: vec![TreeAttachment::Narrated {
            label: "Detailed explanation".into(),
            message: child,
            questions: Vec::new(),
            attachments: vec![TreeAttachment::Narrated {
                label: "Further note".into(),
                message: grandchild,
                questions: Vec::new(),
                attachments: Vec::new(),
            }],
        }],
    };

    // Drive the full CLI → socket → daemon path, signing as a disposable agent.
    let request_secret = disposable_secret();
    let local = LocalTreeRequest {
        version: LOCAL_PROTOCOL_VERSION,
        tree,
        agent_id: Some("indigo-claude".into()),
        agent_nsec: Some(request_secret),
    };
    let mut producer = ProductionProducer::open(loaded.production)?;
    let listener = PrivateUnixListener::bind(&loaded.socket_path)
        .map_err(|error| format!("tree daemon socket could not bind: {error}"))?;
    let socket_path = listener.path().to_path_buf();
    let client = std::thread::spawn(move || submit_local_tree(socket_path, &local));
    let server_result = serve_one(&listener, &mut producer);
    producer.shutdown();
    server_result.map_err(|error| format!("tree daemon request failed: {error}"))?;
    let response = client
        .join()
        .map_err(|_| "tree daemon client panicked".to_string())?
        .map_err(|error| format!("tree daemon client failed: {error}"))?;
    let _ = std::fs::remove_dir_all(&dir);
    let (root_event_id, child_event_ids) = match response {
        LocalPublishResponse::PublishedTree {
            root_event_id,
            child_event_ids,
            ..
        } => (root_event_id, child_event_ids),
        LocalPublishResponse::Error { code, message, .. } => {
            return Err(format!(
                "tree daemon refused publication ({code}): {message}"
            ));
        }
        LocalPublishResponse::Published { .. } => {
            return Err("tree daemon returned a single-item response for a tree".into());
        }
    };
    Ok(TreeSmokeEvidence {
        relay,
        group_id,
        root_event_id,
        child_event_ids,
    })
}

fn disposable_secret() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn public_key_for(secret: &str) -> Result<String, String> {
    let engine = nmp::Engine::new(nmp::EngineConfig::default())
        .map_err(|error| format!("disposable NMP identity engine failed: {error}"))?;
    let account = engine
        .add_account(secret)
        .map_err(|error| format!("disposable NMP identity was refused: {error}"))?;
    let public_key = account.public_key().to_hex();
    engine.shutdown();
    Ok(public_key)
}

fn published(response: LocalPublishResponse) -> Result<(u64, String), String> {
    match response {
        LocalPublishResponse::Published {
            receipt_id,
            event_id,
            answer_wait: AnswerWaitResult::NotRequested,
            ..
        } => Ok((receipt_id, event_id)),
        LocalPublishResponse::Published { .. } => {
            Err("live daemon returned an unexpected answer-wait state".into())
        }
        LocalPublishResponse::PublishedTree { .. } => {
            Err("live daemon returned a tree response for a single item".into())
        }
        LocalPublishResponse::Error { code, message, .. } => Err(format!(
            "live daemon refused publication ({code}): {message}"
        )),
    }
}

fn live_request(request_id: &str, group_id: &str) -> ProducerRequest {
    ProducerRequest {
        request_id: request_id.into(),
        group_id: group_id.into(),
        voice: std::env::var("TTS29_LIVE_VOICE").unwrap_or_else(|_| "af_heart".into()),
        agent_name: "TTS29 live smoke".into(),
        subject: format!("Live relay E2E {request_id}"),
        summary: "Published, reacquired, and played through the real TTS29 path.".into(),
        body: "This is a bounded TTS29 live relay end-to-end verification item.".into(),
        attachments: Vec::new(),
        questions: vec![Question {
            id: "live-e2e".into(),
            kind: QuestionKind::SingleChoice,
            short_title: "Confirmed?".into(),
            title: "Did the second live client confirm this item?".into(),
            description: Some("The live verifier publishes one related answer.".into()),
            options: vec![QuestionOption {
                id: "confirmed".into(),
                title: "Confirmed".into(),
                description: None,
            }],
        }],
        attach: None,
    }
}
