use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nmp::{Engine, EngineConfig, RelayUrl};

use crate::identity::IdentityRegistry;
use crate::{
    AnswerWaitCancel, AnswerWaitError, AnswerWaiter, BlossomArtifactUploader, BlossomUploadConfig,
    Clock, FileJobJournal, JobRecord, KokoroConfig, KokoroSynthesizer, NmpPublisher, ProducerError,
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
    identities: IdentityRegistry,
    author: String,
    group_id: String,
    clock: Arc<dyn Clock + Send + Sync>,
    answer_waiter: AnswerWaiter,
}

impl ProductionProducer {
    pub fn open(config: ProductionConfig) -> Result<Self, String> {
        Self::open_with_clock(config, Arc::new(SystemClock))
    }

    pub fn open_with_clock(
        config: ProductionConfig,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, String> {
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
            Arc::clone(&clock),
        )?;
        let answer_waiter = AnswerWaiter::new(
            Arc::clone(&engine),
            host.clone(),
            config.group_id.clone(),
            Arc::clone(&clock),
        );
        let publisher = NmpPublisher::new(
            Arc::clone(&engine),
            host,
            config.group_id.clone(),
            author_key,
            config.receipt_timeout,
        );
        let journal =
            FileJobJournal::open(config.journal_root).map_err(|error| error.to_string())?;
        let runner =
            ProducerRunner::new(journal, synthesizer, uploader, publisher, config.work_root)
                .map_err(|error| error.to_string())?;
        Ok(Self {
            runner,
            identities: IdentityRegistry::new(Arc::clone(&engine), account),
            engine,
            author: author_key.to_hex(),
            group_id: config.group_id,
            clock,
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

    pub fn publish(
        &mut self,
        request: ProducerRequest,
        created_at: u64,
        agent_secret: Option<&str>,
    ) -> Result<JobRecord, ProducerError> {
        if request.group_id != self.group_id {
            return Err(ProducerError::InvalidRequest("group_id"));
        }
        let identity =
            self.identities
                .request(agent_secret)
                .map_err(|reason| ProducerError::Capability {
                    stage: "request_identity",
                    reason,
                })?;
        let author = identity.author_hex();
        let result = self.publish_as(request, author, created_at);
        let cleanup = identity
            .close()
            .map_err(|reason| ProducerError::Capability {
                stage: "request_identity_cleanup",
                reason,
            });
        match (result, cleanup) {
            (_, Err(error)) => Err(error),
            (result, Ok(())) => result,
        }
    }

    pub fn publish_now(
        &mut self,
        request: ProducerRequest,
        agent_secret: Option<&str>,
    ) -> Result<JobRecord, ProducerError> {
        self.publish(request, self.clock.unix_millis() / 1_000, agent_secret)
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

    fn publish_as(
        &mut self,
        request: ProducerRequest,
        author: String,
        created_at: u64,
    ) -> Result<JobRecord, ProducerError> {
        let request_id = request.request_id.clone();
        let mut job = self.runner.admit(request, author, created_at)?;
        for _ in 0..6 {
            if job.phase.is_published() {
                return Ok(job);
            }
            job = self.runner.advance(&request_id)?;
        }
        job.phase
            .is_published()
            .then_some(job)
            .ok_or_else(|| ProducerError::Capability {
                stage: "publication",
                reason: "producer exceeded its bounded stage count".into(),
            })
    }
}

impl Drop for ProductionProducer {
    fn drop(&mut self) {
        self.engine.shutdown();
    }
}
