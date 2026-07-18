use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::{BlossomUploadConfig, KokoroAuth, KokoroConfig, ProductionConfig};

const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_CAPABILITY_SECONDS: u64 = 300;
const MAX_AUDIO_BYTES: u64 = 100 * 1024 * 1024;

pub struct LoadedDaemonConfig {
    pub socket_path: PathBuf,
    pub production: ProductionConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    socket_path: PathBuf,
    journal_root: PathBuf,
    work_root: PathBuf,
    nmp_store_path: Option<PathBuf>,
    host: String,
    group_id: String,
    kokoro: KokoroFileConfig,
    blossom: BlossomFileConfig,
    #[serde(default = "default_receipt_timeout")]
    receipt_timeout_seconds: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KokoroFileConfig {
    endpoint: String,
    #[serde(default = "default_kokoro_timeout")]
    request_timeout_seconds: u64,
    #[serde(default = "default_audio_bytes")]
    max_audio_bytes: u64,
    #[serde(default)]
    allow_insecure_loopback: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlossomFileConfig {
    server: String,
    #[serde(default)]
    allowed_local_hosts: BTreeSet<String>,
    #[serde(default = "default_blossom_timeout")]
    request_timeout_seconds: u64,
    #[serde(default = "default_authorization_lifetime")]
    authorization_lifetime_seconds: u64,
    #[serde(default = "default_audio_bytes")]
    max_upload_bytes: u64,
}

pub fn load_daemon_config(path: impl AsRef<Path>) -> Result<LoadedDaemonConfig, String> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| format!("daemon config could not be read: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_CONFIG_BYTES {
        return Err("daemon config size is invalid".into());
    }
    let file: FileConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("daemon config is invalid: {error}"))?;
    validate_duration("receipt timeout", file.receipt_timeout_seconds)?;
    validate_duration("Kokoro timeout", file.kokoro.request_timeout_seconds)?;
    validate_duration("Blossom timeout", file.blossom.request_timeout_seconds)?;
    validate_duration(
        "Blossom authorization lifetime",
        file.blossom.authorization_lifetime_seconds,
    )?;
    validate_audio_limit("Kokoro audio", file.kokoro.max_audio_bytes)?;
    validate_audio_limit("Blossom upload", file.blossom.max_upload_bytes)?;

    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let socket_path = resolve(base, file.socket_path);
    let secret_key = required_env("TTS29_DAEMON_NSEC")?;
    let kokoro_auth = kokoro_auth()?;
    let mut kokoro = KokoroConfig::new(file.kokoro.endpoint);
    kokoro.auth = kokoro_auth;
    kokoro.request_timeout = Duration::from_secs(file.kokoro.request_timeout_seconds);
    kokoro.max_audio_bytes = usize::try_from(file.kokoro.max_audio_bytes)
        .map_err(|_| "Kokoro audio limit does not fit this platform".to_string())?;
    kokoro.allow_insecure_loopback = file.kokoro.allow_insecure_loopback;
    let mut blossom = BlossomUploadConfig::new(file.blossom.server);
    blossom.allowed_local_hosts = file.blossom.allowed_local_hosts;
    blossom.request_timeout = Duration::from_secs(file.blossom.request_timeout_seconds);
    blossom.authorization_lifetime =
        Duration::from_secs(file.blossom.authorization_lifetime_seconds);
    blossom.max_upload_bytes = file.blossom.max_upload_bytes;
    let nmp_store_path = file
        .nmp_store_path
        .map(|value| resolve(base, value))
        .map(|value| {
            value
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| "NMP store path is not valid UTF-8".to_string())
        })
        .transpose()?;
    Ok(LoadedDaemonConfig {
        socket_path,
        production: ProductionConfig {
            journal_root: resolve(base, file.journal_root),
            work_root: resolve(base, file.work_root),
            nmp_store_path,
            secret_key,
            host: file.host,
            group_id: file.group_id,
            kokoro,
            blossom,
            receipt_timeout: Duration::from_secs(file.receipt_timeout_seconds),
        },
    })
}

fn kokoro_auth() -> Result<KokoroAuth, String> {
    let bearer = optional_env("TTS29_KOKORO_BEARER")?;
    let username = optional_env("TTS29_KOKORO_BASIC_USERNAME")?;
    let password = optional_env("TTS29_KOKORO_BASIC_PASSWORD")?;
    match (bearer, username, password) {
        (None, None, None) => Ok(KokoroAuth::None),
        (Some(token), None, None) => Ok(KokoroAuth::Bearer(token)),
        (None, Some(username), Some(password)) => Ok(KokoroAuth::Basic { username, password }),
        _ => Err("Kokoro authentication environment is incomplete or ambiguous".into()),
    }
}

fn required_env(name: &str) -> Result<String, String> {
    optional_env(name)?.ok_or_else(|| format!("required environment variable {name} is missing"))
}

fn optional_env(name: &str) -> Result<Option<String>, String> {
    match env::var(name) {
        Ok(value) if value.is_empty() => Err(format!("environment variable {name} is empty")),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            Err(format!("environment variable {name} is not valid UTF-8"))
        }
    }
}

fn validate_duration(name: &str, value: u64) -> Result<(), String> {
    if value == 0 || value > MAX_CAPABILITY_SECONDS {
        Err(format!(
            "{name} must be between 1 and {MAX_CAPABILITY_SECONDS} seconds"
        ))
    } else {
        Ok(())
    }
}

fn validate_audio_limit(name: &str, value: u64) -> Result<(), String> {
    if value == 0 || value > MAX_AUDIO_BYTES {
        Err(format!("{name} byte limit is invalid"))
    } else {
        Ok(())
    }
}

fn resolve(base: &Path, value: PathBuf) -> PathBuf {
    if value.is_absolute() {
        value
    } else {
        base.join(value)
    }
}

fn default_receipt_timeout() -> u64 {
    30
}

fn default_kokoro_timeout() -> u64 {
    120
}

fn default_blossom_timeout() -> u64 {
    60
}

fn default_authorization_lifetime() -> u64 {
    300
}

fn default_audio_bytes() -> u64 {
    50 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_file_refuses_embedded_daemon_secrets() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("daemon.json");
        fs::write(
            &path,
            r#"{
                "socket_path":"runtime/daemon.sock",
                "journal_root":"jobs",
                "work_root":"work",
                "nmp_store_path":null,
                "host":"wss://relay.example.com",
                "group_id":"tts",
                "secret_key":"must-not-live-here",
                "kokoro":{"endpoint":"https://kokoro.example/v1/audio/speech"},
                "blossom":{"server":"https://blossom.example"}
            }"#,
        )
        .unwrap();

        let error = load_daemon_config(path).err().unwrap();

        assert!(error.contains("unknown field `secret_key`"));
    }
}
