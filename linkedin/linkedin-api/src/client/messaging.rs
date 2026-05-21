//! Messaging API methods on `LinkedInClient`.

use std::str::FromStr;

use serde_json::Value;

use crate::error::Error;
use crate::urn::{ConversationUrn, ProfileUrn};

use super::internal::{graphql_params, restli_encode_string, unwrap_graphql};
use super::LinkedInClient;

// ---------------------------------------------------------------------------
// ConversationCategory
// ---------------------------------------------------------------------------

/// Messaging conversation category accepted by LinkedIn's
/// `messengerConversationsByCategory` GraphQL query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationCategory {
    PrimaryInbox,
    Spam,
}

impl ConversationCategory {
    /// Wire-format string sent to the GraphQL endpoint.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrimaryInbox => "PRIMARY_INBOX",
            Self::Spam => "SPAM",
        }
    }
}

impl Default for ConversationCategory {
    fn default() -> Self {
        Self::PrimaryInbox
    }
}

impl FromStr for ConversationCategory {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().replace('-', "_").as_str() {
            "PRIMARY" | "PRIMARY_INBOX" | "INBOX" => Ok(Self::PrimaryInbox),
            "SPAM" => Ok(Self::Spam),
            other => Err(format!(
                "unknown conversation category '{other}' (expected primary or spam)"
            )),
        }
    }
}

impl LinkedInClient {
    /// Fetch the user's messaging conversations.
    ///
    /// This is a convenience wrapper that defaults to `PrimaryInbox` and
    /// ignores the `created_before` parameter (LinkedIn deprecated the
    /// timestamp-based cursor in favor of opaque `nextCursor` tokens).
    /// Prefer [`get_conversations_by_category`] or
    /// [`get_conversations_with_cursor`] for new code.
    pub async fn get_conversations(
        &self,
        count: u32,
        created_before: Option<u64>,
    ) -> Result<Value, Error> {
        let _ = created_before;
        self.get_conversations_by_category(count, ConversationCategory::PrimaryInbox)
            .await
    }

    /// Fetch messaging conversations using the opaque `nextCursor` token
    /// from a previous response's `metadata.nextCursor` field.
    pub async fn get_conversations_with_cursor(
        &self,
        count: u32,
        next_cursor: Option<&str>,
    ) -> Result<Value, Error> {
        self.get_conversations_by_category_with_cursor(
            count,
            next_cursor,
            ConversationCategory::PrimaryInbox,
        )
        .await
    }

    /// Fetch messaging conversations from a specific inbox category.
    pub async fn get_conversations_by_category(
        &self,
        count: u32,
        category: ConversationCategory,
    ) -> Result<Value, Error> {
        self.get_conversations_by_category_with_cursor(count, None, category)
            .await
    }

    /// Fetch messaging conversations from a specific inbox category using the
    /// opaque `nextCursor` token returned by LinkedIn.
    ///
    /// LinkedIn's GraphQL inbox endpoint (`messengerConversationsByCategory`)
    /// accepts `nextCursor` from the response metadata for pagination. The
    /// legacy `lastActivityBefore` timestamp cursor is no longer functional.
    pub async fn get_conversations_by_category_with_cursor(
        &self,
        count: u32,
        next_cursor: Option<&str>,
        category: ConversationCategory,
    ) -> Result<Value, Error> {
        let mailbox_urn = self.my_profile_urn().await?;
        let vars = build_conversations_graphql_vars(mailbox_urn, category, count, next_cursor);
        let params = graphql_params(
            &vars,
            "voyagerMessagingDashMessengerConversations.7dc50d3efc3953190125aca9c05f0af6",
            "MessengerConversationsByCategory",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "messengerConversationsByCategory")
    }

    /// Send a message to a LinkedIn member via the Dash createMessage endpoint.
    pub async fn send_message(
        &self,
        recipient_profile_urn: &ProfileUrn,
        message_body: &str,
    ) -> Result<Value, Error> {
        let my_urn = self.my_profile_urn().await?;
        let recipients = vec![recipient_profile_urn.as_str().to_string()];
        let payload = build_create_message_payload(my_urn, &recipients, message_body);

        self.post(
            "voyagerMessagingDashMessengerMessages?action=createMessage",
            &payload,
        )
        .await
    }

    /// Reply to an existing messaging conversation by sending to the same
    /// participants. LinkedIn routes to the existing thread when the
    /// recipient set matches.
    pub async fn reply_to_conversation(
        &self,
        conversation_id: &ConversationUrn,
        message_body: &str,
    ) -> Result<Value, Error> {
        let conv_data = self.get_conversations(20, None).await?;
        let my_urn = self.my_profile_urn().await?;
        let thread_id = strip_messaging_thread_prefix(conversation_id.as_str());

        let elements = conv_data
            .get("elements")
            .and_then(|e| e.as_array())
            .ok_or_else(|| Error::Api {
                status: 0,
                body: "no conversations found".to_string(),
                correlation_id: None,
            })?;

        let recipient_urns = elements
            .iter()
            .find(|conv| {
                conv.get("backendUrn")
                    .and_then(|u| u.as_str())
                    .map(strip_messaging_thread_prefix)
                    == Some(thread_id)
            })
            .map(|conv| extract_recipient_urns(conv, my_urn))
            .unwrap_or_default();

        if recipient_urns.is_empty() {
            return Err(Error::Api {
                status: 0,
                body: format!(
                    "could not find conversation '{}' or extract participant URNs",
                    conversation_id
                ),
                correlation_id: None,
            });
        }

        let payload = build_create_message_payload(my_urn, &recipient_urns, message_body);
        self.post(
            "voyagerMessagingDashMessengerMessages?action=createMessage",
            &payload,
        )
        .await
    }

    /// Fetch events (messages) within a specific conversation.
    ///
    /// Convenience wrapper that ignores `created_before`. Prefer
    /// [`get_conversation_events_with_cursor`] for new code.
    pub async fn get_conversation_events(
        &self,
        conversation_urn: &ConversationUrn,
        created_before: Option<u64>,
    ) -> Result<Value, Error> {
        let _ = created_before;
        self.get_conversation_events_with_cursor(conversation_urn, None)
            .await
    }

    /// Fetch events (messages) within a specific conversation using the opaque
    /// `nextCursor` token from a previous response.
    pub async fn get_conversation_events_with_cursor(
        &self,
        conversation_urn: &ConversationUrn,
        next_cursor: Option<&str>,
    ) -> Result<Value, Error> {
        let raw = conversation_urn.as_str();
        let full_urn = if raw.starts_with("urn:li:msg_conversation:") {
            raw.to_string()
        } else {
            let thread_id = strip_messaging_thread_prefix(raw);
            let profile_urn = self.my_profile_urn().await?;
            format!("urn:li:msg_conversation:({},{})", profile_urn, thread_id)
        };
        let encoded_urn = restli_encode_string(&full_urn);
        let variables = build_conversation_events_graphql_vars(&encoded_urn, next_cursor);
        let params = graphql_params(
            &variables,
            "voyagerMessagingDashMessengerMessages.7cde5843a127bbecc3de900d3894a74a",
            "MessengerMessagesByConversation",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "messengerMessagesByConversation")
    }
}

/// Strip the `urn:li:messagingThread:` prefix if present, returning the bare
/// thread id (e.g. `2-abc123`). Idempotent: bare ids pass through unchanged.
fn strip_messaging_thread_prefix(urn: &str) -> &str {
    urn.strip_prefix("urn:li:messagingThread:").unwrap_or(urn)
}

/// Build the JSON payload for the
/// `voyagerMessagingDashMessengerMessages?action=createMessage` endpoint.
///
/// `recipient_urns` carries one URN for a new message and one or more URNs
/// for a thread reply; LinkedIn routes to the existing thread when the set
/// matches.
///
/// Captured from live browser traffic via Chrome DevTools MCP. Key
/// discovery: `trackingId` must be 16 random bytes mapped via `byte as
/// char`, NOT base64-encoded — unlike `send_connection_request`. Without
/// this field, or with a UUID/string value, the server returns
/// `{"status": 400}` with no further detail. See `re/send_message.md`.
fn build_create_message_payload(
    my_urn: &str,
    recipient_urns: &[String],
    message_body: &str,
) -> Value {
    let origin_token = uuid::Uuid::new_v4().to_string();
    let tracking_bytes: [u8; 16] = rand::random();
    let tracking_id: String = tracking_bytes.iter().map(|&b| b as char).collect();

    serde_json::json!({
        "message": {
            "body": {
                "attributes": [],
                "text": message_body
            },
            "originToken": origin_token,
            "renderContentUnions": []
        },
        "mailboxUrn": my_urn,
        "trackingId": tracking_id,
        "dedupeByClientGeneratedToken": false,
        "hostRecipientUrns": recipient_urns
    })
}

/// Extract recipient URNs from a conversation, excluding `my_urn`.
fn extract_recipient_urns(conv: &Value, my_urn: &str) -> Vec<String> {
    let Some(participants) = conv
        .get("conversationParticipants")
        .and_then(|p| p.as_array())
    else {
        return Vec::new();
    };
    participants
        .iter()
        .filter_map(participant_urn)
        .filter(|u| *u != my_urn)
        .map(str::to_string)
        .collect()
}

fn participant_urn(p: &Value) -> Option<&str> {
    if let Some(u) = p.get("hostIdentityUrn").and_then(|u| u.as_str()) {
        return Some(u);
    }
    let member = p.get("participantType").and_then(|pt| pt.get("member"))?;
    member
        .get("entityUrn")
        .or_else(|| member.get("hostIdentityUrn"))
        .and_then(|u| u.as_str())
}

// ---------------------------------------------------------------------------
// GraphQL variable builders
// ---------------------------------------------------------------------------

/// Build the parenthesized-record GraphQL variables string for the
/// `messengerConversationsByCategory` query.
fn build_conversations_graphql_vars(
    mailbox_urn: &str,
    category: ConversationCategory,
    count: u32,
    next_cursor: Option<&str>,
) -> String {
    let encoded_urn = restli_encode_string(mailbox_urn);
    if let Some(cursor) = next_cursor.filter(|c| !c.is_empty()) {
        format!(
            "(mailboxUrn:{},category:{},count:{},nextCursor:{})",
            encoded_urn,
            category.as_str(),
            count,
            restli_encode_string(cursor)
        )
    } else {
        format!(
            "(mailboxUrn:{},category:{},count:{})",
            encoded_urn,
            category.as_str(),
            count
        )
    }
}

/// Build the parenthesized-record GraphQL variables string for the
/// `messengerMessagesByConversation` query.
fn build_conversation_events_graphql_vars(
    encoded_conversation_urn: &str,
    next_cursor: Option<&str>,
) -> String {
    if let Some(cursor) = next_cursor.filter(|c| !c.is_empty()) {
        format!(
            "(conversationUrn:{},nextCursor:{})",
            encoded_conversation_urn,
            restli_encode_string(cursor)
        )
    } else {
        format!("(conversationUrn:{})", encoded_conversation_urn)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_category_wire_values_match_live_linkedin_graphql() {
        assert_eq!(ConversationCategory::PrimaryInbox.as_str(), "PRIMARY_INBOX");
        assert_eq!(ConversationCategory::Spam.as_str(), "SPAM");
    }

    #[test]
    fn conversation_category_parses_verified_cli_aliases() {
        assert_eq!(
            "primary".parse::<ConversationCategory>().unwrap(),
            ConversationCategory::PrimaryInbox
        );
        assert_eq!(
            "primary_inbox".parse::<ConversationCategory>().unwrap(),
            ConversationCategory::PrimaryInbox
        );
        assert_eq!(
            "spam".parse::<ConversationCategory>().unwrap(),
            ConversationCategory::Spam
        );
        assert!("other".parse::<ConversationCategory>().is_err());
        assert!("archived".parse::<ConversationCategory>().is_err());
        assert!("sent".parse::<ConversationCategory>().is_err());
    }

    #[test]
    fn conversation_variables_include_requested_category_without_cursor() {
        let vars = build_conversations_graphql_vars(
            "urn:li:fsd_profile:abc123",
            ConversationCategory::Spam,
            25,
            None,
        );
        assert_eq!(
            vars,
            "(mailboxUrn:urn%3Ali%3Afsd_profile%3Aabc123,category:SPAM,count:25)"
        );
    }

    #[test]
    fn conversation_variables_include_linkedin_next_cursor() {
        let vars = build_conversations_graphql_vars(
            "urn:li:fsd_profile:abc123",
            ConversationCategory::PrimaryInbox,
            5,
            Some("DESCENDING&1776357191118&2-MTll...=="),
        );
        assert!(vars.contains("mailboxUrn:urn%3Ali%3Afsd_profile%3Aabc123"));
        assert!(vars.contains("category:PRIMARY_INBOX"));
        assert!(vars.contains("count:5"));
        assert!(vars.contains("nextCursor:DESCENDING%261776357191118%262-MTll...%3D%3D"));
        assert!(!vars.contains("lastActivityBefore"));
    }

    #[test]
    fn message_variables_include_linkedin_next_cursor() {
        let vars = build_conversation_events_graphql_vars(
            "urn%3Ali%3Amsg_conversation%3A%28urn%3Ali%3Afsd_profile%3Aabc%2C2-thread%29",
            Some("ASCENDING&1774929897240&2-MTc3...=="),
        );
        assert!(vars.starts_with("(conversationUrn:urn%3Ali%3Amsg_conversation"));
        assert!(vars.contains("nextCursor:ASCENDING%261774929897240%262-MTc3...%3D%3D"));
    }

    #[test]
    fn conversation_events_without_cursor_omits_next_cursor_field() {
        let vars = build_conversation_events_graphql_vars(
            "urn%3Ali%3Amsg_conversation%3A%28urn%3Ali%3Afsd_profile%3Aabc%2C2-thread%29",
            None,
        );
        assert!(!vars.contains("nextCursor"));
        assert!(vars.starts_with("(conversationUrn:"));
    }
}
