use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nmp::{Engine, EngineConfig, RelayUrl};

use crate::{
    AnswerWaitCancel, AnswerWaitError, AnswerWaiter, BlossomArtifactUploader, BlossomUploadConfig,
    FileJobJournal, JobRecord, KokoroConfig, KokoroSynthesizer, NmpPublisher, ProducerError,
    ProducerRequest, ProducerRunner, SystemClock,
};

type ProductionRunner =
    ProducerRunner<FileJobJournal, KokoroSynthesizer, BlossomArtifactUploader, NmpPublisher>;

pub struct ProductionConfig {
    pub journal_root: PathBuf,
    pub work_root: PathBuf,
    pub nmp_store_path: Option<String>,
    pub secret_key: String,
    pub host: String,
    pub group_id: String,
    pub kokoro: KokoroConfig,
    pub blossom: BlossomUploadConfig,
    pub receipt_timeout: Duration,
}

pub struct ProductionProducer {
    runner: ProductionRunner,
    engine: Arc<Engine>,
    author: String,
    group_id: String,
    answer_waiter: AnswerWaiter,
}

impl ProductionProducer {
    pub fn open(config: ProductionConfig) -> Result<Self, String> {
        if config.group_id.is_empty() || config.group_id.len() > 128 {
            return Err("configured group id is invalid".into());
        }
        let host = RelayUrl::parse(&config.host)
            .map_err(|_| "configured NIP-29 host is invalid".to_string())?;
        let engine = Arc::new(
            Engine::new(EngineConfig {
                store_path: config.nmp_store_path,
                ..EngineConfig::default()
            })
            .map_err(|error| format!("NMP engine refused startup: {error}"))?,
        );
        let account = engine
            .add_account(&config.secret_key)
            .map_err(|error| format!("NMP producer identity was refused: {error}"))?;
        let author_key = account.public_key();
        engine
            .set_active_account(Some(author_key))
            .map_err(|error| format!("NMP producer identity could not activate: {error}"))?;

        let synthesizer = KokoroSynthesizer::new(config.kokoro)?;
        let uploader = BlossomArtifactUploader::new(
            Arc::clone(&engine),
            author_key,
            config.blossom,
            Arc::new(SystemClock),
        )?;
        let answer_waiter = AnswerWaiter::new(
            Arc::clone(&engine),
            host.clone(),
            config.group_id.clone(),
            Arc::new(SystemClock),
        );
        let publisher = NmpPublisher::new(
            Arc::clone(&engine),
            host,
            config.group_id.clone(),
            config.receipt_timeout,
        );
        let journal =
            FileJobJournal::open(config.journal_root).map_err(|error| error.to_string())?;
        let runner =
            ProducerRunner::new(journal, synthesizer, uploader, publisher, config.work_root)
                .map_err(|error| error.to_string())?;
        Ok(Self {
            runner,
            engine,
            author: author_key.to_hex(),
            group_id: config.group_id,
            answer_waiter,
        })
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn admit(
        &mut self,
        request: ProducerRequest,
        created_at: u64,
    ) -> Result<JobRecord, ProducerError> {
        if request.group_id != self.group_id {
            return Err(ProducerError::InvalidRequest("group_id"));
        }
        self.runner.admit(request, self.author.clone(), created_at)
    }

    pub fn advance(&mut self, request_id: &str) -> Result<JobRecord, ProducerError> {
        self.runner.advance(request_id)
    }

    pub fn wait_for_answer(
        &self,
        job: &JobRecord,
        timeout: Duration,
        cancel: &AnswerWaitCancel,
    ) -> Result<tts29_protocol::AnswerBundle, AnswerWaitError> {
        self.answer_waiter.wait(job, timeout, cancel)
    }

    pub fn shutdown(&self) {
        self.engine.shutdown();
    }
}
