use nmp::{AccountRegistration, Engine};

use crate::model::{CredentialOperation, CredentialRequest, IdentityPhase, IdentitySnapshot};

enum PendingCredential {
    Store {
        request: CredentialRequest,
        registration: AccountRegistration,
        pubkey: String,
    },
    Delete {
        request: CredentialRequest,
    },
}

pub struct IdentityController {
    account: Option<AccountRegistration>,
    snapshot: IdentitySnapshot,
    pending: Option<PendingCredential>,
    next_request_id: u64,
}

impl IdentityController {
    pub fn new() -> Self {
        Self {
            account: None,
            snapshot: IdentitySnapshot::signed_out(),
            pending: None,
            next_request_id: 1,
        }
    }

    pub fn snapshot(&self) -> IdentitySnapshot {
        self.snapshot.clone()
    }

    pub fn credential_request(&self) -> Option<CredentialRequest> {
        match &self.pending {
            Some(PendingCredential::Store { request, .. })
            | Some(PendingCredential::Delete { request }) => Some(request.clone()),
            None => None,
        }
    }

    pub fn active_pubkey(&self) -> Option<&str> {
        (self.snapshot.phase == IdentityPhase::SignedIn)
            .then_some(self.snapshot.pubkey.as_deref())
            .flatten()
    }

    pub fn login(&mut self, engine: &Engine, secret: &str, persist: bool) {
        if self.pending.is_some() {
            self.set_error("Another account operation is still in progress.");
            return;
        }
        if self.account.is_some() {
            self.set_error("Log out before signing in with another key.");
            return;
        }
        let registration = match engine.add_account(secret) {
            Ok(value) => value,
            Err(_) => {
                self.snapshot = IdentitySnapshot {
                    phase: IdentityPhase::SignedOut,
                    pubkey: None,
                    error: Some("That nsec is not valid.".into()),
                };
                return;
            }
        };
        let pubkey = registration.public_key().to_hex();
        if persist {
            let request = self.request(CredentialOperation::Store);
            self.pending = Some(PendingCredential::Store {
                request,
                registration,
                pubkey: pubkey.clone(),
            });
            self.snapshot = IdentitySnapshot {
                phase: IdentityPhase::Saving,
                pubkey: Some(pubkey),
                error: None,
            };
            return;
        }
        self.activate(engine, registration, pubkey);
    }

    pub fn logout(&mut self) {
        if self.pending.is_some() {
            self.set_error("Another account operation is still in progress.");
            return;
        }
        if self.account.is_none() {
            self.snapshot = IdentitySnapshot::signed_out();
            return;
        }
        let request = self.request(CredentialOperation::Delete);
        self.pending = Some(PendingCredential::Delete { request });
        self.snapshot.phase = IdentityPhase::LoggingOut;
        self.snapshot.error = None;
    }

    pub fn restore_failed(&mut self, error: String) {
        if self.account.is_none() && self.pending.is_none() {
            self.snapshot = IdentitySnapshot {
                phase: IdentityPhase::SignedOut,
                pubkey: None,
                error: Some(capability_error(
                    "The saved login could not be read.",
                    Some(error),
                )),
            };
        }
    }

    pub fn credential_result(
        &mut self,
        engine: &Engine,
        request_id: u64,
        succeeded: bool,
        error: Option<String>,
    ) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        if pending.request().id != request_id {
            self.pending = Some(pending);
            return;
        }
        match pending {
            PendingCredential::Store {
                registration,
                pubkey,
                ..
            } if succeeded => self.activate(engine, registration, pubkey),
            PendingCredential::Store { registration, .. } => {
                let _ = engine.remove_account(&registration);
                self.snapshot = IdentitySnapshot {
                    phase: IdentityPhase::SignedOut,
                    pubkey: None,
                    error: Some(capability_error("The login could not be saved.", error)),
                };
            }
            PendingCredential::Delete { .. } if succeeded => {
                self.detach(engine);
                self.snapshot = IdentitySnapshot::signed_out();
            }
            PendingCredential::Delete { .. } => {
                self.snapshot.phase = IdentityPhase::SignedIn;
                self.snapshot.error = Some(capability_error(
                    "The saved login could not be removed.",
                    error,
                ));
            }
        }
    }

    pub fn shutdown(&mut self, engine: &Engine) {
        if let Some(PendingCredential::Store { registration, .. }) = self.pending.take() {
            let _ = engine.remove_account(&registration);
        }
        self.detach(engine);
    }

    fn activate(&mut self, engine: &Engine, registration: AccountRegistration, pubkey: String) {
        if let Err(error) = engine.set_active_account(Some(registration.public_key())) {
            let _ = engine.remove_account(&registration);
            self.snapshot = IdentitySnapshot {
                phase: IdentityPhase::SignedOut,
                pubkey: None,
                error: Some(format!("The account could not be activated: {error}")),
            };
            return;
        }
        self.account = Some(registration);
        self.snapshot = IdentitySnapshot {
            phase: IdentityPhase::SignedIn,
            pubkey: Some(pubkey),
            error: None,
        };
    }

    fn detach(&mut self, engine: &Engine) {
        let _ = engine.set_active_account(None);
        if let Some(registration) = self.account.take() {
            let _ = engine.remove_account(&registration);
        }
    }

    fn request(&mut self, operation: CredentialOperation) -> CredentialRequest {
        let request = CredentialRequest {
            id: self.next_request_id,
            operation,
        };
        self.next_request_id = self.next_request_id.saturating_add(1);
        request
    }

    fn set_error(&mut self, message: &str) {
        self.snapshot.error = Some(message.into());
    }
}

impl PendingCredential {
    fn request(&self) -> &CredentialRequest {
        match self {
            Self::Store { request, .. } | Self::Delete { request } => request,
        }
    }
}

fn capability_error(prefix: &str, error: Option<String>) -> String {
    match error.filter(|value| !value.trim().is_empty()) {
        Some(error) => format!("{prefix} {error}"),
        None => prefix.into(),
    }
}

#[cfg(test)]
mod tests {
    use nmp::EngineConfig;

    use super::*;

    #[test]
    fn login_snapshot_contains_pubkey_but_never_secret() {
        let engine = Engine::new(EngineConfig::default()).unwrap();
        let secret = "1".repeat(64);
        let mut identity = IdentityController::new();

        identity.login(&engine, &secret, true);

        let encoded = serde_json::to_string(&identity.snapshot()).unwrap();
        assert_eq!(identity.snapshot().phase, IdentityPhase::Saving);
        assert!(!encoded.contains(&secret));
        assert!(identity.credential_request().is_some());
        identity.shutdown(&engine);
        engine.shutdown();
    }

    #[test]
    fn logout_waits_for_keychain_deletion_and_preserves_no_signer() {
        let engine = Engine::new(EngineConfig::default()).unwrap();
        let mut identity = IdentityController::new();
        identity.login(&engine, &"2".repeat(64), false);
        assert!(identity.active_pubkey().is_some());

        identity.logout();
        let request = identity.credential_request().unwrap();
        identity.credential_result(&engine, request.id, true, None);

        assert_eq!(identity.snapshot(), IdentitySnapshot::signed_out());
        assert_eq!(engine.active_account().unwrap(), None);
        engine.shutdown();
    }
}
