#![cfg(unix)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use tempfile::TempDir;
use tts29_daemon::{
    serve_one, AnswerWaitResult, LocalPublishRequest, LocalPublishResponse, LocalPublishService,
    PrivateUnixListener, ProducerRequest, LOCAL_PROTOCOL_VERSION,
};

struct CaptureService(Arc<Mutex<Option<String>>>);
struct ErrorService;

impl LocalPublishService for CaptureService {
    fn publish_local(&mut self, request: LocalPublishRequest) -> LocalPublishResponse {
        *self.0.lock().unwrap() = request.agent_nsec;
        LocalPublishResponse::Published {
            version: LOCAL_PROTOCOL_VERSION,
            request_id: request.request.request_id,
            receipt_id: 17,
            event_id: "b".repeat(64),
            answer_wait: AnswerWaitResult::NotRequested,
        }
    }
}

impl LocalPublishService for ErrorService {
    fn publish_local(&mut self, _request: LocalPublishRequest) -> LocalPublishResponse {
        LocalPublishResponse::error("request_conflict", "request id was reused")
    }
}

#[test]
fn cli_forwards_request_and_ephemeral_signer_to_the_daemon() {
    let temporary = TempDir::new().unwrap();
    let socket = temporary.path().join("runtime/daemon.sock");
    let listener = PrivateUnixListener::bind(&socket).unwrap();
    let observed = Arc::new(Mutex::new(None));
    let service_observed = Arc::clone(&observed);
    let server = std::thread::spawn(move || {
        serve_one(&listener, &mut CaptureService(service_observed)).unwrap()
    });
    let secret = "nsec1ephemeral-cli-signer";
    let mut child = Command::new(env!("CARGO_BIN_EXE_tts29"))
        .args(["--socket", socket.to_str().unwrap()])
        .env_remove("TTS29_SOCKET")
        .env("AGENT_NSEC", secret)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    serde_json::to_writer(child.stdin.as_mut().unwrap(), &request()).unwrap();
    child.stdin.take().unwrap().flush().unwrap();

    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    let response: LocalPublishResponse = serde_json::from_slice(&output.stdout).unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(matches!(
        response,
        LocalPublishResponse::Published { receipt_id: 17, .. }
    ));
    assert_eq!(observed.lock().unwrap().as_deref(), Some(secret));
    assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
}

#[test]
fn cli_preserves_structured_daemon_errors_on_stdout() {
    let temporary = TempDir::new().unwrap();
    let socket = temporary.path().join("runtime/daemon.sock");
    let listener = PrivateUnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || serve_one(&listener, &mut ErrorService).unwrap());
    let mut child = Command::new(env!("CARGO_BIN_EXE_tts29"))
        .args(["--socket", socket.to_str().unwrap()])
        .env_remove("TTS29_SOCKET")
        .env_remove("AGENT_NSEC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    serde_json::to_writer(child.stdin.as_mut().unwrap(), &request()).unwrap();
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    let response: LocalPublishResponse = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(matches!(
        response,
        LocalPublishResponse::Error { code, .. } if code == "request_conflict"
    ));
    assert!(output.stderr.is_empty());
}

fn request() -> ProducerRequest {
    ProducerRequest {
        request_id: "cli-request".into(),
        group_id: "tts".into(),
        voice: "af_heart".into(),
        agent_name: "Codex".into(),
        subject: "CLI proof".into(),
        summary: "The thin CLI reaches the daemon.".into(),
        body: "Publish this request.".into(),
        attachments: Vec::new(),
        questions: Vec::new(),
        attach: None,
    }
}
