use crate::util::truncate_with_ellipsis;

/// Print a conversation from the GraphQL `messengerConversationsByCategory` response.
pub fn print_graphql_conversation(index: usize, conv: &serde_json::Value) {
    let conv_id = conv_thread_id(conv);
    let unread_marker = if conv_is_read(conv) { " " } else { "*" };
    let unread_count = conv
        .get("unreadCount")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let display_name = conv_display_name(conv);

    println!(
        "[{}]{} {} (id: {})",
        index, unread_marker, display_name, conv_id
    );
    if unread_count > 0 {
        println!("    unread: {}", unread_count);
    }
    let last = conv_last_message_text(conv);
    if !last.is_empty() {
        println!("    last: {}", truncate_with_ellipsis(&last, 80));
    }
}

fn conv_thread_id(conv: &serde_json::Value) -> &str {
    let backend_urn = conv
        .get("backendUrn")
        .and_then(|u| u.as_str())
        .unwrap_or("");
    backend_urn
        .strip_prefix("urn:li:messagingThread:")
        .unwrap_or(backend_urn)
}

fn conv_is_read(conv: &serde_json::Value) -> bool {
    conv.get("read").and_then(|r| r.as_bool()).unwrap_or(true)
}

fn conv_display_name(conv: &serde_json::Value) -> String {
    if let Some(title) = conv.get("title").and_then(|n| n.as_str()) {
        if !title.is_empty() {
            return title.to_string();
        }
    }
    let names = conv_participant_names(conv);
    if names.is_empty() {
        return "(unknown)".to_string();
    }
    names.join(", ")
}

fn conv_participant_names(conv: &serde_json::Value) -> Vec<String> {
    conv.get("conversationParticipants")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .filter_map(participant_full_name)
        .collect()
}

fn participant_full_name(p: &serde_json::Value) -> Option<String> {
    let member = p.get("participantType").and_then(|pt| pt.get("member"))?;
    let first = member_text_field(member, "firstName");
    let last = member_text_field(member, "lastName");
    if first.is_empty() && last.is_empty() {
        return None;
    }
    Some(format!("{} {}", first, last).trim().to_string())
}

/// Pull a string out of a member's `firstName`/`lastName` AttributedText
/// field. The field can be either `{text: "..."}` or a bare string.
fn member_text_field<'a>(member: &'a serde_json::Value, field: &str) -> &'a str {
    member
        .get(field)
        .and_then(|f| {
            f.get("text")
                .and_then(|v| v.as_str())
                .or_else(|| f.as_str())
        })
        .unwrap_or("")
}

fn conv_last_message_text(conv: &serde_json::Value) -> String {
    conv.get("messages")
        .and_then(|m| m.get("elements"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(message_body_text)
        .unwrap_or_default()
}

/// Print a message from the GraphQL `messengerMessagesByConversation` response.
///
/// Supports threading: if a message is a reply, shows the parent message
/// context indented above the reply.
pub fn print_graphql_message(msg: &serde_json::Value) {
    print_reply_context(msg);

    let time_str = format_delivered_at(msg);
    if !time_str.is_empty() {
        print!("[{}] ", time_str);
    }
    println!("{}", sender_display_name(msg));

    if let Some(subject) = msg.get("subject").and_then(|s| s.as_str()) {
        if !subject.is_empty() {
            println!("  Subject: {}", subject);
        }
    }
    let body = message_body_text(msg).unwrap_or_default();
    if !body.is_empty() {
        println!("  {}", body);
    }
    for line in render_content_lines(msg) {
        println!("{}", line);
    }
}

fn print_reply_context(msg: &serde_json::Value) {
    let Some(parent) = msg
        .get("replyMessage")
        .or_else(|| msg.get("parentMessage"))
        .or_else(|| msg.get("quotedMessage"))
    else {
        return;
    };
    let parent_body = message_body_text(parent).unwrap_or_default();
    if parent_body.is_empty() {
        return;
    }
    let parent_sender = parent
        .get("sender")
        .and_then(|s| s.get("hostIdentityUrn"))
        .and_then(|u| u.as_str())
        .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
        .unwrap_or("unknown");
    println!(
        "  > {} said: {}",
        parent_sender,
        truncate_with_ellipsis(&parent_body, 80)
    );
}

fn format_delivered_at(msg: &serde_json::Value) -> String {
    let delivered_at = msg.get("deliveredAt").and_then(|c| c.as_u64()).unwrap_or(0);
    if delivered_at == 0 {
        return String::new();
    }
    let secs = (delivered_at / 1000) as i64;
    let nanos = ((delivered_at % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, nanos)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| delivered_at.to_string())
}

fn sender_display_name(msg: &serde_json::Value) -> String {
    let sender = msg.get("sender");
    if let Some(name) = sender.and_then(sender_member_name) {
        return name;
    }
    sender
        .and_then(|s| s.get("hostIdentityUrn"))
        .and_then(|u| u.as_str())
        .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
        .unwrap_or("unknown")
        .to_string()
}

fn sender_member_name(sender: &serde_json::Value) -> Option<String> {
    let member = sender
        .get("participantType")
        .and_then(|pt| pt.get("member"))?;
    let first = member_text_field(member, "firstName");
    let last = member_text_field(member, "lastName");
    if first.is_empty() && last.is_empty() {
        return None;
    }
    Some(format!("{} {}", first, last).trim().to_string())
}

/// Message bodies arrive as either a bare string or `{text: "..."}`.
fn message_body_text(msg: &serde_json::Value) -> Option<String> {
    let body = msg.get("body")?;
    if body.is_string() {
        return body.as_str().map(str::to_string);
    }
    body.get("text")
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

fn render_content_lines(msg: &serde_json::Value) -> Vec<String> {
    let Some(arr) = msg.get("renderContentUnions").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    arr.iter().filter_map(external_media_line).collect()
}

fn external_media_line(rc: &serde_json::Value) -> Option<String> {
    let em = rc.get("externalMedia")?;
    let url = em.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return None;
    }
    let title = em.get("title").and_then(|v| v.as_str()).unwrap_or("");
    Some(format!("  [link] {} {}", title, url))
}
