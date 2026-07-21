use std::collections::{BTreeMap, BTreeSet};

use nmp::{
    AccessContext, Binding, CacheMode, CorrelationToken, Demand, Event, Filter, IndexedTagName,
    PublicKey, RelayUrl, SourceAuthority, Timestamp, WriteIntent,
};
use nmp_nip29::{compose_group_send, GroupTimelineEvidence};

use crate::parse::tags::bounded;

const GROUP_METADATA_KIND: u16 = 39_000;
const GROUP_ADMINS_KIND: u16 = 39_001;
const PUT_USER_KIND: u16 = 9_000;
const CREATE_GROUP_KIND: u16 = 9_007;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupAdminError {
    InvalidGroup,
    InvalidDaemon,
    InvalidOwner,
    InvalidRole,
    InvalidTag,
}

impl std::fmt::Display for GroupAdminError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGroup => output.write_str("the group id is invalid"),
            Self::InvalidDaemon => output.write_str("the daemon identity is not a public key"),
            Self::InvalidOwner => output.write_str("the owner identity is not a public key"),
            Self::InvalidRole => output.write_str("the owner role is invalid"),
            Self::InvalidTag => output.write_str("NMP refused a generated group-management tag"),
        }
    }
}

impl std::error::Error for GroupAdminError {}

pub fn group_admin_demand(host: RelayUrl, group_id: &str) -> Result<Demand, GroupAdminError> {
    validate_group(group_id)?;
    let group_tag = IndexedTagName::new('d').expect("d is an indexed Nostr tag");
    let mut demand = Demand::new(
        Filter {
            kinds: Some(BTreeSet::from([GROUP_METADATA_KIND, GROUP_ADMINS_KIND])),
            tags: BTreeMap::from([(
                group_tag,
                Binding::Literal(BTreeSet::from([group_id.to_string()])),
            )]),
            ..Filter::default()
        },
        SourceAuthority::Pinned(BTreeSet::from([host])),
        AccessContext::Public,
    )
    .expect("one pinned host is always a valid NMP demand");
    demand.cache = CacheMode::Strict;
    Ok(demand)
}

pub fn event_establishes_group(event: &Event, group_id: &str) -> bool {
    event.kind.as_u16() == GROUP_METADATA_KIND
        && event.tags.iter().any(|tag| tag_matches(tag, "d", group_id))
}

pub fn event_is_group_admin_state(event: &Event, group_id: &str) -> bool {
    event.kind.as_u16() == GROUP_ADMINS_KIND
        && event.tags.iter().any(|tag| tag_matches(tag, "d", group_id))
}

pub fn event_assigns_role(event: &Event, group_id: &str, user: &PublicKey, role: &str) -> bool {
    let user_hex = user.to_hex();
    event_is_group_admin_state(event, group_id)
        && event.tags.iter().any(|tag| {
            let row = tag.as_slice();
            row.first().map(String::as_str) == Some("p")
                && row.get(1).map(String::as_str) == Some(user_hex.as_str())
                && row.iter().skip(2).any(|value| value == role)
        })
}

pub fn compose_group_create(
    host: RelayUrl,
    group_id: &str,
    daemon: &str,
    created_at: u64,
    correlation: CorrelationToken,
) -> Result<WriteIntent, GroupAdminError> {
    validate_group(group_id)?;
    let daemon = PublicKey::parse(daemon).map_err(|_| GroupAdminError::InvalidDaemon)?;
    let mut intent = compose_group_send(
        host,
        group_id,
        daemon,
        Timestamp::from(created_at),
        CREATE_GROUP_KIND,
        "TTS29 durable spoken queue".into(),
        Vec::new(),
        &GroupTimelineEvidence::none(),
    )
    .map_err(|_| GroupAdminError::InvalidTag)?;
    intent.correlation = Some(correlation);
    Ok(intent)
}

#[allow(clippy::too_many_arguments)]
pub fn compose_admin_upsert(
    host: RelayUrl,
    group_id: &str,
    daemon: &str,
    owner: &str,
    role: &str,
    created_at: u64,
    correlation: CorrelationToken,
) -> Result<WriteIntent, GroupAdminError> {
    validate_group(group_id)?;
    let daemon = PublicKey::parse(daemon).map_err(|_| GroupAdminError::InvalidDaemon)?;
    let owner = PublicKey::parse(owner).map_err(|_| GroupAdminError::InvalidOwner)?;
    let role = bounded(role, 64).ok_or(GroupAdminError::InvalidRole)?;
    let mut intent = compose_group_send(
        host,
        group_id,
        daemon,
        Timestamp::from(created_at),
        PUT_USER_KIND,
        String::new(),
        vec![vec!["p".into(), owner.to_hex(), role]],
        &GroupTimelineEvidence::none(),
    )
    .map_err(|_| GroupAdminError::InvalidTag)?;
    intent.correlation = Some(correlation);
    Ok(intent)
}

fn validate_group(group_id: &str) -> Result<(), GroupAdminError> {
    bounded(group_id, 128)
        .map(|_| ())
        .ok_or(GroupAdminError::InvalidGroup)
}

fn tag_matches(tag: &nmp::Tag, name: &str, value: &str) -> bool {
    tag.as_slice().first().map(String::as_str) == Some(name)
        && tag.as_slice().get(1).map(String::as_str) == Some(value)
}

#[cfg(test)]
mod tests {
    use nmp::{Kind, Tag, WritePayload, WriteRouting};

    use super::*;

    #[test]
    fn bootstrap_demand_is_bounded_to_one_group_and_host() {
        let host = relay();
        let demand = group_admin_demand(host.clone(), "tts").unwrap();

        assert_eq!(
            demand.selection.kinds,
            Some(BTreeSet::from([GROUP_METADATA_KIND, GROUP_ADMINS_KIND]))
        );
        assert_eq!(
            demand.source,
            SourceAuthority::Pinned(BTreeSet::from([host]))
        );
        assert_eq!(demand.cache, CacheMode::Strict);
    }

    #[test]
    fn create_group_uses_daemon_identity_and_selected_host() {
        let daemon = key('1');
        let correlation = CorrelationToken::try_from("create-group").unwrap();
        let intent = compose_group_create(relay(), "tts", &daemon, 100, correlation).unwrap();
        let WritePayload::Unsigned(event) = intent.payload else {
            panic!("group creation must be unsigned")
        };

        assert_eq!(event.kind, Kind::from(CREATE_GROUP_KIND));
        assert_eq!(event.pubkey.to_hex(), daemon);
        assert!(event.tags.iter().any(|tag| tag.as_slice() == ["h", "tts"]));
        assert!(matches!(intent.routing, WriteRouting::PinnedHost(_)));
        assert_eq!(intent.correlation.unwrap().as_ref(), "create-group");
    }

    #[test]
    fn owner_upsert_assigns_the_admin_role() {
        let daemon = key('1');
        let owner = key('2');
        let correlation = CorrelationToken::try_from("owner-admin").unwrap();
        let intent =
            compose_admin_upsert(relay(), "tts", &daemon, &owner, "admin", 100, correlation)
                .unwrap();
        let WritePayload::Unsigned(event) = intent.payload else {
            panic!("admin updates must be unsigned")
        };

        assert_eq!(event.kind, Kind::from(PUT_USER_KIND));
        assert_eq!(event.pubkey.to_hex(), daemon);
        assert!(event
            .tags
            .iter()
            .any(|tag| { tag.as_slice() == ["p", owner.as_str(), "admin"] }));
        assert!(intent.identity_override.is_none());
    }

    #[test]
    fn role_matching_requires_group_admin_state_and_exact_role() {
        let owner = PublicKey::parse(&key('2')).unwrap();
        let owner_hex = owner.to_hex();
        let tags = vec![
            Tag::parse(["d", "tts"]).unwrap(),
            Tag::parse(["p", owner_hex.as_str(), "admin"]).unwrap(),
        ];

        assert!(tags_assign_role(
            GROUP_ADMINS_KIND,
            &tags,
            "tts",
            &owner,
            "admin"
        ));
        assert!(!tags_assign_role(
            GROUP_ADMINS_KIND,
            &tags,
            "tts",
            &owner,
            "moderator"
        ));
        assert!(!tags_assign_role(
            GROUP_METADATA_KIND,
            &tags,
            "tts",
            &owner,
            "admin"
        ));
    }

    fn tags_assign_role(
        kind: u16,
        tags: &[Tag],
        group_id: &str,
        user: &PublicKey,
        role: &str,
    ) -> bool {
        let user_hex = user.to_hex();
        kind == GROUP_ADMINS_KIND
            && tags.iter().any(|tag| tag_matches(tag, "d", group_id))
            && tags.iter().any(|tag| {
                let row = tag.as_slice();
                row.first().map(String::as_str) == Some("p")
                    && row.get(1).map(String::as_str) == Some(user_hex.as_str())
                    && row.iter().skip(2).any(|value| value == role)
            })
    }

    fn relay() -> RelayUrl {
        RelayUrl::parse("wss://relay.example.com").unwrap()
    }

    fn key(character: char) -> String {
        character.to_string().repeat(64)
    }
}
