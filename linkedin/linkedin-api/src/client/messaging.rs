//! Messaging API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;
use crate::urn::{ConversationUrn, ProfileUrn};

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
    /// recipient set matches — but only for regular member-to-member
    /// threads. InMail/recruiter threads are a different thread type and
    /// the recipient-set heuristic silently creates a NEW conversation
    /// (observed live 14-08-2026), so those are rejected up front. As a
    /// second line of defence, the response's conversation URN is checked
    /// against the requested thread and a mismatch is reported as an
    /// error rather than a false success.
    pub async fn reply_to_conversation(
        &self,
        conversation_id: &ConversationUrn,
        message_body: &str,
    ) -> Result<Value, Error> {
        let conv_data = self.get_conversations(20, None).await?;
        let my_urn = self.my_profile_urn().await?;
        let thread_id = ThreadId::parse(conversation_id.as_str());

        let elements = conv_data
            .get("elements")
            .and_then(|e| e.as_array())
            .ok_or_else(|| synthesized_error("no conversations found".to_string()))?;

        let conversation = elements.iter().find(|conv| {
            conv.get("backendUrn")
                .and_then(|u| u.as_str())
                .map(ThreadId::parse)
                == Some(thread_id)
        });

        if conversation.is_some_and(is_inmail_conversation) {
            return Err(synthesized_error(format!(
                "conversation '{}' is an InMail thread; replying via createMessage \
                 would create a new conversation instead of a reply, so it is not \
                 supported. Reply from the LinkedIn app instead.",
                conversation_id
            )));
        }

        let recipient_urns = conversation
            .map(|conv| extract_recipient_urns(conv, my_urn))
            .unwrap_or_default();

        if recipient_urns.is_empty() {
            return Err(synthesized_error(format!(
                "could not find conversation '{}' or extract participant URNs",
                conversation_id
            )));
        }

        let payload = build_create_message_payload(my_urn, &recipient_urns, message_body);
        let response = self
            .post(
                "voyagerMessagingDashMessengerMessages?action=createMessage",
                &payload,
            )
            .await?;

        verify_reply_landed_in_thread(&response, thread_id)?;
        Ok(response)
    }

    /// Fetch events (messages) within a specific conversation.
    pub async fn get_conversation_events(
        &self,
        conversation_urn: &ConversationUrn,
        created_before: Option<u64>,
    ) -> Result<Value, Error> {
        let raw = conversation_urn.as_str();
        let full_urn = if raw.starts_with("urn:li:msg_conversation:") {
            raw.to_string()
        } else {
            let thread_id = ThreadId::parse(raw);
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

/// A bare messaging thread id (e.g. `2-abc123`), typed so it can't be
/// confused with the `urn:li:messagingThread:`-prefixed URN form of the
/// same identifier — the shape mix-up behind the InMail misroute bug.
#[derive(Clone, Copy, PartialEq)]
struct ThreadId<'a>(&'a str);

impl<'a> ThreadId<'a> {
    /// Parse from either a bare id or a full `messagingThread` URN.
    /// Idempotent: bare ids pass through unchanged.
    fn parse(urn: &'a str) -> Self {
        Self(urn.strip_prefix("urn:li:messagingThread:").unwrap_or(urn))
    }
}

impl std::fmt::Display for ThreadId<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Build a synthesized (non-HTTP) API error with status 0.
fn synthesized_error(body: String) -> Error {
    Error::Api {
        status: 0,
        body,
        correlation_id: None,
    }
}

/// True when a Dash conversation element is an InMail/recruiter thread.
///
/// Primary signal: the conversation's `categories` array contains
/// `INMAIL` (Dash category enum; see `re/pegasus_models.md` messaging
/// enums). Fallback: a `conversationTypeText` label mentioning InMail,
/// which LinkedIn attaches to sponsored/recruiter threads.
fn is_inmail_conversation(conv: &Value) -> bool {
    let has_inmail_category = conv
        .get("categories")
        .and_then(|c| c.as_array())
        .is_some_and(|cats| cats.iter().any(|c| c.as_str() == Some("INMAIL")));
    if has_inmail_category {
        return true;
    }
    conv.get("conversationTypeText")
        .and_then(|t| {
            t.get("text")
                .and_then(|v| v.as_str())
                .or_else(|| t.as_str())
        })
        .is_some_and(|label| label.to_lowercase().contains("inmail"))
}

/// Check that the `createMessage` response's conversation URN matches the
/// thread we meant to reply to. LinkedIn routes replies by recipient set,
/// and when that heuristic fails the message lands in a brand-new thread
/// while the HTTP call still returns 200 — a success receipt that lies.
/// A missing URN in the response is not treated as a failure; only a
/// present-and-different one is.
fn verify_reply_landed_in_thread(response: &Value, requested: ThreadId<'_>) -> Result<(), Error> {
    let landed = response
        .pointer("/value/backendConversationUrn")
        .and_then(|u| u.as_str())
        .map(ThreadId::parse);
    match landed {
        Some(actual) if actual != requested => Err(synthesized_error(format!(
            "message was SENT but landed in a different conversation \
             ('{}' instead of '{}'): LinkedIn created a new thread rather \
             than replying. This happens on InMail threads.",
            actual, requested
        ))),
        _ => Ok(()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inmail_detected_via_categories() {
        let conv = json!({"categories": ["INBOX", "INMAIL"]});
        assert!(is_inmail_conversation(&conv));
    }

    #[test]
    fn inmail_detected_via_type_text() {
        let obj_label = json!({"conversationTypeText": {"text": "InMail"}});
        let bare_label = json!({"conversationTypeText": "InMail"});
        assert!(is_inmail_conversation(&obj_label));
        assert!(is_inmail_conversation(&bare_label));
    }

    #[test]
    fn regular_conversation_is_not_inmail() {
        let conv = json!({
            "categories": ["INBOX", "PRIMARY_INBOX"],
            "conversationTypeText": {"text": "Sponsored"}
        });
        assert!(!is_inmail_conversation(&conv));
        assert!(!is_inmail_conversation(&json!({})));
    }

    #[test]
    fn reply_verification_passes_on_matching_thread() {
        let resp = json!({"value": {
            "backendConversationUrn": "urn:li:messagingThread:2-abc123"
        }});
        assert!(verify_reply_landed_in_thread(&resp, ThreadId("2-abc123")).is_ok());
    }

    #[test]
    fn reply_verification_tolerates_missing_urn() {
        assert!(verify_reply_landed_in_thread(&json!({}), ThreadId("2-abc123")).is_ok());
    }

    #[test]
    fn reply_verification_fails_on_new_thread() {
        let resp = json!({"value": {
            "backendConversationUrn": "urn:li:messagingThread:2-NEW"
        }});
        let err = verify_reply_landed_in_thread(&resp, ThreadId("2-abc123")).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("SENT"),
            "must state the message went out: {msg}"
        );
        assert!(msg.contains("new thread"), "must name the misroute: {msg}");
    }

    #[test]
    fn recipient_urns_exclude_self() {
        let conv = json!({"conversationParticipants": [
            {"hostIdentityUrn": "urn:li:fsd_profile:ME"},
            {"hostIdentityUrn": "urn:li:fsd_profile:OTHER"},
            {"participantType": {"member": {"entityUrn": "urn:li:fsd_profile:THIRD"}}}
        ]});
        let urns = extract_recipient_urns(&conv, "urn:li:fsd_profile:ME");
        assert_eq!(
            urns,
            vec!["urn:li:fsd_profile:OTHER", "urn:li:fsd_profile:THIRD"]
        );
    }
}
