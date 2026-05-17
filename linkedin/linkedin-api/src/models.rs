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
//!
//! Per-resource models live in submodules (`feed`, `profile`, `messaging`,
//! `connections`, `notifications`, `search`). The submodule contents are
//! re-exported flat so existing `use crate::models::{...}` call sites keep
//! working.

mod connections;
mod feed;
mod messaging;
mod notifications;
mod profile;
mod search;

pub use connections::{Connection, ConnectionsResponse};
pub use feed::{FeedResponse, Paging, SocialActivityCounts, SocialDetail, UpdateV2};
pub use messaging::{
    Conversation, ConversationEventsResponse, ConversationsResponse, MessagingEvent,
};
pub use notifications::{NotificationCard, NotificationCardsResponse};
pub use profile::{Education, MiniProfile, Position, Profile};
pub use search::{SearchHit, SearchResponse};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::HashMap;

    // `#[serde(flatten)] extra: Option<HashMap<...>>` always deserializes to
    // `Some(HashMap::new())` when no extra keys are present, never `None`.
    // Expected struct literals below use this helper to match that shape.
    fn empty_extra() -> Option<HashMap<String, Value>> {
        Some(HashMap::new())
    }

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
        assert_eq!(
            counts,
            SocialActivityCounts {
                num_likes: Some(42),
                num_comments: Some(5),
                liked: Some(true),
                ..Default::default()
            },
        );
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
            conv,
            Conversation {
                entity_urn: Some("urn:li:messagingThread:2-abc123".to_string()),
                read: Some(true),
                unread_count: Some(0),
                total_event_count: Some(15),
                name: Some("Test Group".to_string()),
                participants: Some(vec![]),
                extra: empty_extra(),
                ..Default::default()
            },
        );
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
        assert_eq!(
            event,
            MessagingEvent {
                entity_urn: Some("urn:li:fs_event:abc123".to_string()),
                created_at: Some(1711234567890),
                subtype: Some("MEMBER_TO_MEMBER".to_string()),
                event_content: Some(serde_json::json!({
                    "com.linkedin.voyager.messaging.event.MessageEvent": {"body": "Hello!"}
                })),
                extra: empty_extra(),
                ..Default::default()
            },
        );
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
            mp,
            MiniProfile {
                entity_urn: Some("urn:li:fs_miniProfile:ACoAABxxxxxx".to_string()),
                first_name: Some("Jane".to_string()),
                last_name: Some("Doe".to_string()),
                public_identifier: Some("jane-doe-42".to_string()),
                occupation: Some("Software Engineer at Acme".to_string()),
                ..Default::default()
            },
        );
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
            p,
            Profile {
                entity_urn: Some("urn:li:fs_profile:ACoAABxxxxxx".to_string()),
                first_name: Some("Jane".to_string()),
                last_name: Some("Doe".to_string()),
                headline: Some("Senior Engineer".to_string()),
                summary: Some("Building great software.".to_string()),
                industry_name: Some("Computer Software".to_string()),
                location_name: Some("San Francisco Bay Area".to_string()),
                student: Some(false),
                extra: empty_extra(),
                ..Default::default()
            },
        );
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
        assert_eq!(
            pos,
            Position {
                title: Some("Staff Engineer".to_string()),
                company_name: Some("Acme Corp".to_string()),
                location_name: Some("Remote".to_string()),
                time_period: Some(serde_json::json!({
                    "startDate": {"year": 2020, "month": 3},
                    "endDate": {"year": 2023, "month": 12}
                })),
                extra: empty_extra(),
                ..Default::default()
            },
        );
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
        assert_eq!(
            edu,
            Education {
                school_name: Some("MIT".to_string()),
                degree_name: Some("BS".to_string()),
                field_of_study: Some("Computer Science".to_string()),
                time_period: Some(serde_json::json!({
                    "startDate": {"year": 2010},
                    "endDate": {"year": 2014}
                })),
                extra: empty_extra(),
                ..Default::default()
            },
        );
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
            conn,
            Connection {
                entity_urn: Some("urn:li:fs_connection:ACoAABxxxxxx".to_string()),
                mini_profile: Some(serde_json::json!({
                    "firstName": "Alice",
                    "lastName": "Smith",
                    "occupation": "Engineer at Acme"
                })),
                created_at: Some(1711234567890),
                primary_email_address: Some("alice@example.com".to_string()),
                extra: empty_extra(),
                ..Default::default()
            },
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
        let card: NotificationCard = serde_json::from_str("{}").unwrap();
        assert_eq!(
            card,
            NotificationCard {
                extra: empty_extra(),
                ..Default::default()
            },
        );
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
            card,
            NotificationCard {
                entity_urn: Some("urn:li:fs_notificationCard:abc123".to_string()),
                headline: Some(serde_json::json!({"text": "Someone viewed your profile"})),
                sub_headline: Some(serde_json::json!({"text": "John Doe and 2 others"})),
                kicker: Some(serde_json::json!({"text": "2h ago"})),
                content_type: Some("PROFILE_VIEW".to_string()),
                published_at: Some(1711234567890),
                read: Some(false),
                extra: empty_extra(),
                ..Default::default()
            },
        );
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
