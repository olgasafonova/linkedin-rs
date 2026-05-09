//! Messaging API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;

use super::internal::{graphql_params, restli_encode_string, unwrap_graphql};
use super::LinkedInClient;

impl LinkedInClient {
    /// Fetch the user's messaging conversations.
    pub async fn get_conversations(
        &self,
        count: u32,
        created_before: Option<u64>,
    ) -> Result<Value, Error> {
        let mailbox_urn = self.my_profile_urn().await?;
        let encoded_urn = restli_encode_string(mailbox_urn);
        let vars = match created_before {
            Some(ts) => format!(
                "(mailboxUrn:{},category:PRIMARY_INBOX,count:{},lastActivityBefore:{})",
                encoded_urn, count, ts
            ),
            None => format!(
                "(mailboxUrn:{},category:PRIMARY_INBOX,count:{})",
                encoded_urn, count
            ),
        };
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
        recipient_profile_urn: &str,
        message_body: &str,
    ) -> Result<Value, Error> {
        let my_urn = self.my_profile_urn().await?;
        let recipients = vec![recipient_profile_urn.to_string()];
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
        conversation_id: &str,
        message_body: &str,
    ) -> Result<Value, Error> {
        let conv_data = self.get_conversations(20, None).await?;
        let my_urn = self.my_profile_urn().await?;
        let thread_id = strip_messaging_thread_prefix(conversation_id);

        let elements = conv_data
            .get("elements")
            .and_then(|e| e.as_array())
            .ok_or_else(|| Error::Api {
                status: 0,
                body: "no conversations found".to_string(),
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
    pub async fn get_conversation_events(
        &self,
        conversation_urn: &str,
        created_before: Option<u64>,
    ) -> Result<Value, Error> {
        let full_urn = if conversation_urn.starts_with("urn:li:msg_conversation:") {
            conversation_urn.to_string()
        } else {
            let thread_id = strip_messaging_thread_prefix(conversation_urn);
            let profile_urn = self.my_profile_urn().await?;
            format!("urn:li:msg_conversation:({},{})", profile_urn, thread_id)
        };
        let encoded_urn = restli_encode_string(&full_urn);

        let variables = match created_before {
            Some(ts) => format!("(conversationUrn:{},deliveredBefore:{})", encoded_urn, ts),
            None => format!("(conversationUrn:{})", encoded_urn),
        };
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
