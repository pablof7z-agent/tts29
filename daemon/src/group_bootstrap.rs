use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use nmp::{CorrelationToken, Engine, LiveQuery, PublicKey, RelayUrl, RowDelta, SourceStatus};
use sha2::{Digest, Sha256};
use tts29_protocol::{
    compose_admin_upsert, compose_group_create, event_assigns_role, event_establishes_group,
    event_is_group_admin_state, group_admin_demand,
};

use crate::nmp_receipt::await_host_ack;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupBootstrapEvidence {
    pub daemon_pubkey: String,
    pub group_creation_receipt_id: Option<u64>,
    pub group_created_event_id: Option<String>,
    pub owner_admin_receipt_id: Option<u64>,
    pub owner_admin_event_id: Option<String>,
}

pub(crate) fn bootstrap_group(
    engine: &Arc<Engine>,
    host: RelayUrl,
    group_id: &str,
    daemon: PublicKey,
    owner: PublicKey,
    created_at: u64,
    timeout: Duration,
) -> Result<GroupBootstrapEvidence, String> {
    let state = observe_state(engine, host.clone(), group_id, daemon, owner, timeout)?;
    let plan = BootstrapPlan::for_state(state, daemon == owner)?;
    let daemon_hex = daemon.to_hex();
    let owner_hex = owner.to_hex();
    let mut evidence = GroupBootstrapEvidence {
        daemon_pubkey: daemon_hex.clone(),
        group_creation_receipt_id: None,
        group_created_event_id: None,
        owner_admin_receipt_id: None,
        owner_admin_event_id: None,
    };

    if plan.create_group {
        let intent = compose_group_create(
            host.clone(),
            group_id,
            &daemon_hex,
            created_at,
            correlation("create", &host, group_id, &daemon_hex)?,
        )
        .map_err(|error| error.to_string())?;
        let receipt = engine
            .publish_tracked(intent)
            .map_err(|error| format!("group creation could not be accepted by NMP: {error}"))?;
        evidence.group_creation_receipt_id = Some(receipt.id.0);
        evidence.group_created_event_id = Some(await_host_ack(
            engine,
            receipt.id.0,
            &host,
            timeout,
            "group creation",
        )?);
    }

    if plan.promote_owner {
        let intent = compose_admin_upsert(
            host.clone(),
            group_id,
            &daemon_hex,
            &owner_hex,
            "admin",
            created_at,
            correlation("owner-admin", &host, group_id, &owner_hex)?,
        )
        .map_err(|error| error.to_string())?;
        let receipt = engine
            .publish_tracked(intent)
            .map_err(|error| format!("owner promotion could not be accepted by NMP: {error}"))?;
        evidence.owner_admin_receipt_id = Some(receipt.id.0);
        evidence.owner_admin_event_id = Some(await_host_ack(
            engine,
            receipt.id.0,
            &host,
            timeout,
            "owner promotion",
        )?);
    }

    Ok(evidence)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GroupState {
    exists: bool,
    daemon_is_admin: bool,
    owner_is_admin: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BootstrapPlan {
    create_group: bool,
    promote_owner: bool,
}

impl BootstrapPlan {
    fn for_state(state: GroupState, owner_is_daemon: bool) -> Result<Self, String> {
        if !state.exists {
            return Ok(Self {
                create_group: true,
                promote_owner: !owner_is_daemon,
            });
        }
        if !state.daemon_is_admin {
            return Err(
                "configured group exists, but this daemon identity does not have the admin role"
                    .into(),
            );
        }
        Ok(Self {
            create_group: false,
            promote_owner: !state.owner_is_admin,
        })
    }
}

fn observe_state(
    engine: &Engine,
    host: RelayUrl,
    group_id: &str,
    daemon: PublicKey,
    owner: PublicKey,
    timeout: Duration,
) -> Result<GroupState, String> {
    let demand = group_admin_demand(host.clone(), group_id).map_err(|error| error.to_string())?;
    let subscription = engine
        .observe(LiveQuery(demand), None)
        .map_err(|error| format!("group administration observation failed: {error}"))?;
    let result = observe_frames(&subscription, &host, group_id, &daemon, &owner, timeout);
    subscription.cancel();
    result
}

fn observe_frames(
    subscription: &nmp::Subscription,
    host: &RelayUrl,
    group_id: &str,
    daemon: &PublicKey,
    owner: &PublicKey,
    timeout: Duration,
) -> Result<GroupState, String> {
    let mut current = BTreeMap::new();
    let mut last_evidence = "no frame received".to_string();
    for _ in 0..8 {
        let frame = subscription.recv_timeout(timeout).map_err(|_| {
            format!("group administration did not reconcile in time ({last_evidence})")
        })?;
        for delta in &frame.deltas {
            match delta {
                RowDelta::Added(row) => {
                    if !current.contains_key(&row.event.id) && current.len() >= 8 {
                        return Err(
                            "NMP returned too many current group administration states".into()
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
            .find(|source| source.relay == *host);
        if source.is_some_and(|source| {
            matches!(
                source.status,
                SourceStatus::AuthDenied | SourceStatus::Error
            )
        }) {
            return Err("group administration observation was refused by the selected host".into());
        }
        let reconciled = source.is_some_and(|source| {
            source.status == SourceStatus::Requesting && source.reconciled_through.is_some()
        });
        if reconciled {
            return Ok(GroupState {
                exists: current.values().any(|event| {
                    event_establishes_group(event, group_id)
                        || event_is_group_admin_state(event, group_id)
                }),
                daemon_is_admin: current
                    .values()
                    .any(|event| event_assigns_role(event, group_id, daemon, "admin")),
                owner_is_admin: current
                    .values()
                    .any(|event| event_assigns_role(event, group_id, owner, "admin")),
            });
        }
    }
    Err(format!(
        "group administration exceeded its bounded observation stream ({last_evidence})"
    ))
}

fn correlation(
    operation: &str,
    host: &RelayUrl,
    group_id: &str,
    subject: &str,
) -> Result<CorrelationToken, String> {
    let digest = format!(
        "{:x}",
        Sha256::digest(
            [
                b"tts29-group-bootstrap-v1\0".as_slice(),
                operation.as_bytes(),
                b"\0",
                host.as_str().as_bytes(),
                b"\0",
                group_id.as_bytes(),
                b"\0",
                subject.as_bytes(),
            ]
            .concat()
        )
    );
    CorrelationToken::try_from(digest.as_str())
        .map_err(|error| format!("group bootstrap correlation was refused: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_group_is_created_before_owner_promotion() {
        assert_eq!(
            BootstrapPlan::for_state(GroupState::default(), false).unwrap(),
            BootstrapPlan {
                create_group: true,
                promote_owner: true
            }
        );
    }

    #[test]
    fn converged_group_needs_no_writes() {
        let state = GroupState {
            exists: true,
            daemon_is_admin: true,
            owner_is_admin: true,
        };

        assert_eq!(
            BootstrapPlan::for_state(state, false).unwrap(),
            BootstrapPlan {
                create_group: false,
                promote_owner: false
            }
        );
    }

    #[test]
    fn existing_group_cannot_be_seized_by_an_untrusted_daemon() {
        let state = GroupState {
            exists: true,
            daemon_is_admin: false,
            owner_is_admin: false,
        };

        let error = BootstrapPlan::for_state(state, false).unwrap_err();

        assert!(error.contains("does not have the admin role"));
    }
}
