use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nmp::{Engine, EngineConfig, Kind, RelayUrl, SignEventRequest, Tag, Timestamp};
use serde_json::{json, Value};
use tungstenite::{accept, Message};

use crate::{AuthorizationStep, NmpPublisher, Publisher};

#[test]
fn membership_repair_uses_the_daemon_and_selected_nmp_host() {
    let relay = TestNmpRelay::start(true);
    let (engine, daemon_author) = engine_for();
    seed_group(&engine, &relay, &"4".repeat(64));
    let member_author = "2".repeat(64);
    let mut publisher = NmpPublisher::new(
        Arc::clone(&engine),
        relay.url.clone(),
        "tts".into(),
        daemon_author,
        Duration::from_secs(5),
    );

    let AuthorizationStep::Accepted { receipt_id } = publisher
        .authorize("membership-repair", &member_author, 1_700_000_000)
        .unwrap()
    else {
        panic!("an empty reconciled group must require a membership write")
    };
    let repeated = publisher
        .authorize("membership-repair", &member_author, 1_700_000_000)
        .unwrap();
    assert_eq!(
        repeated,
        AuthorizationStep::Accepted { receipt_id },
        "stable NMP correlation must recover the accepted obligation"
    );
    let event_id = publisher
        .resume_authorization(receipt_id, &member_author)
        .unwrap();
    let event = relay.event.recv_timeout(Duration::from_secs(5)).unwrap();

    assert_eq!(event["id"], event_id);
    assert_eq!(event["kind"], 9_000);
    assert_eq!(event["pubkey"], daemon_author.to_hex());
    assert!(has_tag(&event, "h", "tts"));
    assert!(has_tag(&event, "p", &member_author));
    engine.shutdown();
    relay.finish();
}

#[test]
fn selected_host_rejection_is_a_terminal_membership_failure() {
    let relay = TestNmpRelay::start(false);
    let (engine, daemon_author) = engine_for();
    seed_group(&engine, &relay, &"4".repeat(64));
    let mut publisher = NmpPublisher::new(
        Arc::clone(&engine),
        relay.url.clone(),
        "tts".into(),
        daemon_author,
        Duration::from_secs(5),
    );
    let member_author = "3".repeat(64);

    let AuthorizationStep::Accepted { receipt_id } = publisher
        .authorize("membership-rejected", &member_author, 1_700_000_000)
        .unwrap()
    else {
        panic!("an empty reconciled group must require a membership write")
    };
    let error = publisher
        .resume_authorization(receipt_id, &member_author)
        .unwrap_err();

    assert!(error.contains("rejected by"));
    engine.shutdown();
    relay.finish();
}

#[test]
fn current_member_state_skips_the_membership_write() {
    let relay = TestNmpRelay::start(true);
    let (engine, daemon_author) = engine_for();
    let member_author = "5".repeat(64);
    let state_event_id = seed_group(&engine, &relay, &member_author);
    let mut publisher = NmpPublisher::new(
        Arc::clone(&engine),
        relay.url.clone(),
        "tts".into(),
        daemon_author,
        Duration::from_secs(5),
    );

    let authorization = publisher
        .authorize("existing-member", &member_author, 1_700_000_000)
        .unwrap();

    assert_eq!(
        authorization,
        AuthorizationStep::Authorized {
            event_id: state_event_id
        }
    );
    assert!(relay.event.try_recv().is_err());
    engine.shutdown();
    relay.finish();
}

fn engine_for() -> (Arc<Engine>, nmp::PublicKey) {
    let engine = Arc::new(
        Engine::new(EngineConfig {
            allowed_local_relay_hosts: vec!["127.0.0.1".into()],
            ..EngineConfig::default()
        })
        .unwrap(),
    );
    let account = engine.add_account(&"1".repeat(64)).unwrap();
    let author = account.public_key();
    engine.set_active_account(Some(author)).unwrap();
    (engine, author)
}

fn seed_group(engine: &Engine, relay: &TestNmpRelay, member: &str) -> String {
    let event = engine
        .sign_event(SignEventRequest {
            created_at: Timestamp::from(1_699_999_999u64),
            kind: Kind::from(39_002u16),
            tags: vec![
                Tag::parse(["d", "tts"]).unwrap(),
                Tag::parse(["p", member]).unwrap(),
            ],
            content: String::new(),
        })
        .unwrap()
        .recv()
        .unwrap();
    let event_id = event.id.to_hex();
    relay
        .fixture
        .send(serde_json::to_value(event).unwrap())
        .unwrap();
    event_id
}

fn has_tag(event: &Value, name: &str, value: &str) -> bool {
    event["tags"].as_array().is_some_and(|tags| {
        tags.iter().any(|tag| {
            tag.as_array().is_some_and(|parts| {
                parts.first() == Some(&Value::String(name.into()))
                    && parts.get(1) == Some(&Value::String(value.into()))
            })
        })
    })
}

struct TestNmpRelay {
    url: RelayUrl,
    address: SocketAddr,
    fixture: Sender<Value>,
    event: Receiver<Value>,
    thread: JoinHandle<()>,
}

impl TestNmpRelay {
    fn start(accept_write: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let url = RelayUrl::parse(&format!("ws://{address}")).unwrap();
        let (fixture, fixture_rx) = mpsc::channel();
        let (event_tx, event) = mpsc::channel();
        let thread = thread::spawn(move || {
            let fixture_rx = Arc::new(Mutex::new(fixture_rx));
            let mut connections = Vec::new();
            loop {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 2_048];
                let count = stream.peek(&mut request).unwrap();
                if request[..count].starts_with(b"SHUTDOWN") {
                    break;
                }
                let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
                if !request.contains("upgrade: websocket") {
                    respond_nip11(&mut stream);
                    continue;
                }
                let fixtures = Arc::clone(&fixture_rx);
                let events = event_tx.clone();
                connections.push(thread::spawn(move || {
                    handle_connection(stream, fixtures, events, accept_write)
                }));
            }
            for connection in connections {
                connection.join().unwrap();
            }
        });
        Self {
            url,
            address,
            fixture,
            event,
            thread,
        }
    }

    fn finish(self) {
        TcpStream::connect(self.address)
            .unwrap()
            .write_all(b"SHUTDOWN")
            .unwrap();
        self.thread.join().unwrap();
    }
}

fn respond_nip11(stream: &mut TcpStream) {
    let mut request = [0u8; 2_048];
    let _ = stream.read(&mut request);
    let body = r#"{"supported_nips":[29]}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/nostr+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).unwrap();
}

fn handle_connection(
    stream: TcpStream,
    fixtures: Arc<Mutex<Receiver<Value>>>,
    events: Sender<Value>,
    accept_write: bool,
) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut socket = accept(stream).unwrap();
    while let Ok(Message::Text(text)) = socket.read() {
        let message: Value = serde_json::from_str(text.as_str()).unwrap();
        match message[0].as_str() {
            Some("REQ") => {
                let membership_request = message[2]["kinds"]
                    .as_array()
                    .is_some_and(|kinds| kinds.contains(&json!(39_002)));
                if membership_request {
                    if let Ok(event) = fixtures.lock().unwrap().try_recv() {
                        socket
                            .send(Message::text(
                                json!(["EVENT", message[1], event]).to_string(),
                            ))
                            .unwrap();
                    }
                }
                socket
                    .send(Message::text(json!(["EOSE", message[1]]).to_string()))
                    .unwrap();
            }
            Some("EVENT") => {
                let event = message[1].clone();
                let event_id = event["id"].clone();
                events.send(event).unwrap();
                let reason = if accept_write {
                    ""
                } else {
                    "blocked: not authorized"
                };
                socket
                    .send(Message::text(
                        json!(["OK", event_id, accept_write, reason]).to_string(),
                    ))
                    .unwrap();
            }
            Some("NEG-OPEN") => {
                socket
                    .send(Message::text(
                        json!(["NEG-ERR", message[1], "unsupported"]).to_string(),
                    ))
                    .unwrap();
            }
            _ => {}
        }
    }
}
