use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use nmp::{
    AccessContext, Binding, Demand, Engine, EngineConfig, Filter, LiveQuery, ReceiptId,
    ReceiptReattachment, RelayUrl, SourceAuthority, Window, WriteStatus,
};
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tts29_protocol::{
    compose_answer, parse, FrozenAnswer, ParsedEvent, QuestionAnswer, SpokenItem,
};

use crate::{Clock, SystemClock};

const LIVE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Serialize)]
pub struct SourceEvidence {
    pub status: String,
    pub reconciled: bool,
    pub query_source_count: usize,
    pub shortfall_count: usize,
    pub row_source_count: usize,
}

#[derive(Debug, Serialize)]
pub struct AudioEvidence {
    pub url: String,
    pub sha256: String,
    pub byte_count: u64,
    pub media_type: String,
    pub downloaded_and_matched: bool,
}

pub(crate) struct ObservedEvent {
    pub parsed: ParsedEvent,
    pub source: SourceEvidence,
}

pub(crate) fn observe_exact(
    host: &RelayUrl,
    group_id: &str,
    event_id: &str,
) -> Result<ObservedEvent, String> {
    let engine = Engine::new(EngineConfig::default())
        .map_err(|error| format!("independent NMP consumer refused startup: {error}"))?;
    let demand = Demand::new(
        Filter {
            ids: Some(Binding::Literal(BTreeSet::from([event_id.to_string()]))),
            ..Filter::default()
        },
        SourceAuthority::Pinned(BTreeSet::from([host.clone()])),
        AccessContext::Public,
    )
    .map_err(|error| format!("exact-event demand was refused: {error}"))?;
    let window = Window::Expandable {
        initial: NonZeroUsize::new(1).unwrap(),
        max: NonZeroUsize::new(1).unwrap(),
    };
    let subscription = engine
        .observe(LiveQuery(demand), Some(window))
        .map_err(|error| format!("independent group observation failed: {error}"))?;
    let mut last_evidence = "no relay frame".to_string();
    let mut result = Err(format!(
        "event {event_id} was not reacquired ({last_evidence})"
    ));
    for _ in 0..12 {
        let Ok(frame) = subscription.recv_timeout(Duration::from_secs(5)) else {
            continue;
        };
        last_evidence = format!("{:?}", frame.evidence);
        let source = frame
            .evidence
            .sources
            .iter()
            .find(|source| source.relay == *host);
        let Some(row) = frame.window.as_ref().and_then(|window| {
            window
                .rows
                .iter()
                .find(|row| row.event.id.to_hex() == event_id)
        }) else {
            continue;
        };
        let Some(source) = source else { continue };
        if !row.sources.contains(host) {
            continue;
        }
        let parsed = parse(row, group_id)
            .ok_or_else(|| format!("event {event_id} failed the TTS29 protocol parser"))?;
        result = Ok(ObservedEvent {
            parsed,
            source: SourceEvidence {
                status: format!("{:?}", source.status),
                reconciled: source.reconciled_through.is_some(),
                query_source_count: frame.evidence.sources.len(),
                shortfall_count: frame.evidence.shortfall.len(),
                row_source_count: row.sources.len(),
            },
        });
        break;
    }
    if result.is_err() {
        result = Err(format!(
            "event {event_id} was not reacquired ({last_evidence})"
        ));
    }
    subscription.cancel();
    engine.shutdown();
    result
}

pub(crate) fn verify_audio(item: &SpokenItem) -> Result<AudioEvidence, String> {
    let response = Client::builder()
        .no_proxy()
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 {
                attempt.error("durable audio exceeded three redirects")
            } else if attempt.url().scheme() != "https" {
                attempt.error("durable audio redirect attempted an HTTPS downgrade")
            } else {
                attempt.follow()
            }
        }))
        .timeout(LIVE_TIMEOUT)
        .build()
        .map_err(|error| format!("audio verifier could not start: {error}"))?
        .get(&item.audio.url)
        .send()
        .map_err(|error| format!("durable audio could not be downloaded: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("durable audio returned HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("durable audio body failed: {error}"))?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if digest != item.audio.sha256 || bytes.len() as u64 != item.audio.byte_count {
        return Err("downloaded audio did not match its durable descriptor".into());
    }
    Ok(AudioEvidence {
        url: item.audio.url.clone(),
        sha256: digest,
        byte_count: bytes.len() as u64,
        media_type: item.audio.media_type.clone(),
        downloaded_and_matched: true,
    })
}

pub(crate) fn publish_answer(
    host: &RelayUrl,
    group_id: &str,
    root_event_id: &str,
    secret: &str,
) -> Result<(u64, String, String), String> {
    let engine = Arc::new(Engine::new(EngineConfig::default()).map_err(|error| error.to_string())?);
    let account = engine
        .add_account(secret)
        .map_err(|error| error.to_string())?;
    let author = account.public_key();
    engine
        .set_active_account(Some(author))
        .map_err(|error| error.to_string())?;
    let intent = compose_answer(
        host.clone(),
        &FrozenAnswer {
            author: author.to_hex(),
            created_at: unix_seconds()?,
            group_id: group_id.into(),
            root_event_id: root_event_id.into(),
            content: "Live E2E confirmed.".into(),
            answers: vec![QuestionAnswer {
                question_id: "live-e2e".into(),
                values: vec!["confirmed".into()],
            }],
        },
    )
    .map_err(|error| error.to_string())?;
    let receipt = engine
        .publish_tracked(intent)
        .map_err(|error| error.to_string())?;
    let receipt_id = receipt.id.0;
    let event_id = await_ack(&engine, receipt_id, host, "answer")?;
    engine.shutdown();
    Ok((receipt_id, event_id, author.to_hex()))
}

fn await_ack(
    engine: &Engine,
    receipt_id: u64,
    host: &RelayUrl,
    operation: &str,
) -> Result<String, String> {
    let ReceiptReattachment::Attached(_, statuses) = engine
        .reattach_receipt(ReceiptId(receipt_id))
        .map_err(|error| error.to_string())?
    else {
        return Err(format!(
            "{operation} receipt {receipt_id} could not be reattached"
        ));
    };
    let mut event_id = None;
    let mut acknowledged = false;
    for _ in 0..32 {
        let Ok(status) = statuses.recv_timeout(Duration::from_secs(2)) else {
            continue;
        };
        match status {
            WriteStatus::Signed(id) => event_id = Some(id.to_hex()),
            WriteStatus::Acked(relay) if relay == *host => acknowledged = true,
            WriteStatus::Rejected(relay, reason) => {
                return Err(format!("{operation} was rejected by {relay}: {reason}"));
            }
            WriteStatus::GaveUp(relay) | WriteStatus::OutcomeUnknown(relay) => {
                return Err(format!("{operation} delivery was unresolved at {relay}"));
            }
            WriteStatus::Failed(reason) => {
                return Err(format!("{operation} write failed: {reason}"));
            }
            _ => {}
        }
        if acknowledged {
            if let Some(event_id) = event_id {
                return Ok(event_id);
            }
        }
    }
    Err(format!(
        "{operation} receipt {receipt_id} exceeded its bounded status stream"
    ))
}

pub(crate) fn unix_seconds() -> Result<u64, String> {
    Ok(SystemClock.unix_millis() / 1_000)
}
