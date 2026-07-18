use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use tts29_protocol::FrozenSpokenItem;

use crate::{AuthorizationStep, Publisher};

#[derive(Default)]
pub(crate) struct FakePublisher {
    pub(crate) authorize_calls: usize,
    pub(crate) authorization_resume_calls: usize,
    pub(crate) already_authorized: bool,
    pub(crate) reject_authorization: bool,
    pub(crate) membership_event_ids: BTreeSet<String>,
    pub(crate) accept_calls: usize,
    pub(crate) resume_calls: usize,
    pub(crate) event_ids: BTreeSet<String>,
    receipts: BTreeMap<u64, String>,
}

impl Publisher for FakePublisher {
    fn authorize(
        &mut self,
        request_id: &str,
        author: &str,
        _created_at: u64,
    ) -> Result<AuthorizationStep, String> {
        self.authorize_calls += 1;
        if self.reject_authorization {
            return Err("selected host rejected membership".into());
        }
        let event_id = format!("{:x}", Sha256::digest(format!("{request_id}:{author}")));
        self.membership_event_ids.insert(event_id.clone());
        if self.already_authorized {
            Ok(AuthorizationStep::Authorized { event_id })
        } else {
            Ok(AuthorizationStep::Accepted { receipt_id: 7 })
        }
    }

    fn resume_authorization(&mut self, receipt_id: u64, _author: &str) -> Result<String, String> {
        self.authorization_resume_calls += 1;
        if receipt_id != 7 {
            return Err("unknown membership receipt".into());
        }
        self.membership_event_ids
            .iter()
            .next()
            .cloned()
            .ok_or_else(|| "missing membership event".into())
    }

    fn accept(&mut self, item: &FrozenSpokenItem) -> Result<u64, String> {
        self.accept_calls += 1;
        let event_id = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(item).map_err(|error| error.to_string())?)
        );
        self.event_ids.insert(event_id.clone());
        let receipt_id = self.accept_calls as u64;
        self.receipts.insert(receipt_id, event_id);
        Ok(receipt_id)
    }

    fn resume(&mut self, receipt_id: u64, _item: &FrozenSpokenItem) -> Result<String, String> {
        self.resume_calls += 1;
        self.receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| "unknown receipt".into())
    }
}
