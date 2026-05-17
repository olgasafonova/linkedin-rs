//! Notifications-shaped response models.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Paging;

/// Top-level response from the `identity/notificationCards` endpoint.
///
/// Wraps a standard Rest.li collection of `NotificationCard` items.
/// See `re/api_endpoint_catalog.md` section 11 and `re/pegasus_models.md`
/// section 3.8 for the `Card` (NotificationCard) model definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationCardsResponse {
    /// Array of notification card items.
    #[serde(default)]
    pub elements: Vec<Value>,

    /// Pagination metadata for this page of results.
    #[serde(default)]
    pub paging: Option<Paging>,

    /// Collection-level metadata (type varies by endpoint).
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// A notification card.
///
/// Reference: `re/pegasus_models.md` -- `Card (voyager.identity.notifications)`.
/// Fields kept as `Option` since we haven't validated against live API yet.
/// The `headline` and `subHeadline` fields use LinkedIn's `TextViewModel`
/// wrapper, which typically has a `text` field containing the display string.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationCard {
    /// Entity URN identifying this notification card.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Primary headline (TextViewModel with a `text` field).
    // TODO(live-validation): Type as TextViewModel struct. Confirm shape -- may have
    // {text: "..."} or {text: "...", attributedText: {...}} with rich formatting.
    #[serde(default)]
    pub headline: Option<Value>,

    /// Secondary headline (TextViewModel with a `text` field).
    // TODO(live-validation): Same TextViewModel shape as headline.
    #[serde(default)]
    pub sub_headline: Option<Value>,

    /// Timestamp label (TextViewModel, e.g. "2h ago").
    #[serde(default)]
    pub kicker: Option<Value>,

    /// Header image (ImageViewModel).
    #[serde(default)]
    pub header_image: Option<Value>,

    /// Badge icon (ImageViewModel).
    #[serde(default)]
    pub badge_icon: Option<Value>,

    /// Content type discriminator (e.g. "PROFILE_VIEW", "REACTION", etc.).
    #[serde(default)]
    pub content_type: Option<String>,

    /// Primary content text lines (List<TextViewModel>).
    #[serde(default)]
    pub content_primary_text: Option<Vec<Value>>,

    /// Secondary content text lines (List<TextViewModel>).
    #[serde(default)]
    pub content_secondary_text: Option<Vec<Value>>,

    /// Content images (List<ImageViewModel>).
    #[serde(default)]
    pub content_images: Option<Vec<Value>>,

    /// Total count of content images.
    #[serde(default)]
    pub content_images_total_count: Option<i32>,

    /// Actions available on this card (List<CardAction>).
    #[serde(default)]
    pub actions: Option<Vec<Value>>,

    /// Primary card action (CardAction).
    #[serde(default)]
    pub card_action: Option<Value>,

    /// Content action (CardAction).
    #[serde(default)]
    pub content_action: Option<Value>,

    /// Insight action (CardAction).
    #[serde(default)]
    pub insight_action: Option<Value>,

    /// Insight text (TextViewModel).
    #[serde(default)]
    pub insight: Option<Value>,

    /// Insight type discriminator.
    #[serde(default)]
    pub insight_type: Option<String>,

    /// Timestamp when the notification was published (epoch millis).
    #[serde(default)]
    pub published_at: Option<i64>,

    /// Whether this notification has been read.
    #[serde(default)]
    pub read: Option<bool>,

    /// Tracking object for analytics.
    #[serde(default)]
    pub tracking_object: Option<Value>,

    /// Social activity counts (likes, comments, etc.).
    #[serde(default)]
    pub social_activity_counts: Option<Value>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}
