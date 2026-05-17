//! Search-shaped response models.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Paging;

/// Top-level response from the `search/hits` endpoint.
///
/// Wraps a standard Rest.li collection of search hit items.
/// See `re/search_protocol.md` section 7 for the response model structure.
///
/// The response may contain `SearchCluster` elements (when using `search/cluster`)
/// or flat `SearchHit` elements (when using `search/hits`). We keep `elements`
/// as `Vec<Value>` because the hit payloads use Rest.li unions (`hitInfo`)
/// that are polymorphic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// Array of search result items (SearchHit or SearchCluster).
    #[serde(default)]
    pub elements: Vec<Value>,

    /// Pagination metadata for this page of results.
    #[serde(default)]
    pub paging: Option<Paging>,

    /// Search-level metadata (keywords echo, facets, total count, etc.).
    /// See `re/search_protocol.md` section 7.4 (SearchMetadata).
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// A single search hit from a people/company/content search.
///
/// Reference: `re/search_protocol.md` section 7.2 (SearchHit).
/// The `hit_info` field is a Rest.li union -- exactly one member is present,
/// discriminated by a type key (e.g. `com.linkedin.voyager.search.SearchProfile`).
/// We keep it as `Value` until we've validated the live response shapes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// Tracking identifier for analytics.
    #[serde(default)]
    pub tracking_id: Option<String>,

    /// Target page instance identifier.
    #[serde(default)]
    pub target_page_instance: Option<String>,

    /// The actual result payload (union: SearchProfile, SearchJob, etc.).
    /// The union key indicates the result type.
    // TODO(live-validation): Union type. Confirm discriminator key format and
    // which variants appear for people search vs job search vs content search.
    #[serde(default)]
    pub hit_info: Option<Value>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}
