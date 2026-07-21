use std::num::NonZeroUsize;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use nmp::{Engine, EngineConfig, Frame, LiveQuery, RelayUrl, Window, WriteStatus};
use nmp_nip29::group_content_demand;

use crate::action::AppAction;
use crate::clock::Clock;
use crate::model::{KernelConfiguration, KernelPhase, QueueSnapshot};
use crate::session::Session;
use crate::{Control, Emitter};

pub(crate) enum RuntimeEvent {
    Action(AppAction),
    ActionError(String),
    Login {
        secret: String,
        persist: bool,
    },
    CredentialLoadFailed(String),
    CredentialResult {
        request_id: u64,
        succeeded: bool,
        error: Option<String>,
    },
    Frame(Frame),
    QueryClosed,
    ReceiptStatus {
        item_id: String,
        receipt_id: u64,
        status: WriteStatus,
    },
    ReceiptClosed {
        item_id: String,
        receipt_id: u64,
    },
    Stop,
}

pub fn run(
    configuration: KernelConfiguration,
    emitter: Emitter,
    control: Arc<Control>,
    receiver: Receiver<RuntimeEvent>,
    sender: Sender<RuntimeEvent>,
    clock: Arc<dyn Clock>,
) {
    emitter.emit(&QueueSnapshot::lifecycle(
        &configuration,
        KernelPhase::Starting,
    ));
    if let Err(error) = run_inner(&configuration, &emitter, &control, receiver, sender, clock) {
        emitter.emit(&QueueSnapshot::failed(&configuration, error));
    }
    emitter.emit(&QueueSnapshot::lifecycle(
        &configuration,
        KernelPhase::Stopped,
    ));
}

fn run_inner(
    configuration: &KernelConfiguration,
    emitter: &Emitter,
    control: &Arc<Control>,
    receiver: Receiver<RuntimeEvent>,
    sender: Sender<RuntimeEvent>,
    clock: Arc<dyn Clock>,
) -> Result<(), String> {
    let engine = Arc::new(
        Engine::new(EngineConfig {
            store_path: configuration.store_path.clone(),
            ..EngineConfig::default()
        })
        .map_err(|error| format!("NMP engine refused startup: {error}"))?,
    );
    let host = RelayUrl::parse(&configuration.relay)
        .map_err(|_| "The configured NIP-29 host is invalid.".to_string())?;
    let demand = group_content_demand(host.clone(), &configuration.group_id);
    let window = Window::Expandable {
        initial: NonZeroUsize::new(40).expect("nonzero initial window"),
        max: NonZeroUsize::new(100).expect("nonzero maximum window"),
    };
    let subscription = engine
        .observe(LiveQuery(demand), Some(window))
        .map_err(|error| format!("NMP refused the group observation: {error}"))?;
    if !control.install(subscription.cancel_handle()) {
        subscription.cancel();
        engine.shutdown();
        return Ok(());
    }

    let query_sender = sender.clone();
    let query_thread = std::thread::Builder::new()
        .name("tts29-query".into())
        .spawn(move || {
            while let Ok(frame) = subscription.recv() {
                if query_sender.send(RuntimeEvent::Frame(frame)).is_err() {
                    return;
                }
            }
            let _ = query_sender.send(RuntimeEvent::QueryClosed);
        })
        .map_err(|error| format!("TTS29 could not start its query observer: {error}"))?;

    let mut session = Session::new(configuration.clone(), host, Arc::clone(&engine), clock);
    let mut receipt_threads = Vec::new();
    emitter.emit(&session.snapshot());
    while let Ok(event) = receiver.recv() {
        match event {
            RuntimeEvent::Frame(frame) => {
                if let Some(contents) = frame.window {
                    session.update_rows(contents.rows, frame.evidence);
                    emitter.emit(&session.snapshot());
                }
            }
            RuntimeEvent::Login { secret, persist } => {
                session.login(&secret, persist);
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::CredentialResult {
                request_id,
                succeeded,
                error,
            } => {
                session.credential_result(request_id, succeeded, error);
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::CredentialLoadFailed(error) => {
                session.credential_load_failed(error);
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::Action(AppAction::Logout) => {
                session.logout();
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::Action(AppAction::SubmitAnswer { item_id, answers }) => {
                match session.submit_answer(&item_id, answers) {
                    Ok(receipt) => {
                        let receipt_id = receipt.id.0;
                        receipt_threads.push(forward_receipt(
                            item_id,
                            receipt_id,
                            receipt.statuses,
                            sender.clone(),
                        )?);
                    }
                    Err(error) => session.action_error(Some(&item_id), error),
                }
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::ActionError(error) => {
                session.action_error(None, error);
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::ReceiptStatus {
                item_id,
                receipt_id,
                status,
            } => {
                session.receipt_status(&item_id, receipt_id, status);
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::ReceiptClosed {
                item_id,
                receipt_id,
            } => {
                session.receipt_closed(&item_id, receipt_id);
                emitter.emit(&session.snapshot());
            }
            RuntimeEvent::QueryClosed if !control.is_stopping() => {
                session.action_error(None, "The NMP group observation closed.".into());
                emitter.emit(&session.snapshot());
                break;
            }
            RuntimeEvent::QueryClosed => {}
            RuntimeEvent::Stop => break,
        }
    }

    control.cancel_observation();
    session.shutdown();
    engine.shutdown();
    join(query_thread);
    for thread in receipt_threads {
        join(thread);
    }
    Ok(())
}

fn forward_receipt(
    item_id: String,
    receipt_id: u64,
    statuses: std::sync::mpsc::Receiver<WriteStatus>,
    sender: Sender<RuntimeEvent>,
) -> Result<JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("tts29-answer-receipt".into())
        .spawn(move || {
            while let Ok(status) = statuses.recv() {
                if sender
                    .send(RuntimeEvent::ReceiptStatus {
                        item_id: item_id.clone(),
                        receipt_id,
                        status,
                    })
                    .is_err()
                {
                    return;
                }
            }
            let _ = sender.send(RuntimeEvent::ReceiptClosed {
                item_id,
                receipt_id,
            });
        })
        .map_err(|error| format!("TTS29 could not observe the answer receipt: {error}"))
}

fn join(thread: JoinHandle<()>) {
    let _ = thread.join();
}
