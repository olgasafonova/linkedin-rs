//! CLI-side URN resolution wrappers.
//!
//! The pure URN parsers (`extract_activity_urn`, `extract_activity_urn_from_url`,
//! `normalize_reactions_urn`) live in `linkedin_api::urn`. Only resolution
//! logic that depends on the CLI's feed cache stays here.

use linkedin_api::models::FeedResponse;

use super::cache::load_feed_cache;

// Re-exports so sibling modules can keep using `super::urn::*`. The
// `extract_activity_urn` re-export is also what `resolve_post_urn` below
// resolves against — `pub use` brings the name into scope here too.
pub(super) use linkedin_api::urn::{extract_activity_urn, normalize_reactions_urn};

/// Resolve a literal URN string or a 1-based feed index to an activity URN.
/// Used by react/unreact/comment. Reads the CLI's cached `last_feed.json`
/// when an index is supplied.
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
