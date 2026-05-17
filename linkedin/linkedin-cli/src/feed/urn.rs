//! CLI-side URN resolution wrappers.
//!
//! The pure URN parsers (`extract_activity_urn`, `extract_activity_urn_from_url`)
//! live in `linkedin_api::urn`. Only resolution logic that depends on the
//! CLI's feed cache stays here.

use crate::error::{CliError, CliResult};
use linkedin_api::models::FeedResponse;
use linkedin_api::urn::ActivityUrn;

use super::cache::load_feed_cache;

// Re-export so sibling modules can keep using `super::urn::*`.
pub(super) use linkedin_api::urn::extract_activity_urn;

/// Resolve a literal URN string or a 1-based feed index to an activity URN.
/// Used by react/unreact/comment. Reads the CLI's cached `last_feed.json`
/// when an index is supplied.
pub(super) fn resolve_post_urn(post_urn_or_index: &str) -> CliResult<ActivityUrn> {
    let Ok(index) = post_urn_or_index.parse::<usize>() else {
        return Ok(ActivityUrn::from(post_urn_or_index));
    };
    if index == 0 {
        return Err(CliError::Other("index must be >= 1".to_string()));
    }
    let cache = load_feed_cache()?;
    let feed: FeedResponse = serde_json::from_value(cache)
        .map_err(|e| CliError::Other(format!("failed to parse cached feed: {e}")))?;
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
        .ok_or_else(|| CliError::Other("feed item has no entityUrn".to_string()))?;
    extract_activity_urn(entity_urn)
        .map(ActivityUrn::from)
        .ok_or_else(|| {
            CliError::Other(format!(
                "could not extract activity URN from: {}",
                entity_urn
            ))
        })
}
