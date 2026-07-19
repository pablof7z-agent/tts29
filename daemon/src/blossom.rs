use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use nmp::{Engine, PublicKey, SignEventRequest, Timestamp};
use nmp_blossom::{
    upload_authorization_draft, BlossomClient, BlossomClientConfig, BlossomServerUrl, BlossomVerb,
    ExpectedAuthorization, Sha256Hash, SignedAuthorization,
};
use reqwest::Url;
use tokio::runtime::Runtime;
use tts29_protocol::DurableArtifact;

use crate::{ArtifactUploader, LocalAudioArtifact};

pub trait Clock {
    fn unix_millis(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn unix_millis(&self) -> u64 {
        UNIX_EPOCH
            .elapsed()
            .map_or(0, |duration| duration.as_millis() as u64)
    }
}

#[derive(Clone)]
pub struct BlossomUploadConfig {
    pub server: String,
    pub allowed_local_hosts: BTreeSet<String>,
    pub request_timeout: Duration,
    pub authorization_lifetime: Duration,
    pub max_upload_bytes: u64,
}

impl BlossomUploadConfig {
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            allowed_local_hosts: BTreeSet::new(),
            request_timeout: Duration::from_secs(60),
            authorization_lifetime: Duration::from_secs(300),
            max_upload_bytes: 50 * 1024 * 1024,
        }
    }
}

pub struct BlossomArtifactUploader {
    engine: Arc<Engine>,
    author: PublicKey,
    client: BlossomClient,
    server: BlossomServerUrl,
    runtime: Runtime,
    clock: Arc<dyn Clock + Send + Sync>,
    authorization_lifetime: Duration,
    max_upload_bytes: u64,
}

impl BlossomArtifactUploader {
    pub fn new(
        engine: Arc<Engine>,
        author: PublicKey,
        config: BlossomUploadConfig,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, String> {
        if config.authorization_lifetime.is_zero() || config.max_upload_bytes == 0 {
            return Err("Blossom authorization lifetime and upload limit must be positive".into());
        }
        let server = BlossomServerUrl::parse(&config.server).map_err(|error| error.to_string())?;
        let client = BlossomClient::new(BlossomClientConfig {
            allowed_local_hosts: config.allowed_local_hosts,
            request_deadline: config.request_timeout,
            ..BlossomClientConfig::default()
        })
        .map_err(|error| error.to_string())?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .build()
            .map_err(|error| format!("Blossom runtime could not start: {error}"))?;
        Ok(Self {
            engine,
            author,
            client,
            server,
            runtime,
            clock,
            authorization_lifetime: config.authorization_lifetime,
            max_upload_bytes: config.max_upload_bytes,
        })
    }

    /// Uploads an arbitrary attachment file (image, markdown, …) as a durable
    /// artifact. Unlike synthesized audio this accepts any media type; the URL
    /// must still resolve to public HTTPS.
    pub fn make_durable_file(
        &self,
        path: &std::path::Path,
        label: String,
    ) -> Result<DurableArtifact, String> {
        let bytes = fs::read(path)
            .map_err(|error| format!("attachment could not be read: {error}"))?;
        let size = bytes.len() as u64;
        if size == 0 || size > self.max_upload_bytes {
            return Err("attachment is empty or exceeds the Blossom upload limit".into());
        }
        let media_type = infer_media_type(path);
        let hash = Sha256Hash::of(&bytes);
        let now = Timestamp::from(self.clock.unix_millis() / 1_000);
        let authorization = self.authorization(hash, now)?;
        let upload = self
            .runtime
            .block_on(
                self.client
                    .upload(&self.server, &bytes, Some(&media_type), &authorization),
            )
            .map_err(|error| error.to_string())?;
        let descriptor = upload.into_descriptor();
        validate_public_url(&descriptor.url)?;
        if descriptor.size != size {
            return Err("Blossom descriptor size does not match the attachment".into());
        }
        Ok(DurableArtifact {
            url: descriptor.url,
            sha256: descriptor.sha256.to_hex(),
            media_type,
            byte_count: descriptor.size,
            label: Some(label),
        })
    }

    fn authorization(
        &self,
        hash: Sha256Hash,
        now: Timestamp,
    ) -> Result<SignedAuthorization, String> {
        let expiration = now
            .as_secs()
            .checked_add(self.authorization_lifetime.as_secs())
            .map(Timestamp::from)
            .ok_or_else(|| "Blossom authorization time overflow".to_string())?;
        let draft = upload_authorization_draft(
            self.author,
            hash,
            now,
            expiration,
            "Upload TTS29 spoken audio",
        )
        .map_err(|error| error.to_string())?;
        let operation = self
            .engine
            .sign_event(SignEventRequest {
                created_at: draft.created_at,
                kind: draft.kind,
                tags: draft.tags.into_iter().collect(),
                content: draft.content,
            })
            .map_err(|error| error.to_string())?;
        let signed = operation.recv().map_err(|error| error.to_string())?;
        SignedAuthorization::validate(
            signed,
            &ExpectedAuthorization {
                verb: BlossomVerb::Upload,
                blob: Some(hash),
            },
            now,
        )
        .map_err(|error| error.to_string())
    }
}

impl ArtifactUploader for BlossomArtifactUploader {
    fn make_durable(&mut self, audio: &LocalAudioArtifact) -> Result<DurableArtifact, String> {
        let size = fs::metadata(&audio.path)
            .map_err(|error| format!("synthesized audio could not be inspected: {error}"))?
            .len();
        if size == 0 || size > self.max_upload_bytes {
            return Err("synthesized audio exceeds the Blossom upload limit".into());
        }
        let bytes = fs::read(&audio.path)
            .map_err(|error| format!("synthesized audio could not be read: {error}"))?;
        let hash = Sha256Hash::of(&bytes);
        if hash.to_hex() != audio.sha256 || bytes.len() as u64 != audio.byte_count {
            return Err("synthesized audio changed before Blossom upload".into());
        }
        let now = Timestamp::from(self.clock.unix_millis() / 1_000);
        let authorization = self.authorization(hash, now)?;
        let upload = self
            .runtime
            .block_on(self.client.upload(
                &self.server,
                &bytes,
                Some(&audio.media_type),
                &authorization,
            ))
            .map_err(|error| error.to_string())?;
        let descriptor = upload.into_descriptor();
        validate_descriptor(&descriptor.url, descriptor.mime_type.as_deref())?;
        if descriptor.size != audio.byte_count {
            return Err("Blossom descriptor size does not match synthesized audio".into());
        }
        Ok(DurableArtifact {
            url: descriptor.url,
            sha256: descriptor.sha256.to_hex(),
            media_type: audio.media_type.clone(),
            byte_count: descriptor.size,
            label: None,
        })
    }
}

fn infer_media_type(path: &std::path::Path) -> String {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "svg" => "image/svg+xml",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn validate_public_url(url: &str) -> Result<(), String> {
    let url = Url::parse(url).map_err(|_| "Blossom descriptor URL is invalid".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Blossom descriptor URL must be public HTTPS".into());
    }
    Ok(())
}

fn validate_descriptor(url: &str, media_type: Option<&str>) -> Result<(), String> {
    validate_public_url(url)?;
    if let Some(media_type) = media_type {
        if !matches!(
            media_type,
            "audio/mpeg" | "audio/mp3" | "application/octet-stream"
        ) {
            return Err("Blossom descriptor is not MP3 audio".into());
        }
    }
    Ok(())
}
