use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;

use nmp::{AccountRegistration, Engine};
use rand_core::{OsRng, RngCore};

const MAX_IDENTITY_BYTES: u64 = 512;
const PRIVATE_FILE_MODE: u32 = 0o600;

pub(crate) fn install_daemon_identity(
    engine: &Arc<Engine>,
    identity_path: &Path,
    secret_override: Option<&str>,
) -> Result<AccountRegistration, String> {
    if let Some(secret) = secret_override {
        return register(engine, secret);
    }
    match load_identity(identity_path) {
        Ok(Some(secret)) => register(engine, &secret),
        Ok(None) => generate_and_install(engine, identity_path),
        Err(error) => Err(error),
    }
}

fn load_identity(path: &Path) -> Result<Option<String>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "daemon identity metadata could not be read: {error}"
            ))
        }
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_IDENTITY_BYTES
    {
        return Err("daemon identity file is not a bounded regular file".into());
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err("daemon identity file must not be accessible by group or other users".into());
    }
    let secret = fs::read_to_string(path)
        .map_err(|error| format!("daemon identity could not be read: {error}"))?;
    let secret = secret.trim();
    if secret.is_empty() || secret.lines().count() != 1 {
        return Err("daemon identity file is invalid".into());
    }
    Ok(Some(secret.to_string()))
}

fn generate_and_install(engine: &Arc<Engine>, path: &Path) -> Result<AccountRegistration, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("daemon identity directory could not be created: {error}"))?;
    }
    for _ in 0..16 {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let secret = encode_hex(bytes);
        let registration = match register(engine, &secret) {
            Ok(registration) => registration,
            Err(_) => continue,
        };
        match persist_new(path, &secret) {
            Ok(()) => return Ok(registration),
            Err(PersistError::AlreadyExists) => {
                engine.remove_account(&registration).map_err(|error| {
                    format!("generated daemon identity cleanup failed after a file race: {error}")
                })?;
                let secret = load_identity(path)?.ok_or_else(|| {
                    "daemon identity disappeared during concurrent creation".to_string()
                })?;
                return register(engine, &secret);
            }
            Err(PersistError::Failed(reason)) => {
                let _ = engine.remove_account(&registration);
                return Err(reason);
            }
        }
    }
    Err("could not generate a valid daemon identity".into())
}

fn register(engine: &Engine, secret: &str) -> Result<AccountRegistration, String> {
    engine
        .add_account(secret)
        .map_err(|error| format!("NMP daemon identity was refused: {error}"))
}

enum PersistError {
    AlreadyExists,
    Failed(String),
}

fn persist_new(path: &Path, secret: &str) -> Result<(), PersistError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
        .map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                PersistError::AlreadyExists
            } else {
                PersistError::Failed(format!("daemon identity could not be created: {error}"))
            }
        })?;
    if let Err(error) = file
        .write_all(secret.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
    {
        let _ = fs::remove_file(path);
        return Err(PersistError::Failed(format!(
            "daemon identity could not be persisted: {error}"
        )));
    }
    Ok(())
}

fn encode_hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use nmp::EngineConfig;

    use super::*;

    #[test]
    fn generated_identity_is_private_and_reused() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("daemon.key");
        let first_engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
        let first = install_daemon_identity(&first_engine, &path, None).unwrap();
        let first_author = first.public_key();
        first_engine.shutdown();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let second_engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
        let second = install_daemon_identity(&second_engine, &path, None).unwrap();

        assert_eq!(second.public_key(), first_author);
        second_engine.shutdown();
    }

    #[test]
    fn deployment_override_is_not_written_to_disk() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("daemon.key");
        let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());

        install_daemon_identity(&engine, &path, Some(&"1".repeat(64))).unwrap();

        assert!(!path.exists());
        engine.shutdown();
    }

    #[test]
    fn permissive_identity_file_is_refused() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("daemon.key");
        fs::write(&path, format!("{}\n", "1".repeat(64))).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());

        let error = install_daemon_identity(&engine, &path, None).err().unwrap();

        assert!(error.contains("group or other users"));
        engine.shutdown();
    }
}
