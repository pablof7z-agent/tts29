use std::time::Duration;

use nmp::{Engine, ReceiptId, ReceiptReattachment, RelayUrl, WriteStatus};
use tts29_protocol::{compose_spoken_item, FrozenSpokenItem};

use crate::Publisher;

pub struct NmpPublisher {
    engine: Engine,
    host: RelayUrl,
    receipt_timeout: Duration,
}

impl NmpPublisher {
    pub fn new(engine: Engine, host: RelayUrl, receipt_timeout: Duration) -> Self {
        Self {
            engine,
            host,
            receipt_timeout,
        }
    }

    pub fn into_engine(self) -> Engine {
        self.engine
    }
}

impl Publisher for NmpPublisher {
    fn accept(&mut self, item: &FrozenSpokenItem) -> Result<u64, String> {
        let intent =
            compose_spoken_item(self.host.clone(), item).map_err(|error| error.to_string())?;
        let receipt = self
            .engine
            .publish_tracked(intent)
            .map_err(|error| error.to_string())?;
        Ok(receipt.id.0)
    }

    fn resume(&mut self, receipt_id: u64, _item: &FrozenSpokenItem) -> Result<String, String> {
        let statuses = match self
            .engine
            .reattach_receipt(ReceiptId(receipt_id))
            .map_err(|error| error.to_string())?
        {
            ReceiptReattachment::Attached(_, statuses) => statuses,
            ReceiptReattachment::NotFound => {
                return Err(format!("NMP receipt {receipt_id} was not found"));
            }
            ReceiptReattachment::RetainedButUnreadable => {
                return Err(format!(
                    "NMP receipt {receipt_id} is retained but unreadable"
                ));
            }
        };

        let mut event_id = None;
        let mut host_acked = false;
        for _ in 0..32 {
            let status = statuses
                .recv_timeout(self.receipt_timeout)
                .map_err(|_| format!("NMP receipt {receipt_id} is still pending"))?;
            match status {
                WriteStatus::Signed(id) => event_id = Some(id.to_hex()),
                WriteStatus::Acked(relay) if relay == self.host => host_acked = true,
                WriteStatus::Rejected(relay, reason) => {
                    return Err(format!("NMP receipt rejected by {relay}: {reason}"));
                }
                WriteStatus::GaveUp(relay) => {
                    return Err(format!("NMP receipt gave up delivery to {relay}"));
                }
                WriteStatus::OutcomeUnknown(relay) => {
                    return Err(format!("NMP receipt outcome is unknown for {relay}"));
                }
                WriteStatus::Failed(reason) => return Err(format!("NMP write failed: {reason}")),
                WriteStatus::Cancelled => return Err("NMP write was cancelled".into()),
                WriteStatus::ReplaceableConflict { .. } => {
                    return Err("NMP write encountered a replaceable conflict".into());
                }
                _ => {}
            }
            if host_acked {
                if let Some(event_id) = event_id {
                    return Ok(event_id);
                }
            }
        }
        Err(format!(
            "NMP receipt {receipt_id} exceeded its bounded status stream"
        ))
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
            engine,
            RelayUrl::parse("wss://relay.example.com").unwrap(),
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
        }
    }
}
