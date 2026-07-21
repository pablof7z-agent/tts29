use std::time::Duration;

use nmp::{Engine, ReceiptId, ReceiptReattachment, RelayUrl, WriteStatus};

pub(crate) fn await_host_ack(
    engine: &Engine,
    receipt_id: u64,
    host: &RelayUrl,
    timeout: Duration,
    operation: &str,
) -> Result<String, String> {
    let statuses = match engine
        .reattach_receipt(ReceiptId(receipt_id))
        .map_err(|error| error.to_string())?
    {
        ReceiptReattachment::Attached(_, statuses) => statuses,
        ReceiptReattachment::NotFound => {
            return Err(format!("{operation} receipt {receipt_id} was not found"));
        }
        ReceiptReattachment::RetainedButUnreadable => {
            return Err(format!(
                "{operation} receipt {receipt_id} is retained but unreadable"
            ));
        }
    };
    let mut event_id = None;
    let mut host_acked = false;
    let mut last_status = "no status received".to_string();
    for _ in 0..32 {
        let status = statuses.recv_timeout(timeout).map_err(|_| {
            format!("{operation} receipt {receipt_id} is still pending ({last_status})")
        })?;
        last_status = format!("{status:?}");
        match status {
            WriteStatus::Signed(id) => event_id = Some(id.to_hex()),
            WriteStatus::Acked(relay) if relay == *host => host_acked = true,
            WriteStatus::Rejected(relay, reason) => {
                return Err(format!("{operation} was rejected by {relay}: {reason}"));
            }
            WriteStatus::GaveUp(relay) | WriteStatus::OutcomeUnknown(relay) => {
                return Err(format!("{operation} delivery was unresolved at {relay}"));
            }
            WriteStatus::Failed(reason) => return Err(format!("{operation} failed: {reason}")),
            WriteStatus::Cancelled => return Err(format!("{operation} was cancelled")),
            WriteStatus::ReplaceableConflict { .. } => {
                return Err(format!("{operation} encountered a replaceable conflict"));
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
        "{operation} receipt {receipt_id} exceeded its bounded status stream"
    ))
}
