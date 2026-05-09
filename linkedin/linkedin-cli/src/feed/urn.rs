//! Activity-URN extraction and resolution helpers.

use linkedin_api::models::FeedResponse;

use super::cache::load_feed_cache;

/// Pull the activity URN out of a notification cardAction URL like
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

/// Extract the inner `urn:li:activity:XXXXX` from a feed element's entityUrn.
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
pub(super) fn normalize_reactions_urn(input: &str) -> String {
    if input.starts_with("urn:li:") {
        input.to_string()
    } else {
        format!("urn:li:activity:{}", input)
    }
}

/// Resolve a literal URN string or a 1-based feed index to an activity URN.
/// Used by react/unreact/comment.
pub(super) fn resolve_post_urn(post_urn_or_index: &str) -> Result<String, String> {
    let Ok(index) = post_urn_or_index.parse::<usize>() else {
        return Ok(post_urn_or_index.to_string());
    };
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }
    let cache = load_feed_cache()?;
    let feed: FeedResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached feed: {e}"))?;
    let element = feed.elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (feed has {} items)",
            index,
            feed.elements.len()
        )
    })?;
    let entity_urn = element
        .get("entityUrn")
        .or_else(|| element.get("urn"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| "feed item has no entityUrn".to_string())?;
    extract_activity_urn(entity_urn)
        .ok_or_else(|| format!("could not extract activity URN from: {}", entity_urn))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
