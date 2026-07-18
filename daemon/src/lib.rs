mod answer_wait;
mod blossom;
mod daemon_config;
mod identity;
mod journal;
mod kokoro;
#[cfg(unix)]
mod local_server;
mod local_service;
mod model;
mod nmp_publisher;
mod production;
mod request;
mod runner;

pub use answer_wait::{AnswerWaitCancel, AnswerWaitError, AnswerWaiter};
pub use blossom::{BlossomArtifactUploader, BlossomUploadConfig, Clock, SystemClock};
pub use daemon_config::{load_daemon_config, LoadedDaemonConfig};
pub use journal::{FileJobJournal, InsertOutcome, JobJournal, JournalError};
pub use kokoro::{KokoroAuth, KokoroConfig, KokoroSynthesizer};
#[cfg(unix)]
pub use local_server::{
    serve_forever, serve_one, serve_until_shutdown, LocalServerShutdown, PrivateUnixListener,
};
pub use local_service::LocalPublishService;
pub use model::{JobPhase, JobRecord, LocalAudioArtifact, MembershipEvidence};
pub use nmp_publisher::NmpPublisher;
pub use production::{ProductionConfig, ProductionProducer};
pub use runner::{
    ArtifactUploader, AuthorizationStep, ProducerError, ProducerRunner, Publisher, Synthesizer,
};
#[cfg(unix)]
pub use tts29_producer_api::{submit_local, submit_local_with_timeout};
pub use tts29_producer_api::{
    AnswerWaitResult, LocalPublishRequest, LocalPublishResponse, LocalRequestError,
    ProducerRequest, LOCAL_PROTOCOL_VERSION, MAX_ANSWER_WAIT_SECONDS, MAX_LOCAL_FRAME_BYTES,
};

#[cfg(test)]
mod answer_wait_tests;
#[cfg(all(test, unix))]
mod local_server_tests;
#[cfg(test)]
mod production_tests;
#[cfg(test)]
mod test_http;
#[cfg(test)]
mod test_nmp_relay;
#[cfg(test)]
mod test_publisher;
#[cfg(test)]
mod tests;
