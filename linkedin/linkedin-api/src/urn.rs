//! LinkedIn URN parsing and JSON-shape extraction.
//!
//! Everything in this module is pure logic over LinkedIn-domain shapes:
//! string-walking activity URNs, recursing through profile-URN trees,
//! reading `updateMetadata` from a feed element. None of it depends on the
//! CLI or on any I/O.

use serde_json::Value;

use crate::restli::unwrap_update_v2;

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
/// (which looks like `urn:li:fs_updateV2:(urn:li:activity:NNN,…)`).
pub fn extract_activity_urn(feed_entity_urn: &str) -> Option<String> {
    if let Some(start) = feed_entity_urn.find("urn:li:activity:") {
        let rest = &feed_entity_urn[start..];
        let end = rest.find(')').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

/// Normalize a user-supplied reactions URN. Accepts full URNs (any type) or a
/// bare activity ID (digits only), which is wrapped as `urn:li:activity:...`.
pub fn normalize_reactions_urn(input: &str) -> String {
    if input.starts_with("urn:li:") {
        input.to_string()
    } else {
        format!("urn:li:activity:{}", input)
    }
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
        // The function stops at ')' — note that this means the URN+suffix
        // shape `(urn:li:activity:NNN,MEMBER_SHARES)` returns
        // `urn:li:activity:NNN,MEMBER_SHARES`, not `urn:li:activity:NNN`.
        // Existing behavior preserved as-is on the move; see follow-up bead
        // for the comma-stripping question.
        assert_eq!(
            extract_activity_urn("/path/urn:li:activity:7312345)"),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn normalize_reactions_urn_passes_full_urn_through() {
        assert_eq!(
            normalize_reactions_urn("urn:li:activity:7450808005048094720"),
            "urn:li:activity:7450808005048094720"
        );
        assert_eq!(
            normalize_reactions_urn("urn:li:ugcPost:7450808001881534464"),
            "urn:li:ugcPost:7450808001881534464"
        );
    }

    #[test]
    fn normalize_reactions_urn_wraps_bare_id_as_activity() {
        assert_eq!(
            normalize_reactions_urn("7450808005048094720"),
            "urn:li:activity:7450808005048094720"
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
}
