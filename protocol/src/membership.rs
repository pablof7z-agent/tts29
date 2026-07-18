use std::collections::{BTreeMap, BTreeSet};

use nmp::{
    AccessContext, Binding, CacheMode, CorrelationToken, Demand, Event, Filter, IndexedTagName,
    PublicKey, RelayUrl, SourceAuthority, Timestamp, WriteIntent,
};
use nmp_nip29::{compose_group_send, GroupTimelineEvidence};

use crate::parse::tags::bounded;

const MEMBER_STATE_KINDS: [u16; 2] = [39_001, 39_002];
const PUT_USER_KIND: u16 = 9_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MembershipError {
    InvalidGroup,
    InvalidAdmin,
    InvalidMember,
    InvalidTag,
}

impl std::fmt::Display for MembershipError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGroup => output.write_str("the group id is invalid"),
            Self::InvalidAdmin => output.write_str("the group admin is not a public key"),
            Self::InvalidMember => output.write_str("the group member is not a public key"),
            Self::InvalidTag => output.write_str("NMP refused a generated membership tag"),
        }
    }
}

impl std::error::Error for MembershipError {}

pub fn group_membership_demand(host: RelayUrl, group_id: &str) -> Result<Demand, MembershipError> {
    validate_group(group_id)?;
    let group_tag = IndexedTagName::new('d').expect("d is an indexed Nostr tag");
    let mut demand = Demand::new(
        Filter {
            kinds: Some(BTreeSet::from(MEMBER_STATE_KINDS)),
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

pub fn event_authorizes_member(event: &Event, group_id: &str, member: &PublicKey) -> bool {
    MEMBER_STATE_KINDS.contains(&event.kind.as_u16())
        && event.tags.iter().any(|tag| tag_matches(tag, "d", group_id))
        && event
            .tags
            .iter()
            .any(|tag| tag_matches(tag, "p", &member.to_hex()))
}

pub fn compose_membership_upsert(
    host: RelayUrl,
    group_id: &str,
    admin: &str,
    member: &str,
    created_at: u64,
    correlation: CorrelationToken,
) -> Result<WriteIntent, MembershipError> {
    validate_group(group_id)?;
    let admin = PublicKey::parse(admin).map_err(|_| MembershipError::InvalidAdmin)?;
    let member = PublicKey::parse(member).map_err(|_| MembershipError::InvalidMember)?;
    let mut intent = compose_group_send(
        host,
        group_id,
        admin,
        Timestamp::from(created_at),
        PUT_USER_KIND,
        String::new(),
        vec![vec!["p".into(), member.to_hex()]],
        &GroupTimelineEvidence::none(),
    )
    .map_err(|_| MembershipError::InvalidTag)?;
    intent.correlation = Some(correlation);
    Ok(intent)
}

fn validate_group(group_id: &str) -> Result<(), MembershipError> {
    bounded(group_id, 128)
        .map(|_| ())
        .ok_or(MembershipError::InvalidGroup)
}

#[cfg(test)]
fn tags_authorize_member(kind: u16, tags: &[nmp::Tag], group_id: &str, member: &PublicKey) -> bool {
    MEMBER_STATE_KINDS.contains(&kind)
        && has_tag(tags, "d", group_id)
        && has_tag(tags, "p", &member.to_hex())
}

#[cfg(test)]
fn has_tag(tags: &[nmp::Tag], name: &str, value: &str) -> bool {
    tags.iter().any(|tag| tag_matches(tag, name, value))
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
    fn membership_demand_is_bounded_to_state_on_one_host() {
        let host = relay();
        let demand = group_membership_demand(host.clone(), "tts").unwrap();

        assert_eq!(
            demand.selection.kinds,
            Some(BTreeSet::from(MEMBER_STATE_KINDS))
        );
        assert_eq!(
            demand.source,
            SourceAuthority::Pinned(BTreeSet::from([host]))
        );
        assert_eq!(demand.cache, CacheMode::Strict);
    }

    #[test]
    fn membership_upsert_uses_nmp_group_context_and_daemon_identity() {
        let admin = key('1');
        let member = key('2');
        let token = CorrelationToken::try_from("membership-request").unwrap();

        let intent =
            compose_membership_upsert(relay(), "tts", &admin, &member, 100, token).unwrap();
        let WritePayload::Unsigned(event) = intent.payload else {
            panic!("membership upserts must be unsigned")
        };

        assert_eq!(event.kind, Kind::from(PUT_USER_KIND));
        assert_eq!(event.pubkey.to_hex(), admin);
        assert!(event.tags.iter().any(|tag| tag.as_slice() == ["h", "tts"]));
        assert!(event
            .tags
            .iter()
            .any(|tag| tag.as_slice() == ["p", &member]));
        assert!(matches!(intent.routing, WriteRouting::PinnedHost(_)));
        assert!(intent.identity_override.is_none());
        assert_eq!(intent.correlation.unwrap().as_ref(), "membership-request");
    }

    #[test]
    fn only_current_host_state_authorizes_a_member() {
        let member = PublicKey::parse(&key('2')).unwrap();
        let member_hex = member.to_hex();
        let tags = vec![
            Tag::parse(["d", "tts"]).unwrap(),
            Tag::parse(["p", member_hex.as_str()]).unwrap(),
        ];

        assert!(tags_authorize_member(39_002, &tags, "tts", &member));
        assert!(!tags_authorize_member(39_002, &tags, "another", &member));
        assert!(!tags_authorize_member(9_000, &tags, "tts", &member));
    }

    fn relay() -> RelayUrl {
        RelayUrl::parse("wss://relay.example.com").unwrap()
    }

    fn key(character: char) -> String {
        character.to_string().repeat(64)
    }
}
