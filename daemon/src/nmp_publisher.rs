use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nmp::{CorrelationToken, Engine, LiveQuery, PublicKey, RelayUrl, RowDelta, SourceStatus};
use sha2::{Digest, Sha256};
use tts29_protocol::{
    compose_membership_upsert, compose_spoken_item, event_authorizes_member,
    group_membership_demand, FrozenSpokenItem,
};

use crate::nmp_receipt::await_host_ack;
use crate::{AuthorizationStep, Publisher};

pub struct NmpPublisher {
    engine: Arc<Engine>,
    host: RelayUrl,
    group_id: String,
    admin_author: PublicKey,
    receipt_timeout: Duration,
    /// Authors this publisher has already established membership for in this
    /// session, mapped to their membership event id. Prevents re-adding the
    /// same agent for every node of a tree before the relay view catches up.
    authorized_members: BTreeMap<String, String>,
}

impl NmpPublisher {
    pub fn new(
        engine: Arc<Engine>,
        host: RelayUrl,
        group_id: String,
        admin_author: PublicKey,
        receipt_timeout: Duration,
    ) -> Self {
        Self {
            engine,
            host,
            group_id,
            admin_author,
            receipt_timeout,
            authorized_members: BTreeMap::new(),
        }
    }

    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }
}

impl Publisher for NmpPublisher {
    fn authorize(
        &mut self,
        request_id: &str,
        author: &str,
        created_at: u64,
    ) -> Result<AuthorizationStep, String> {
        if let Some(event_id) = self.authorized_members.get(author) {
            return Ok(AuthorizationStep::Authorized {
                event_id: event_id.clone(),
            });
        }
        let member = PublicKey::parse(author)
            .map_err(|_| "frozen spoken-item author is not a public key".to_string())?;
        if let Some(event_id) = self.current_membership(&member)? {
            self.authorized_members
                .insert(author.to_string(), event_id.clone());
            return Ok(AuthorizationStep::Authorized { event_id });
        }

        let digest = format!(
            "{:x}",
            Sha256::digest(
                [
                    b"tts29-membership-v1\0".as_slice(),
                    request_id.as_bytes(),
                    b"\0",
                    author.as_bytes()
                ]
                .concat(),
            )
        );
        let correlation = CorrelationToken::try_from(digest.as_str())
            .map_err(|error| format!("membership correlation was refused: {error}"))?;
        let intent = compose_membership_upsert(
            self.host.clone(),
            &self.group_id,
            &self.admin_author.to_hex(),
            author,
            created_at,
            correlation,
        )
        .map_err(|error| error.to_string())?;
        let receipt = self
            .engine
            .publish_tracked(intent)
            .map_err(|error| error.to_string())?;
        Ok(AuthorizationStep::Accepted {
            receipt_id: receipt.id.0,
        })
    }

    fn resume_authorization(&mut self, receipt_id: u64, author: &str) -> Result<String, String> {
        match self.await_host_ack(receipt_id) {
            Ok(event_id) => {
                self.authorized_members
                    .insert(author.to_string(), event_id.clone());
                Ok(event_id)
            }
            // A duplicate add means the author is already authorized; reuse the
            // membership event id cached from the first successful add.
            Err(error)
                if error.contains("members already") || error.contains("already a member") =>
            {
                self.authorized_members.get(author).cloned().ok_or_else(|| {
                    format!("membership add was rejected as duplicate but no prior membership is known: {error}")
                })
            }
            Err(error) => Err(error),
        }
    }

    fn accept(&mut self, item: &FrozenSpokenItem) -> Result<u64, String> {
        if item.group_id != self.group_id {
            return Err("spoken item group does not match the configured producer group".into());
        }
        let intent =
            compose_spoken_item(self.host.clone(), item).map_err(|error| error.to_string())?;
        let receipt = self
            .engine
            .publish_tracked(intent)
            .map_err(|error| error.to_string())?;
        Ok(receipt.id.0)
    }

    fn resume(&mut self, receipt_id: u64, _item: &FrozenSpokenItem) -> Result<String, String> {
        self.await_host_ack(receipt_id)
    }
}

impl NmpPublisher {
    fn current_membership(&self, member: &PublicKey) -> Result<Option<String>, String> {
        let demand = group_membership_demand(self.host.clone(), &self.group_id)
            .map_err(|error| error.to_string())?;
        let subscription = self
            .engine
            .observe(LiveQuery(demand), None)
            .map_err(|error| error.to_string())?;
        let deadline = Instant::now() + self.receipt_timeout;
        let mut last_evidence = "no frame received".to_string();
        let mut current = BTreeMap::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "NMP membership observation did not reconcile in time ({last_evidence})"
                ));
            }
            let frame = subscription.recv_timeout(remaining).map_err(|_| {
                format!("NMP membership observation did not reconcile in time ({last_evidence})")
            })?;
            for delta in &frame.deltas {
                match delta {
                    RowDelta::Added(row) => {
                        if !current.contains_key(&row.event.id) && current.len() >= 4 {
                            return Err(
                                "NMP returned an invalid number of current membership states"
                                    .into(),
                            );
                        }
                        current.insert(row.event.id, row.event.clone());
                    }
                    RowDelta::Removed(id) => {
                        current.remove(id);
                    }
                    RowDelta::SourcesGrew { .. } => {}
                }
            }
            last_evidence = format!("{:?}", frame.evidence);
            let source = frame
                .evidence
                .sources
                .iter()
                .find(|source| source.relay == self.host);
            if source.is_some_and(|source| {
                matches!(
                    source.status,
                    SourceStatus::AuthDenied | SourceStatus::Error
                )
            }) {
                return Err("NMP membership observation was refused by the selected host".into());
            }
            let reconciled = source.is_some_and(|source| {
                source.status == SourceStatus::Requesting && source.reconciled_through.is_some()
            });
            if !reconciled {
                continue;
            }
            return Ok(current.values().find_map(|event| {
                event_authorizes_member(event, &self.group_id, member).then(|| event.id.to_hex())
            }));
        }
    }

    fn await_host_ack(&self, receipt_id: u64) -> Result<String, String> {
        await_host_ack(
            &self.engine,
            receipt_id,
            &self.host,
            self.receipt_timeout,
            "NMP write",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nmp::EngineConfig;
    use tts29_protocol::DurableArtifact;

    #[test]
    fn missing_receipt_is_not_republished_or_misreported() {
        let engine = Engine::new(EngineConfig::default()).unwrap();
        let mut publisher = NmpPublisher::new(
            Arc::new(engine),
            RelayUrl::parse("wss://relay.example.com").unwrap(),
            "tts".into(),
            PublicKey::parse(&"1".repeat(64)).unwrap(),
            Duration::from_millis(10),
        );

        let error = publisher.resume(404, &fixture()).unwrap_err();

        assert!(error.contains("not found"));
    }

    fn fixture() -> FrozenSpokenItem {
        FrozenSpokenItem {
            author: "1".repeat(64),
            created_at: 1,
            group_id: "tts".into(),
            agent_name: "Codex".into(),
            subject: "Recovery".into(),
            summary: "Receipt recovery".into(),
            body: "Receipt recovery".into(),
            audio: DurableArtifact {
                url: "https://cdn.example/audio.mp3".into(),
                sha256: "a".repeat(64),
                media_type: "audio/mpeg".into(),
                byte_count: 10,
                label: None,
            },
            attachments: Vec::new(),
            questions: Vec::new(),
            attach: None,
        }
    }
}
