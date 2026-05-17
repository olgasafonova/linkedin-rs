//! Feed-shaped response models.
//!
//! `Paging` lives here because the feed types use it most heavily; all other
//! sibling modules pull it in via the parent `models` module re-export.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Standard Rest.li collection response paging metadata.
///
/// Returned as `paging` in all collection endpoints. See
/// `re/restli_protocol.md` section 7.2.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paging {
    /// 0-based offset of the current page.
    #[serde(default)]
    pub start: u32,

    /// Number of items requested (page size).
    #[serde(default)]
    pub count: u32,

    /// Total number of items available. May be absent if the server
    /// doesn't know or doesn't want to disclose.
    #[serde(default)]
    pub total: Option<u32>,

    /// HATEOAS-style links (rarely used by mobile client).
    #[serde(default)]
    pub links: Option<Vec<Value>>,
}

/// Top-level response from the `feed/updates` endpoint.
///
/// Wraps a standard Rest.li collection of `UpdateV2` items.
/// See `re/restli_protocol.md` section 7.1 for the generic structure
/// and `re/pegasus_models.md` for the `UpdateV2` model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedResponse {
    /// Array of feed update items. Each element is an `UpdateV2` record,
    /// but we keep it as `Value` until we've validated the full shape.
    // TODO(live-validation): Replace Vec<Value> with Vec<UpdateV2> once live
    // response shape is confirmed. Check if response uses entity deduplication.
    #[serde(default)]
    pub elements: Vec<Value>,

    /// Pagination metadata for this page of results.
    #[serde(default)]
    pub paging: Option<Paging>,

    /// Collection-level metadata (type varies by endpoint).
    #[serde(default)]
    pub metadata: Option<Value>,

    /// URN identifying this collection.
    #[serde(default)]
    pub entity_urn: Option<String>,
}

/// Minimal representation of an `UpdateV2` feed item.
///
/// Only the fields we actually display in the CLI are typed; everything
/// else is captured as `Option<Value>` so we don't drop unknown fields.
///
/// Reference: `re/pegasus_models.md` -- `UpdateV2 (voyager.feed.render)`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateV2 {
    /// URN identifying this feed update.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// The actor (author) component of the update.
    // TODO(live-validation): Type as ActorComponent once shape confirmed.
    // Expected: {name, image, subDescription} from decompiled ActorComponent.
    #[serde(default)]
    pub actor: Option<Value>,

    /// Post text / commentary.
    // TODO(live-validation): Type as TextComponent. May be {text: {text: "..."}}
    // wrapper or flat string. Check live response.
    #[serde(default)]
    pub commentary: Option<Value>,

    /// Content attachment (article, image, video, etc.).
    // TODO(live-validation): This is a union (FeedComponent) with variants for
    // article, image, video, etc. Need to confirm union discriminator format
    // (FQN key vs short key) before typing.
    #[serde(default)]
    pub content: Option<Value>,

    /// Social engagement metadata (likes, comments, shares).
    #[serde(default)]
    pub social_detail: Option<Value>,

    /// Update metadata (tracking, visibility, etc.).
    #[serde(default)]
    pub update_metadata: Option<Value>,

    /// Contextual header (e.g., "John Doe liked this").
    #[serde(default)]
    pub contextual_header: Option<Value>,

    /// Header component.
    #[serde(default)]
    pub header: Option<Value>,

    /// Reshared update (recursive -- contains another UpdateV2).
    // TODO(live-validation): Type as Box<UpdateV2> once confirmed. Self-referential
    // struct needs Box for serde. May hit depth limits on deeply reshared content.
    #[serde(default)]
    pub reshared_update: Option<Value>,
}

/// Social engagement metadata for a feed item.
///
/// Reference: `re/pegasus_models.md` -- `SocialDetail (voyager.feed)`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialDetail {
    /// URN for the social detail.
    #[serde(default)]
    pub urn: Option<String>,

    /// URN identifying this entity.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Aggregated activity counts (likes, comments, shares, views).
    #[serde(default)]
    pub total_social_activity_counts: Option<SocialActivityCounts>,

    /// Whether commenting is disabled.
    #[serde(default)]
    pub commenting_disabled: Option<bool>,

    /// Whether the share button is shown.
    #[serde(default)]
    pub show_share_button: Option<bool>,

    /// Thread identifier.
    #[serde(default)]
    pub thread_id: Option<String>,
}

/// Aggregated social activity counts.
///
/// Reference: `re/pegasus_models.md` -- `SocialActivityCounts (voyager.feed.shared)`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialActivityCounts {
    /// Number of likes.
    #[serde(default)]
    pub num_likes: Option<u64>,

    /// Number of comments.
    #[serde(default)]
    pub num_comments: Option<u64>,

    /// Number of shares.
    #[serde(default)]
    pub num_shares: Option<u64>,

    /// Number of views.
    #[serde(default)]
    pub num_views: Option<u64>,

    /// Whether the current user has liked this item.
    #[serde(default)]
    pub liked: Option<bool>,
}
