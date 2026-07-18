use std::sync::Arc;

use nmp::{AccountRegistration, Engine, PublicKey};

pub(crate) struct IdentityRegistry {
    engine: Arc<Engine>,
    daemon: AccountRegistration,
}

impl IdentityRegistry {
    pub(crate) fn new(engine: Arc<Engine>, daemon: AccountRegistration) -> Self {
        Self { engine, daemon }
    }

    pub(crate) fn daemon_author(&self) -> PublicKey {
        self.daemon.public_key()
    }

    pub(crate) fn request(
        &mut self,
        agent_secret: Option<&str>,
    ) -> Result<RequestIdentity, String> {
        let Some(secret) = agent_secret else {
            return Ok(RequestIdentity::daemon(
                Arc::clone(&self.engine),
                self.daemon_author(),
            ));
        };
        let registration = self
            .engine
            .add_account(secret)
            .map_err(|error| format!("NMP request identity was refused: {error}"))?;
        let author = registration.public_key();
        if author == self.daemon_author() {
            self.daemon = registration;
            Ok(RequestIdentity::daemon(Arc::clone(&self.engine), author))
        } else {
            Ok(RequestIdentity {
                engine: Arc::clone(&self.engine),
                author,
                request_registration: Some(registration),
            })
        }
    }
}

pub(crate) struct RequestIdentity {
    engine: Arc<Engine>,
    author: PublicKey,
    request_registration: Option<AccountRegistration>,
}

impl RequestIdentity {
    fn daemon(engine: Arc<Engine>, author: PublicKey) -> Self {
        Self {
            engine,
            author,
            request_registration: None,
        }
    }

    pub(crate) fn author_hex(&self) -> String {
        self.author.to_hex()
    }

    pub(crate) fn close(mut self) -> Result<(), String> {
        self.remove_request_registration()
    }

    fn remove_request_registration(&mut self) -> Result<(), String> {
        let Some(registration) = self.request_registration.take() else {
            return Ok(());
        };
        match self.engine.remove_account(&registration) {
            Ok(true) => Ok(()),
            Ok(false) => Err("NMP request identity changed before cleanup".into()),
            Err(error) => Err(format!("NMP request identity cleanup failed: {error}")),
        }
    }
}

impl Drop for RequestIdentity {
    fn drop(&mut self) {
        let _ = self.remove_request_registration();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use nmp::EngineConfig;
    use sha2::{Digest, Sha256};

    use super::*;

    static TEST_KEY_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn request_signer_is_removed_by_its_exact_nmp_registration() {
        let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
        let daemon_secret = test_secret(&engine);
        let agent_secret = test_secret(&engine);
        let daemon = engine.add_account(&daemon_secret).unwrap();
        let mut identities = IdentityRegistry::new(Arc::clone(&engine), daemon);
        let request = identities.request(Some(&agent_secret)).unwrap();
        let installed = request.request_registration.as_ref().unwrap().clone();

        request.close().unwrap();

        assert!(!engine.remove_account(&installed).unwrap());
        engine.shutdown();
    }

    #[test]
    fn daemon_key_replacement_stays_installed() {
        let engine = Arc::new(Engine::new(EngineConfig::default()).unwrap());
        let daemon_secret = test_secret(&engine);
        let original = engine.add_account(&daemon_secret).unwrap();
        let mut identities = IdentityRegistry::new(Arc::clone(&engine), original.clone());

        identities
            .request(Some(&daemon_secret))
            .unwrap()
            .close()
            .unwrap();

        assert!(!engine.remove_account(&original).unwrap());
        assert!(engine.remove_account(&identities.daemon).unwrap());
        engine.shutdown();
    }

    fn test_secret(engine: &Engine) -> String {
        loop {
            let id = TEST_KEY_ID.fetch_add(1, Ordering::Relaxed);
            let secret = format!(
                "{:x}",
                Sha256::digest(format!("tts29-identity-test-{}-{id}", std::process::id()))
            );
            if let Ok(registration) = engine.add_account(&secret) {
                engine.remove_account(&registration).unwrap();
                return secret;
            }
        }
    }
}
