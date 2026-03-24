//! Data models for LinkedIn API responses.
//!
//! Maps LinkedIn's Rest.li response format into typed Rust structs.
//! These models are intentionally loose (heavy use of `Option<T>` and
//! `Option<Value>`) because we haven't validated them against the live API yet.
//! Fields will be tightened as we confirm the actual response shapes.
//!
//! See `re/model_corrections.md` for the full list of known risks, validation
//! checklist, and per-field correction log.
//!
//! Reference: `re/pegasus_models.md`, `re/restli_protocol.md` section 7.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Standard Rest.li collection response paging metadata.
///
/// Returned as `paging` in all collection endpoints. See
/// `re/restli_protocol.md` section 7.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Lightweight profile used everywhere as an embedded reference.
///
/// Reference: `re/pegasus_models.md` -- `MiniProfile (voyager.identity.shared)`.
/// Appears inside `Me`, `Profile`, `Conversation` participants, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniProfile {
    /// Entity URN, e.g. `urn:li:fs_miniProfile:ACoAABxxxxxx`.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// First name.
    #[serde(default)]
    pub first_name: Option<String>,

    /// Last name.
    #[serde(default)]
    pub last_name: Option<String>,

    /// URL slug (vanity name), e.g. `john-doe-123`.
    #[serde(default)]
    pub public_identifier: Option<String>,

    /// Current role headline (labelled `occupation` in the API).
    #[serde(default)]
    pub occupation: Option<String>,

    /// Object URN, e.g. `urn:li:member:123456`.
    #[serde(default)]
    pub object_urn: Option<String>,

    /// Base64 tracking token.
    #[serde(default)]
    pub tracking_id: Option<String>,

    /// Profile photo.
    // TODO(live-validation): This is an Image union (VectorImage, MediaProxyImage,
    // URL string, MediaProcessorImage). Confirm union discriminator format --
    // FQN key like "com.linkedin.common.VectorImage" or short key "vectorImage".
    #[serde(default)]
    pub picture: Option<Value>,

    /// Background/banner image.
    // TODO(live-validation): Same Image union as picture. See above.
    #[serde(default)]
    pub background_image: Option<Value>,
}

/// Full profile with all details (~30+ fields).
///
/// Reference: `re/pegasus_models.md` -- `Profile (voyager.identity.profile)`.
/// Returned by `GET /voyager/api/identity/profiles/{id}` with decoration.
/// Fields kept as `Option` since we haven't validated all shapes against
/// the live API yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Entity URN.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// First name.
    #[serde(default)]
    pub first_name: Option<String>,

    /// Last name.
    #[serde(default)]
    pub last_name: Option<String>,

    /// Maiden name.
    #[serde(default)]
    pub maiden_name: Option<String>,

    /// Headline (distinct from MiniProfile's `occupation`).
    #[serde(default)]
    pub headline: Option<String>,

    /// "About" section.
    #[serde(default)]
    pub summary: Option<String>,

    /// Industry name.
    #[serde(default)]
    pub industry_name: Option<String>,

    /// Industry URN.
    #[serde(default)]
    pub industry_urn: Option<String>,

    /// Location name (e.g. "San Francisco Bay Area").
    #[serde(default)]
    pub location_name: Option<String>,

    /// Geo location name.
    #[serde(default)]
    pub geo_location_name: Option<String>,

    /// Geo country name.
    #[serde(default)]
    pub geo_country_name: Option<String>,

    /// Geo country URN.
    #[serde(default)]
    pub geo_country_urn: Option<String>,

    /// Structured location.
    // TODO(live-validation): Type as ProfileLocation struct (countryCode, postalCode, etc.).
    #[serde(default)]
    pub location: Option<Value>,

    /// Structured geo location.
    // TODO(live-validation): Type as ProfileGeoLocation struct (geoUrn, etc.).
    #[serde(default)]
    pub geo_location: Option<Value>,

    /// Embedded mini profile reference.
    // TODO(live-validation): Type as MiniProfile once confirmed it's inlined (not a
    // URN reference in deduped responses). May need entity resolution if deduped.
    #[serde(default)]
    pub mini_profile: Option<Value>,

    /// Profile picture.
    // TODO(live-validation): Type as PhotoFilterPicture -- may wrap Image union with
    // an extra layer. Confirm actual nesting depth in live response.
    #[serde(default)]
    pub profile_picture: Option<Value>,

    /// Background image.
    // TODO(live-validation): May be BackgroundImage struct or direct Image union.
    #[serde(default)]
    pub background_image: Option<Value>,

    /// Whether this member is a student.
    #[serde(default)]
    pub student: Option<bool>,

    /// Version tag for optimistic concurrency.
    #[serde(default)]
    pub version_tag: Option<String>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// Work experience entry.
///
/// Reference: `re/pegasus_models.md` -- `Position (voyager.identity.profile)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Entity URN.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Job title.
    #[serde(default)]
    pub title: Option<String>,

    /// Company name.
    #[serde(default)]
    pub company_name: Option<String>,

    /// Company URN.
    #[serde(default)]
    pub company_urn: Option<String>,

    /// Structured company info.
    #[serde(default)]
    pub company: Option<Value>,

    /// Role description.
    #[serde(default)]
    pub description: Option<String>,

    /// Location name.
    #[serde(default)]
    pub location_name: Option<String>,

    /// Geo location name.
    #[serde(default)]
    pub geo_location_name: Option<String>,

    /// Geo URN.
    #[serde(default)]
    pub geo_urn: Option<String>,

    /// Time period (start/end dates).
    #[serde(default)]
    pub time_period: Option<Value>,

    /// Whether this is a promotion within the same company.
    #[serde(default)]
    pub promotion: Option<bool>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// Education entry.
///
/// Reference: `re/pegasus_models.md` -- `Education (voyager.identity.profile)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Education {
    /// Entity URN.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// School name.
    #[serde(default)]
    pub school_name: Option<String>,

    /// School URN.
    #[serde(default)]
    pub school_urn: Option<String>,

    /// Structured school info.
    #[serde(default)]
    pub school: Option<Value>,

    /// Degree name.
    #[serde(default)]
    pub degree_name: Option<String>,

    /// Degree URN.
    #[serde(default)]
    pub degree_urn: Option<String>,

    /// Field of study.
    #[serde(default)]
    pub field_of_study: Option<String>,

    /// Field of study URN.
    #[serde(default)]
    pub field_of_study_urn: Option<String>,

    /// Description.
    #[serde(default)]
    pub description: Option<String>,

    /// Grade.
    #[serde(default)]
    pub grade: Option<String>,

    /// Activities.
    #[serde(default)]
    pub activities: Option<String>,

    /// Time period (start/end dates).
    #[serde(default)]
    pub time_period: Option<Value>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// Top-level response from the `messaging/conversations` endpoint.
///
/// Wraps a standard Rest.li collection of `Conversation` items.
/// See `re/api_endpoint_catalog.md` section 6 and `re/pegasus_models.md`
/// for the `Conversation` model definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationsResponse {
    /// Array of conversation items.
    #[serde(default)]
    pub elements: Vec<Value>,

    /// Pagination metadata for this page of results.
    #[serde(default)]
    pub paging: Option<Paging>,

    /// Collection-level metadata (type varies by endpoint).
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// A messaging conversation (thread).
///
/// Reference: `re/pegasus_models.md` -- `Conversation (voyager.messaging)`.
/// Fields kept as `Option` since we haven't validated against live API yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    /// URN identifying this conversation, e.g. `urn:li:messagingThread:...`.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Backend URN.
    #[serde(default)]
    pub backend_urn: Option<String>,

    /// Participants in this conversation (union: member/company/bot).
    // TODO(live-validation): Each element is a MessagingProfile union. Confirm union
    // discriminator format (FQN "com.linkedin.voyager.messaging.MessagingMember" vs
    // short key "messagingMember"). Type as Vec<MessagingProfile> enum after.
    #[serde(default)]
    pub participants: Option<Vec<Value>>,

    /// Messages/events in this conversation (may be inline or empty).
    // TODO(live-validation): Check if events are inlined in conversation list response
    // or if they require a separate fetch to messaging/conversations/{id}/events.
    #[serde(default)]
    pub events: Option<Vec<Value>>,

    /// Whether the conversation has been read.
    #[serde(default)]
    pub read: Option<bool>,

    /// Whether the conversation is muted.
    #[serde(default)]
    pub muted: Option<bool>,

    /// Whether the conversation is archived.
    #[serde(default)]
    pub archived: Option<bool>,

    /// Whether the conversation is blocked.
    #[serde(default)]
    pub blocked: Option<bool>,

    /// Unread message count.
    #[serde(default)]
    pub unread_count: Option<u32>,

    /// Total number of events in the conversation.
    #[serde(default)]
    pub total_event_count: Option<u32>,

    /// Group chat name (if any).
    #[serde(default)]
    pub name: Option<String>,

    /// Whether this is with a non-connection.
    #[serde(default)]
    pub with_non_connection: Option<bool>,

    /// Last activity timestamp.
    // TODO(live-validation): This field is NOT in the decompiled Conversation model.
    // May be computed client-side or from Dash endpoint. Verify if server returns it.
    #[serde(default)]
    pub last_activity_at: Option<u64>,

    /// Read receipts.
    #[serde(default)]
    pub receipts: Option<Vec<Value>>,

    /// Notification status.
    #[serde(default)]
    pub notification_status: Option<String>,

    /// Message request state (ACCEPTED, DECLINED, PENDING).
    #[serde(default)]
    pub message_request_state: Option<String>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// A single messaging event (message, participant change, etc.).
///
/// Reference: `re/pegasus_models.md` -- `Event (voyager.messaging)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagingEvent {
    /// URN identifying this event.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Backend URN.
    #[serde(default)]
    pub backend_urn: Option<String>,

    /// Timestamp when the event was created (epoch millis).
    #[serde(default)]
    pub created_at: Option<u64>,

    /// Timestamp when the event expires (epoch millis).
    #[serde(default)]
    pub expires_at: Option<u64>,

    /// The sender of this event (union: MessagingProfile).
    // TODO(live-validation): Union type -- confirm discriminator key format.
    // Expected FQN: "com.linkedin.voyager.messaging.MessagingMember".
    #[serde(default)]
    pub from: Option<Value>,

    /// Event subtype (MEMBER_TO_MEMBER, INMAIL, etc.).
    #[serde(default)]
    pub subtype: Option<String>,

    /// The event content (union: MessageEvent, ParticipantChangeEvent, etc.).
    // TODO(live-validation): Critical union type. Confirm whether key is FQN
    // "com.linkedin.voyager.messaging.event.MessageEvent" or short "messageEvent".
    // This determines the serde strategy for EventContent enum.
    #[serde(default)]
    pub event_content: Option<Value>,

    /// Quick reply options.
    #[serde(default)]
    pub quick_replies: Option<Vec<Value>>,

    /// URN of the previous event in the conversation.
    #[serde(default)]
    pub previous_event_in_conversation: Option<String>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// Top-level response from the `messaging/conversations/{id}/events` endpoint.
///
/// Wraps a standard Rest.li collection of `MessagingEvent` items.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEventsResponse {
    /// Array of event items.
    #[serde(default)]
    pub elements: Vec<Value>,

    /// Pagination metadata.
    #[serde(default)]
    pub paging: Option<Paging>,

    /// Collection-level metadata.
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// Top-level response from the `relationships/connections` endpoint.
///
/// Wraps a standard Rest.li collection of `Connection` items.
/// See `re/api_endpoint_catalog.md` section 8 and `re/pegasus_models.md`
/// for the `Connection` model definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsResponse {
    /// Array of connection items.
    #[serde(default)]
    pub elements: Vec<Value>,

    /// Pagination metadata for this page of results.
    #[serde(default)]
    pub paging: Option<Paging>,

    /// Collection-level metadata (type varies by endpoint).
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// A connection (1st-degree network member).
///
/// Reference: `re/pegasus_models.md` -- `Connection (voyager.relationships.shared.connection)`.
/// Fields kept as `Option` since we haven't validated against live API yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// URN identifying this connection, e.g. `urn:li:fs_connection:ACoAABxxxxxx`.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Embedded mini profile of the connected member.
    // TODO(live-validation): Type as MiniProfile once confirmed inlined (not URN ref).
    #[serde(default)]
    pub mini_profile: Option<Value>,

    /// Phone numbers shared by this connection.
    #[serde(default)]
    pub phone_numbers: Option<Vec<Value>>,

    /// Primary email address of this connection (if shared).
    #[serde(default)]
    pub primary_email_address: Option<String>,

    /// Twitter handles of this connection.
    #[serde(default)]
    pub twitter_handles: Option<Vec<Value>>,

    /// WeChat contact info.
    #[serde(default)]
    pub we_chat_contact_info: Option<Value>,

    /// Timestamp when the connection was established (epoch millis).
    #[serde(default)]
    pub created_at: Option<u64>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// Top-level response from the `identity/notificationCards` endpoint.
///
/// Wraps a standard Rest.li collection of `NotificationCard` items.
/// See `re/api_endpoint_catalog.md` section 11 and `re/pegasus_models.md`
/// section 3.8 for the `Card` (NotificationCard) model definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Top-level response from the `search/hits` endpoint.
///
/// Wraps a standard Rest.li collection of search hit items.
/// See `re/search_protocol.md` section 7 for the response model structure.
///
/// The response may contain `SearchCluster` elements (when using `search/cluster`)
/// or flat `SearchHit` elements (when using `search/hits`). We keep `elements`
/// as `Vec<Value>` because the hit payloads use Rest.li unions (`hitInfo`)
/// that are polymorphic.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_deserializes_minimal() {
        let json = r#"{"start": 0, "count": 10}"#;
        let paging: Paging = serde_json::from_str(json).unwrap();
        assert_eq!(paging.start, 0);
        assert_eq!(paging.count, 10);
        assert!(paging.total.is_none());
    }

    #[test]
    fn paging_deserializes_full() {
        let json = r#"{"start": 5, "count": 10, "total": 42, "links": []}"#;
        let paging: Paging = serde_json::from_str(json).unwrap();
        assert_eq!(paging.start, 5);
        assert_eq!(paging.count, 10);
        assert_eq!(paging.total, Some(42));
    }

    #[test]
    fn feed_response_deserializes_empty() {
        let json = r#"{"elements": [], "paging": {"start": 0, "count": 10}}"#;
        let resp: FeedResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert_eq!(resp.paging.as_ref().unwrap().start, 0);
    }

    #[test]
    fn feed_response_handles_missing_fields() {
        let json = r#"{}"#;
        let resp: FeedResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert!(resp.paging.is_none());
        assert!(resp.metadata.is_none());
    }

    #[test]
    fn social_activity_counts_deserializes() {
        let json = r#"{"numLikes": 42, "numComments": 5, "liked": true}"#;
        let counts: SocialActivityCounts = serde_json::from_str(json).unwrap();
        assert_eq!(counts.num_likes, Some(42));
        assert_eq!(counts.num_comments, Some(5));
        assert_eq!(counts.liked, Some(true));
        assert!(counts.num_shares.is_none());
    }

    #[test]
    fn conversation_deserializes_minimal() {
        let json = r#"{}"#;
        let conv: Conversation = serde_json::from_str(json).unwrap();
        assert!(conv.entity_urn.is_none());
        assert!(conv.participants.is_none());
        assert!(conv.read.is_none());
    }

    #[test]
    fn conversation_deserializes_with_fields() {
        let json = r#"{
            "entityUrn": "urn:li:messagingThread:2-abc123",
            "read": true,
            "unreadCount": 0,
            "totalEventCount": 15,
            "name": "Test Group",
            "participants": []
        }"#;
        let conv: Conversation = serde_json::from_str(json).unwrap();
        assert_eq!(
            conv.entity_urn.as_deref(),
            Some("urn:li:messagingThread:2-abc123")
        );
        assert_eq!(conv.read, Some(true));
        assert_eq!(conv.unread_count, Some(0));
        assert_eq!(conv.total_event_count, Some(15));
        assert_eq!(conv.name.as_deref(), Some("Test Group"));
    }

    #[test]
    fn messaging_event_deserializes_minimal() {
        let json = r#"{}"#;
        let event: MessagingEvent = serde_json::from_str(json).unwrap();
        assert!(event.entity_urn.is_none());
        assert!(event.subtype.is_none());
        assert!(event.event_content.is_none());
    }

    #[test]
    fn messaging_event_deserializes_with_fields() {
        let json = r#"{
            "entityUrn": "urn:li:fs_event:abc123",
            "createdAt": 1711234567890,
            "subtype": "MEMBER_TO_MEMBER",
            "eventContent": {
                "com.linkedin.voyager.messaging.event.MessageEvent": {
                    "body": "Hello!"
                }
            }
        }"#;
        let event: MessagingEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.entity_urn.as_deref(), Some("urn:li:fs_event:abc123"));
        assert_eq!(event.created_at, Some(1711234567890));
        assert_eq!(event.subtype.as_deref(), Some("MEMBER_TO_MEMBER"));
        assert!(event.event_content.is_some());
    }

    #[test]
    fn conversations_response_deserializes_empty() {
        let json = r#"{"elements": [], "paging": {"start": 0, "count": 10}}"#;
        let resp: ConversationsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert_eq!(resp.paging.as_ref().unwrap().start, 0);
    }

    #[test]
    fn conversation_events_response_handles_missing_fields() {
        let json = r#"{}"#;
        let resp: ConversationEventsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert!(resp.paging.is_none());
    }

    #[test]
    fn mini_profile_deserializes_minimal() {
        let json = r#"{}"#;
        let mp: MiniProfile = serde_json::from_str(json).unwrap();
        assert!(mp.entity_urn.is_none());
        assert!(mp.first_name.is_none());
        assert!(mp.public_identifier.is_none());
    }

    #[test]
    fn mini_profile_deserializes_with_fields() {
        let json = r#"{
            "entityUrn": "urn:li:fs_miniProfile:ACoAABxxxxxx",
            "firstName": "Jane",
            "lastName": "Doe",
            "publicIdentifier": "jane-doe-42",
            "occupation": "Software Engineer at Acme"
        }"#;
        let mp: MiniProfile = serde_json::from_str(json).unwrap();
        assert_eq!(
            mp.entity_urn.as_deref(),
            Some("urn:li:fs_miniProfile:ACoAABxxxxxx")
        );
        assert_eq!(mp.first_name.as_deref(), Some("Jane"));
        assert_eq!(mp.last_name.as_deref(), Some("Doe"));
        assert_eq!(mp.public_identifier.as_deref(), Some("jane-doe-42"));
        assert_eq!(mp.occupation.as_deref(), Some("Software Engineer at Acme"));
    }

    #[test]
    fn profile_deserializes_minimal() {
        let json = r#"{}"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert!(p.entity_urn.is_none());
        assert!(p.headline.is_none());
        assert!(p.summary.is_none());
    }

    #[test]
    fn profile_deserializes_with_fields() {
        let json = r#"{
            "entityUrn": "urn:li:fs_profile:ACoAABxxxxxx",
            "firstName": "Jane",
            "lastName": "Doe",
            "headline": "Senior Engineer",
            "summary": "Building great software.",
            "industryName": "Computer Software",
            "locationName": "San Francisco Bay Area",
            "student": false
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(
            p.entity_urn.as_deref(),
            Some("urn:li:fs_profile:ACoAABxxxxxx")
        );
        assert_eq!(p.headline.as_deref(), Some("Senior Engineer"));
        assert_eq!(p.summary.as_deref(), Some("Building great software."));
        assert_eq!(p.industry_name.as_deref(), Some("Computer Software"));
        assert_eq!(p.student, Some(false));
    }

    #[test]
    fn position_deserializes() {
        let json = r#"{
            "title": "Staff Engineer",
            "companyName": "Acme Corp",
            "locationName": "Remote",
            "timePeriod": {
                "startDate": {"year": 2020, "month": 3},
                "endDate": {"year": 2023, "month": 12}
            }
        }"#;
        let pos: Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.title.as_deref(), Some("Staff Engineer"));
        assert_eq!(pos.company_name.as_deref(), Some("Acme Corp"));
        assert_eq!(pos.location_name.as_deref(), Some("Remote"));
        assert!(pos.time_period.is_some());
    }

    #[test]
    fn education_deserializes() {
        let json = r#"{
            "schoolName": "MIT",
            "degreeName": "BS",
            "fieldOfStudy": "Computer Science",
            "timePeriod": {
                "startDate": {"year": 2010},
                "endDate": {"year": 2014}
            }
        }"#;
        let edu: Education = serde_json::from_str(json).unwrap();
        assert_eq!(edu.school_name.as_deref(), Some("MIT"));
        assert_eq!(edu.degree_name.as_deref(), Some("BS"));
        assert_eq!(edu.field_of_study.as_deref(), Some("Computer Science"));
        assert!(edu.time_period.is_some());
    }

    #[test]
    fn connection_deserializes_minimal() {
        let json = r#"{}"#;
        let conn: Connection = serde_json::from_str(json).unwrap();
        assert!(conn.entity_urn.is_none());
        assert!(conn.mini_profile.is_none());
        assert!(conn.created_at.is_none());
    }

    #[test]
    fn connection_deserializes_with_fields() {
        let json = r#"{
            "entityUrn": "urn:li:fs_connection:ACoAABxxxxxx",
            "miniProfile": {
                "firstName": "Alice",
                "lastName": "Smith",
                "occupation": "Engineer at Acme"
            },
            "createdAt": 1711234567890,
            "primaryEmailAddress": "alice@example.com"
        }"#;
        let conn: Connection = serde_json::from_str(json).unwrap();
        assert_eq!(
            conn.entity_urn.as_deref(),
            Some("urn:li:fs_connection:ACoAABxxxxxx")
        );
        assert!(conn.mini_profile.is_some());
        assert_eq!(conn.created_at, Some(1711234567890));
        assert_eq!(
            conn.primary_email_address.as_deref(),
            Some("alice@example.com")
        );
    }

    #[test]
    fn connections_response_deserializes_empty() {
        let json = r#"{"elements": [], "paging": {"start": 0, "count": 10}}"#;
        let resp: ConnectionsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert_eq!(resp.paging.as_ref().unwrap().start, 0);
    }

    #[test]
    fn connections_response_handles_missing_fields() {
        let json = r#"{}"#;
        let resp: ConnectionsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert!(resp.paging.is_none());
    }

    #[test]
    fn notification_card_deserializes_minimal() {
        let json = r#"{}"#;
        let card: NotificationCard = serde_json::from_str(json).unwrap();
        assert!(card.entity_urn.is_none());
        assert!(card.headline.is_none());
        assert!(card.published_at.is_none());
        assert!(card.read.is_none());
    }

    #[test]
    fn notification_card_deserializes_with_fields() {
        let json = r#"{
            "entityUrn": "urn:li:fs_notificationCard:abc123",
            "headline": {"text": "Someone viewed your profile"},
            "subHeadline": {"text": "John Doe and 2 others"},
            "kicker": {"text": "2h ago"},
            "contentType": "PROFILE_VIEW",
            "publishedAt": 1711234567890,
            "read": false
        }"#;
        let card: NotificationCard = serde_json::from_str(json).unwrap();
        assert_eq!(
            card.entity_urn.as_deref(),
            Some("urn:li:fs_notificationCard:abc123")
        );
        assert!(card.headline.is_some());
        assert!(card.sub_headline.is_some());
        assert!(card.kicker.is_some());
        assert_eq!(card.content_type.as_deref(), Some("PROFILE_VIEW"));
        assert_eq!(card.published_at, Some(1711234567890));
        assert_eq!(card.read, Some(false));
    }

    #[test]
    fn notification_cards_response_deserializes_empty() {
        let json = r#"{"elements": [], "paging": {"start": 0, "count": 10}}"#;
        let resp: NotificationCardsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert_eq!(resp.paging.as_ref().unwrap().start, 0);
    }

    #[test]
    fn notification_cards_response_handles_missing_fields() {
        let json = r#"{}"#;
        let resp: NotificationCardsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.elements.is_empty());
        assert!(resp.paging.is_none());
    }
}
