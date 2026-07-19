use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nmp::{Engine, EngineConfig, RelayUrl};
use tts29_producer_api::{SpokenTree, TreeAttachment};
use tts29_protocol::{AttachLink, Question};

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
    pub default_voice: String,
    pub kokoro: KokoroConfig,
    pub blossom: BlossomUploadConfig,
    pub receipt_timeout: Duration,
}

pub struct ProductionProducer {
    runner: ProductionRunner,
    engine: Arc<Engine>,
    identities: IdentityRegistry,
    file_uploader: BlossomArtifactUploader,
    default_voice: String,
    author: String,
    group_id: String,
    clock: Arc<dyn Clock + Send + Sync>,
    answer_waiter: AnswerWaiter,
}

/// Evidence for one published spoken tree: the root event and every narrated
/// child, in publication order.
#[derive(Clone, Debug)]
pub struct TreePublication {
    pub root_event_id: String,
    pub child_event_ids: Vec<String>,
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
            config.blossom.clone(),
            Arc::clone(&clock),
        )?;
        let file_uploader = BlossomArtifactUploader::new(
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
            file_uploader,
            default_voice: config.default_voice,
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

    /// Synthesizes, uploads, and publishes a whole spoken tree: the root first,
    /// then each narrated child linked back to its parent event id, depth-first.
    /// Every message and file attachment is a local path the daemon reads. When
    /// `agent_secret` is set, the whole tree is signed as that agent; otherwise
    /// the daemon identity signs.
    pub fn publish_tree(
        &mut self,
        tree: SpokenTree,
        agent_name: &str,
        agent_secret: Option<&str>,
    ) -> Result<TreePublication, ProducerError> {
        if tree.group_id != self.group_id {
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
        let created_at = self.clock.unix_millis() / 1_000;
        let mut children = Vec::new();
        let result = self
            .publish_node(
                tree.request_id.clone(),
                tree.title,
                tree.summary.unwrap_or_default(),
                &tree.message,
                tree.questions,
                tree.attachments,
                None,
                agent_name,
                &author,
                created_at,
                &mut children,
            )
            .map(|root| TreePublication {
                root_event_id: root,
                child_event_ids: children,
            });
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

    #[allow(clippy::too_many_arguments)]
    fn publish_node(
        &mut self,
        request_id: String,
        title: String,
        summary: String,
        message_path: &str,
        questions: Vec<Question>,
        attachments: Vec<TreeAttachment>,
        parent_event_id: Option<String>,
        agent_name: &str,
        author: &str,
        created_at: u64,
        published_children: &mut Vec<String>,
    ) -> Result<String, ProducerError> {
        let body = fs::read_to_string(message_path).map_err(|error| ProducerError::Capability {
            stage: "read_message",
            reason: error.to_string(),
        })?;
        let mut durable = Vec::new();
        let mut narrated = Vec::new();
        for attachment in attachments {
            match attachment {
                TreeAttachment::File { label, file } => {
                    let artifact = self
                        .file_uploader
                        .make_durable_file(Path::new(&file), label)
                        .map_err(|reason| ProducerError::Capability {
                            stage: "attachment_upload",
                            reason,
                        })?;
                    durable.push(artifact);
                }
                TreeAttachment::Narrated {
                    label,
                    message,
                    questions,
                    attachments,
                } => narrated.push((label, message, questions, attachments)),
            }
        }
        let request = ProducerRequest {
            request_id: request_id.clone(),
            group_id: self.group_id.clone(),
            voice: self.default_voice.clone(),
            agent_name: agent_name.to_string(),
            subject: title,
            summary,
            body,
            attachments: durable,
            questions,
            attach: parent_event_id.map(|parent_id| AttachLink { parent_id }),
        };
        let job = self.publish_as(request, author.to_string(), created_at)?;
        let event_id = job
            .phase
            .event_id()
            .ok_or_else(|| ProducerError::Capability {
                stage: "publication",
                reason: "spoken node did not reach publication".into(),
            })?
            .to_string();
        for (index, (label, message, child_questions, child_attachments)) in
            narrated.into_iter().enumerate()
        {
            let child_id = child_request_id(&request_id, index);
            let child_event = self.publish_node(
                child_id,
                label,
                String::new(),
                &message,
                child_questions,
                child_attachments,
                Some(event_id.clone()),
                agent_name,
                author,
                created_at,
                published_children,
            )?;
            published_children.push(child_event);
        }
        Ok(event_id)
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

/// A per-child journaled request id derived from its parent's, bounded to the
/// request-id length limit and restricted to allowed characters.
fn child_request_id(parent: &str, index: usize) -> String {
    let mut id: String = parent.chars().take(60).collect();
    id.push('-');
    id.push_str(&index.to_string());
    id
}
