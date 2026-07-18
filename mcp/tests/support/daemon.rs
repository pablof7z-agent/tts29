use std::io::Read;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use tts29_producer_api::{LocalPublishRequest, LocalPublishResponse};

pub struct FakeDaemon {
    request: Arc<Mutex<Receiver<LocalPublishRequest>>>,
    response: Sender<LocalPublishResponse>,
    task: std::thread::JoinHandle<()>,
}

impl FakeDaemon {
    pub fn start(socket: PathBuf) -> Self {
        let listener = UnixListener::bind(socket).unwrap();
        let (request_tx, request) = std::sync::mpsc::channel();
        let (response, response_rx) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            let request = serde_json::from_slice(&bytes).unwrap();
            request_tx.send(request).unwrap();
            let response = response_rx.recv().unwrap();
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
        });
        Self {
            request: Arc::new(Mutex::new(request)),
            response,
            task,
        }
    }

    pub async fn received(&self) -> LocalPublishRequest {
        let request = Arc::clone(&self.request);
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || request.lock().unwrap().recv().unwrap()),
        )
        .await
        .unwrap()
        .unwrap()
    }

    pub async fn respond(self, response: LocalPublishResponse) {
        tokio::task::spawn_blocking(move || {
            self.response.send(response).unwrap();
            self.task.join().unwrap();
        })
        .await
        .unwrap();
    }
}
