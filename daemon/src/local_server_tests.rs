use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use tempfile::TempDir;

use crate::{
    serve_one, serve_until_shutdown, submit_local, AnswerWaitResult, LocalPublishRequest,
    LocalPublishResponse, LocalPublishService, LocalServerShutdown, PrivateUnixListener,
    ProducerRequest, LOCAL_PROTOCOL_VERSION, MAX_LOCAL_FRAME_BYTES,
};

struct CaptureService(Arc<Mutex<Option<String>>>);

impl LocalPublishService for CaptureService {
    fn publish_local(&mut self, request: LocalPublishRequest) -> LocalPublishResponse {
        *self.0.lock().unwrap() = request.agent_nsec;
        LocalPublishResponse::Published {
            version: LOCAL_PROTOCOL_VERSION,
            request_id: request.request.request_id,
            receipt_id: 9,
            event_id: "a".repeat(64),
            answer_wait: AnswerWaitResult::NotRequested,
        }
    }
}

#[test]
fn private_socket_carries_but_never_returns_request_signer() {
    let temporary = TempDir::new().unwrap();
    let socket = temporary.path().join("runtime/daemon.sock");
    let listener = PrivateUnixListener::bind(&socket).unwrap();
    assert_eq!(
        fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let observed = Arc::new(Mutex::new(None));
    let service_observed = Arc::clone(&observed);
    let server = std::thread::spawn(move || {
        serve_one(&listener, &mut CaptureService(service_observed)).unwrap()
    });
    let secret = "nsec1request-only";

    let response = submit_local(&socket, &request(Some(secret.into()))).unwrap();
    server.join().unwrap();

    assert_eq!(observed.lock().unwrap().as_deref(), Some(secret));
    assert!(!serde_json::to_string(&response).unwrap().contains(secret));
}

#[test]
fn oversized_frame_is_rejected_without_reaching_the_service() {
    let temporary = TempDir::new().unwrap();
    let socket = temporary.path().join("runtime/daemon.sock");
    let listener = PrivateUnixListener::bind(&socket).unwrap();
    let observed = Arc::new(Mutex::new(None));
    let service_observed = Arc::clone(&observed);
    let server = std::thread::spawn(move || {
        serve_one(&listener, &mut CaptureService(service_observed)).unwrap()
    });
    let mut stream = UnixStream::connect(&socket).unwrap();
    stream
        .write_all(&vec![b'x'; MAX_LOCAL_FRAME_BYTES + 1])
        .unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();

    let response: LocalPublishResponse = serde_json::from_slice(&bytes).unwrap();
    server.join().unwrap();

    assert!(matches!(
        response,
        LocalPublishResponse::Error { code, .. } if code == "request_too_large"
    ));
    assert!(observed.lock().unwrap().is_none());
}

#[test]
fn stale_socket_is_replaced_but_a_regular_file_is_preserved() {
    let temporary = TempDir::new().unwrap();
    let runtime = temporary.path().join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let socket = runtime.join("daemon.sock");
    std::os::unix::net::UnixListener::bind(&socket).unwrap();

    let listener = PrivateUnixListener::bind(&socket).unwrap();
    drop(listener);
    assert!(!socket.exists());

    fs::write(&socket, b"do not delete").unwrap();
    assert_eq!(
        PrivateUnixListener::bind(&socket).err().unwrap().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&socket).unwrap(), b"do not delete");
}

#[test]
fn shutdown_wakes_the_listener_without_dispatching_a_request() {
    let temporary = TempDir::new().unwrap();
    let socket = temporary.path().join("runtime/daemon.sock");
    let listener = PrivateUnixListener::bind(&socket).unwrap();
    let shutdown = LocalServerShutdown::new(&socket);
    let signal = shutdown.clone();
    let observed = Arc::new(Mutex::new(None));
    let service_observed = Arc::clone(&observed);
    let server = std::thread::spawn(move || {
        serve_until_shutdown(&listener, &mut CaptureService(service_observed), &shutdown).unwrap()
    });

    signal.request();
    server.join().unwrap();

    assert!(observed.lock().unwrap().is_none());
}

fn request(agent_nsec: Option<String>) -> LocalPublishRequest {
    LocalPublishRequest {
        version: LOCAL_PROTOCOL_VERSION,
        request: ProducerRequest {
            request_id: "local-request".into(),
            group_id: "tts".into(),
            voice: "af_heart".into(),
            agent_name: "Codex".into(),
            subject: "Local publication".into(),
            summary: "The local contract is bounded.".into(),
            body: "Publish this through the daemon.".into(),
            attachments: Vec::new(),
            questions: Vec::new(),
        },
        wait_for_answer_seconds: None,
        agent_nsec,
    }
}
