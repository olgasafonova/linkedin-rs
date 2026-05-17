//! Messaging-shaped response models (conversations, events).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Paging;

/// Top-level response from the `messaging/conversations` endpoint.
///
/// Wraps a standard Rest.li collection of `Conversation` items.
/// See `re/api_endpoint_catalog.md` section 6 and `re/pegasus_models.md`
/// for the `Conversation` model definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
