use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::Url;
use sha2::{Digest, Sha256};

use crate::{LocalAudioArtifact, ProducerRequest, Synthesizer};

static STAGED_ID: AtomicU64 = AtomicU64::new(1);

pub enum KokoroAuth {
    None,
    Bearer(String),
    Basic { username: String, password: String },
}

pub struct KokoroConfig {
    pub endpoint: String,
    pub auth: KokoroAuth,
    pub request_timeout: Duration,
    pub max_audio_bytes: usize,
    pub allow_insecure_loopback: bool,
}

impl KokoroConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            auth: KokoroAuth::None,
            request_timeout: Duration::from_secs(120),
            max_audio_bytes: 50 * 1024 * 1024,
            allow_insecure_loopback: false,
        }
    }
}

pub struct KokoroSynthesizer {
    client: Client,
    endpoint: Url,
    auth: KokoroAuth,
    max_audio_bytes: usize,
}

impl KokoroSynthesizer {
    pub fn new(config: KokoroConfig) -> Result<Self, String> {
        let endpoint = validated_endpoint(&config)?;
        if config.max_audio_bytes == 0 {
            return Err("Kokoro audio limit must be positive".into());
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .timeout(config.request_timeout)
            .build()
            .map_err(|error| format!("Kokoro HTTP client could not start: {error}"))?;
        Ok(Self {
            client,
            endpoint,
            auth: config.auth,
            max_audio_bytes: config.max_audio_bytes,
        })
    }

    fn generate(&self, request: &ProducerRequest) -> Result<Vec<u8>, String> {
        if request.body.trim().is_empty() {
            return Err("Kokoro input is empty".into());
        }
        if request.voice.is_empty() || request.voice.len() > 64 {
            return Err("Kokoro voice is invalid".into());
        }
        let body = serde_json::json!({
            "model": "kokoro",
            "input": request.body,
            "voice": request.voice,
            "response_format": "mp3"
        });
        let mut pending = self.client.post(self.endpoint.clone()).json(&body);
        pending = match &self.auth {
            KokoroAuth::None => pending,
            KokoroAuth::Bearer(token) => pending.bearer_auth(token),
            KokoroAuth::Basic { username, password } => {
                pending.basic_auth(username, Some(password))
            }
        };
        let response = pending
            .send()
            .map_err(|error| format!("Kokoro request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Kokoro rejected synthesis with HTTP {}",
                response.status()
            ));
        }
        if let Some(length) = response.content_length() {
            if length == 0 || length > self.max_audio_bytes as u64 {
                return Err("Kokoro response size is invalid".into());
            }
        }
        if let Some(media_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
            let media_type = media_type.to_str().unwrap_or_default();
            if !matches!(
                media_type.split(';').next().unwrap_or_default(),
                "audio/mpeg" | "audio/mp3" | "application/octet-stream"
            ) {
                return Err("Kokoro response is not MP3 audio".into());
            }
        }
        let mut bytes = Vec::new();
        response
            .take(self.max_audio_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Kokoro audio could not be read: {error}"))?;
        if bytes.is_empty() || bytes.len() > self.max_audio_bytes {
            return Err("Kokoro response size is invalid".into());
        }
        Ok(bytes)
    }
}

impl Synthesizer for KokoroSynthesizer {
    fn synthesize(
        &mut self,
        request: &ProducerRequest,
        output: &Path,
    ) -> Result<LocalAudioArtifact, String> {
        if output.exists() {
            return artifact_for_existing(output, self.max_audio_bytes);
        }
        let bytes = self.generate(request)?;
        write_atomic(output, &bytes)?;
        artifact_for_existing(output, self.max_audio_bytes)
    }
}

fn validated_endpoint(config: &KokoroConfig) -> Result<Url, String> {
    let endpoint = Url::parse(&config.endpoint)
        .map_err(|_| "Kokoro endpoint is not a valid URL".to_string())?;
    if endpoint.username() != ""
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err("Kokoro endpoint cannot contain credentials, query, or fragment".into());
    }
    let secure = endpoint.scheme() == "https";
    let admitted_loopback = endpoint.scheme() == "http"
        && config.allow_insecure_loopback
        && matches!(endpoint.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if !secure && !admitted_loopback {
        return Err("Kokoro endpoint must use HTTPS".into());
    }
    Ok(endpoint)
}

fn artifact_for_existing(path: &Path, limit: usize) -> Result<LocalAudioArtifact, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("Kokoro audio could not be read: {error}"))?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err("existing Kokoro audio size is invalid".into());
    }
    Ok(LocalAudioArtifact {
        path: path.to_string_lossy().into_owned(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        media_type: "audio/mpeg".into(),
        byte_count: bytes.len() as u64,
    })
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let staged = staged_path(destination)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&staged)
        .map_err(|error| format!("Kokoro audio staging failed: {error}"))?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        match fs::hard_link(&staged, destination) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        fs::remove_file(&staged)?;
        if let Some(parent) = destination.parent() {
            File::open(parent)?.sync_all()?;
        }
        Ok::<_, std::io::Error>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result.map_err(|error| format!("Kokoro audio commit failed: {error}"))
}

fn staged_path(destination: &Path) -> Result<PathBuf, String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "Kokoro output has no parent directory".to_string())?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Kokoro output filename is invalid".to_string())?;
    let id = STAGED_ID.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_audio_is_never_overwritten_by_a_competing_commit() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("speech.mp3");

        write_atomic(&destination, b"first complete audio").unwrap();
        write_atomic(&destination, b"second divergent audio").unwrap();

        assert_eq!(fs::read(destination).unwrap(), b"first complete audio");
    }
}
