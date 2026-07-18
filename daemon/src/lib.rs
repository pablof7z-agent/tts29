mod answer_wait;
mod blossom;
mod daemon_config;
mod identity;
mod journal;
mod kokoro;
mod local_protocol;
#[cfg(unix)]
mod local_server;
mod local_service;
mod model;
mod nmp_publisher;
mod production;
mod runner;

pub use answer_wait::{AnswerWaitCancel, AnswerWaitError, AnswerWaiter};
pub use blossom::{BlossomArtifactUploader, BlossomUploadConfig, Clock, SystemClock};
pub use daemon_config::{load_daemon_config, LoadedDaemonConfig};
pub use journal::{FileJobJournal, InsertOutcome, JobJournal, JournalError};
pub use kokoro::{KokoroAuth, KokoroConfig, KokoroSynthesizer};
pub use local_protocol::{
    AnswerWaitResult, LocalPublishRequest, LocalPublishResponse, LocalRequestError,
    LOCAL_PROTOCOL_VERSION, MAX_ANSWER_WAIT_SECONDS, MAX_LOCAL_FRAME_BYTES,
};
#[cfg(unix)]
pub use local_server::{
    serve_forever, serve_one, serve_until_shutdown, submit_local, LocalServerShutdown,
    PrivateUnixListener,
};
pub use local_service::LocalPublishService;
pub use model::{JobPhase, JobRecord, LocalAudioArtifact, ProducerRequest};
pub use nmp_publisher::NmpPublisher;
pub use production::{ProductionConfig, ProductionProducer};
pub use runner::{ArtifactUploader, ProducerError, ProducerRunner, Publisher, Synthesizer};

#[cfg(test)]
mod answer_wait_tests;
#[cfg(all(test, unix))]
mod local_server_tests;
#[cfg(test)]
mod production_tests;
#[cfg(test)]
mod test_http;
#[cfg(test)]
mod tests;
