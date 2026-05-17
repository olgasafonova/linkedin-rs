//! LinkedIn URN parsing and JSON-shape extraction.
//!
//! Everything in this module is pure logic over LinkedIn-domain shapes:
//! string-walking activity URNs, recursing through profile-URN trees,
//! reading `updateMetadata` from a feed element. None of it depends on the
//! CLI or on any I/O.
//!
//! Also defines transparent newtype wrappers around LinkedIn URN strings
//! (`ActivityUrn`, `ProfileUrn`, etc.) so per-resource client methods can
//! take a typed argument instead of `&str`. Construction normalizes where
//! a normalization rule exists (e.g. activity URNs accept a bare numeric id
//! and wrap it as `urn:li:activity:<id>`); other newtypes are pass-through.

use std::fmt;

use serde_json::Value;

use crate::restli::unwrap_update_v2;

macro_rules! pass_through_urn {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self::new(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self::new(s.to_string())
            }
        }
    };
}

/// Activity URN. Accepts a full `urn:li:activity:N` URN, any other
/// `urn:li:` URN (passed through unchanged — the reactions endpoint also
/// accepts `urn:li:ugcPost:N` and `urn:li:share:N`), or a bare numeric id
/// (wrapped as `urn:li:activity:<id>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActivityUrn(String);

impl ActivityUrn {
    /// Construct from any string-like input, normalizing bare numeric ids
    /// to `urn:li:activity:<id>`.
    pub fn new(s: impl AsRef<str>) -> Self {
        let raw = s.as_ref();
        let normalized = if raw.starts_with("urn:li:") {
            raw.to_string()
        } else {
            format!("urn:li:activity:{}", raw)
        };
        Self(normalized)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    /// Strip the `urn:li:activity:` prefix if present. Used by callers that
    /// need the bare numeric id (e.g. `get_post` searches by id within the
    /// feed window).
    pub fn activity_id(&self) -> &str {
        self.0.strip_prefix("urn:li:activity:").unwrap_or(&self.0)
    }
}

impl AsRef<str> for ActivityUrn {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActivityUrn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ActivityUrn {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for ActivityUrn {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

pass_through_urn! {
    /// `urn:li:fsd_profile:<id>` — modern profile URN used for messaging
    /// recipients, connection requests, mailbox identification.
    ProfileUrn
}

pass_through_urn! {
    /// `urn:li:msg_conversation:(<profile>,<thread>)` or a bare messaging
    /// thread id (`2-abc123...`). Both forms are accepted because LinkedIn
    /// surfaces the bare thread id in several places and clients normalize
    /// it back to a full URN before issuing GraphQL queries.
    ConversationUrn
}

pass_through_urn! {
    /// `urn:li:invitation:<id>` — pending connection invitations.
    InvitationUrn
}

pass_through_urn! {
    /// `urn:li:fs_socialDetail:(<activity>,...)` — passed to the comments
    /// finder. Used as-is, no normalization.
    SocialDetailUrn
}

/// Pull an activity URN out of a URL like
/// `/feed/update/urn:li:activity:7312345/?...` (URL- or percent-encoded).
pub fn extract_activity_urn_from_url(url: &str) -> Option<String> {
    let decoded = url.replace("%3A", ":").replace("%2F", "/");
    let prefix = "urn:li:activity:";
    let start = decoded.find(prefix)?;
    let tail = &decoded[start..];
    let end = tail
        .find(|c: char| !c.is_ascii_digit() && c != ':' && !c.is_ascii_alphabetic())
        .unwrap_or(tail.len());
    let urn = &tail[..end];
    if urn.len() > prefix.len() {
        Some(urn.to_string())
    } else {
        None
    }
}

/// Extract the inner `urn:li:activity:XXXXX` from a feed element's entityUrn
/// (which looks like `urn:li:fs_updateV2:(urn:li:activity:NNN,SUFFIX,…)`).
///
/// The end of the URN is detected as the first character that can't appear in
/// a URN — comma, closing paren, whitespace, etc. The earlier version of this
/// function only stopped on `)`, which left trailing `,MEMBER_SHARES,…` in
/// the result; callers (notably `feed/my_posts.rs`) had to strip it
/// themselves with `.split(',').next()`.
pub fn extract_activity_urn(feed_entity_urn: &str) -> Option<String> {
    let start = feed_entity_urn.find("urn:li:activity:")?;
    let rest = &feed_entity_urn[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.')))
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Recursively search a JSON value for a string that starts with
/// `urn:li:fsd_profile:`. Returns the first match in DFS order.
pub fn find_fsd_profile_urn(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if s.starts_with("urn:li:fsd_profile:") => Some(s.clone()),
        Value::Object(map) => map.values().find_map(find_fsd_profile_urn),
        Value::Array(arr) => arr.iter().find_map(find_fsd_profile_urn),
        _ => None,
    }
}

/// Pick the right thread URN for the reactions endpoint from a feed element.
/// ugcPost-backed posts require the ugcPost URN, share-backed posts require
/// the activity URN. Accepts both the `value.UpdateV2`-wrapped and the bare
/// UpdateV2 shape.
pub fn extract_reactions_urn(element: &Value) -> Option<String> {
    let update = unwrap_update_v2(element);
    let metadata = update.get("updateMetadata")?;
    let activity_urn = metadata.get("urn").and_then(|u| u.as_str());
    let share_urn = metadata.get("shareUrn").and_then(|u| u.as_str());

    match (share_urn, activity_urn) {
        (Some(s), _) if s.starts_with("urn:li:ugcPost:") => Some(s.to_string()),
        (_, Some(a)) => Some(a.to_string()),
        (Some(s), None) => Some(s.to_string()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_activity_urn_from_url_handles_plain_path() {
        assert_eq!(
            extract_activity_urn_from_url("/feed/update/urn:li:activity:7312345/"),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_from_url_handles_percent_encoded() {
        assert_eq!(
            extract_activity_urn_from_url(
                "/feed/update/urn%3Ali%3Aactivity%3A7312345/?trackingId=foo"
            ),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_from_url_returns_none_for_unrelated() {
        assert_eq!(extract_activity_urn_from_url("/in/some-profile/"), None);
    }

    #[test]
    fn extract_activity_urn_returns_none_when_no_inner_activity() {
        assert!(extract_activity_urn("urn:li:fsd_profile:abc").is_none());
    }

    #[test]
    fn extract_activity_urn_stops_at_paren() {
        assert_eq!(
            extract_activity_urn("/path/urn:li:activity:7312345)"),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_strips_member_shares_suffix() {
        // The real Voyager entityUrn shape: a wrapped fs_updateV2 with the
        // activity URN as the first comma-separated field. We want only the
        // activity URN, not the comma-tail of trackers / backing-types.
        assert_eq!(
            extract_activity_urn(
                "urn:li:fs_updateV2:(urn:li:activity:7312345,MEMBER_SHARES,DEFAULT,false)"
            ),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_stops_at_whitespace() {
        assert_eq!(
            extract_activity_urn("see urn:li:activity:7312345 for details"),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn find_fsd_profile_urn_walks_nested_shapes() {
        let v = json!({
            "outer": {
                "list": [
                    {"other": "urn:li:member:1"},
                    {"inner": "urn:li:fsd_profile:ACoAA_target"}
                ]
            }
        });
        assert_eq!(
            find_fsd_profile_urn(&v),
            Some("urn:li:fsd_profile:ACoAA_target".to_string())
        );
    }

    #[test]
    fn find_fsd_profile_urn_returns_none_when_absent() {
        let v = json!({"foo": "urn:li:activity:123"});
        assert!(find_fsd_profile_urn(&v).is_none());
    }

    #[test]
    fn extract_reactions_urn_picks_correct_urn_per_post_backing() {
        fn wrapped(activity: &str, share: &str) -> Value {
            json!({
                "value": {
                    "com.linkedin.voyager.feed.render.UpdateV2": {
                        "updateMetadata": { "urn": activity, "shareUrn": share }
                    }
                }
            })
        }
        let cases = [
            (
                "urn:li:activity:7450891298279972864",
                "urn:li:ugcPost:7450888187100639232",
                "urn:li:ugcPost:7450888187100639232",
            ),
            (
                "urn:li:activity:7450895500045598720",
                "urn:li:share:7450895499496202240",
                "urn:li:activity:7450895500045598720",
            ),
        ];
        for (activity, share, expected) in cases {
            let element = wrapped(activity, share);
            assert_eq!(
                extract_reactions_urn(&element),
                Some(expected.to_string()),
                "activity={activity} share={share}"
            );
        }
    }

    #[test]
    fn extract_reactions_urn_handles_unwrapped_update_element() {
        let element = json!({
            "updateMetadata": {
                "urn": "urn:li:activity:7450808005048094720",
                "shareUrn": "urn:li:ugcPost:7450808001881534464"
            }
        });
        assert_eq!(
            extract_reactions_urn(&element),
            Some("urn:li:ugcPost:7450808001881534464".to_string())
        );
    }

    #[test]
    fn extract_reactions_urn_returns_none_when_metadata_missing() {
        let element = json!({"foo": "bar"});
        assert_eq!(extract_reactions_urn(&element), None);
    }

    #[test]
    fn activity_urn_passes_full_urn_through() {
        assert_eq!(
            ActivityUrn::new("urn:li:activity:7450808005048094720").as_str(),
            "urn:li:activity:7450808005048094720"
        );
    }

    #[test]
    fn activity_urn_passes_ugcpost_urn_through() {
        // The reactions endpoint accepts ugcPost URNs as threadUrn.
        assert_eq!(
            ActivityUrn::new("urn:li:ugcPost:7450808001881534464").as_str(),
            "urn:li:ugcPost:7450808001881534464"
        );
    }

    #[test]
    fn activity_urn_wraps_bare_numeric_id() {
        assert_eq!(
            ActivityUrn::new("7450808005048094720").as_str(),
            "urn:li:activity:7450808005048094720"
        );
    }

    #[test]
    fn activity_urn_activity_id_strips_prefix() {
        let urn = ActivityUrn::new("urn:li:activity:7450808005048094720");
        assert_eq!(urn.activity_id(), "7450808005048094720");
    }

    #[test]
    fn activity_urn_activity_id_returns_inner_for_non_activity_urn() {
        let urn = ActivityUrn::new("urn:li:ugcPost:1234");
        assert_eq!(urn.activity_id(), "urn:li:ugcPost:1234");
    }

    #[test]
    fn activity_urn_display_matches_as_str() {
        let urn = ActivityUrn::new("urn:li:activity:42");
        assert_eq!(format!("{}", urn), urn.as_str());
    }

    #[test]
    fn profile_urn_passes_through_unchanged() {
        let p = ProfileUrn::new("urn:li:fsd_profile:ACoAAA111");
        assert_eq!(p.as_str(), "urn:li:fsd_profile:ACoAAA111");
        assert_eq!(format!("{}", p), "urn:li:fsd_profile:ACoAAA111");
    }

    #[test]
    fn conversation_urn_accepts_full_urn_or_bare_id() {
        assert_eq!(
            ConversationUrn::new("urn:li:msg_conversation:(urn:li:fsd_profile:X,2-abc)").as_str(),
            "urn:li:msg_conversation:(urn:li:fsd_profile:X,2-abc)"
        );
        assert_eq!(ConversationUrn::new("2-abc123").as_str(), "2-abc123");
    }

    #[test]
    fn invitation_urn_passes_through() {
        assert_eq!(
            InvitationUrn::new("urn:li:invitation:7000").as_str(),
            "urn:li:invitation:7000"
        );
    }

    #[test]
    fn social_detail_urn_passes_through() {
        let s = "urn:li:fs_socialDetail:(urn:li:activity:42,urn:li:activity:42,EMPTY)";
        assert_eq!(SocialDetailUrn::new(s).as_str(), s);
    }

    #[test]
    fn from_str_and_string_construct_equivalently() {
        let a: ActivityUrn = "urn:li:activity:1".into();
        let b: ActivityUrn = String::from("urn:li:activity:1").into();
        assert_eq!(a, b);
    }
}
