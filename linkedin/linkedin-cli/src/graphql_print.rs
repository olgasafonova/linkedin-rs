use crate::util::truncate_with_ellipsis;

/// Print a conversation from the GraphQL `messengerConversationsByCategory` response.
pub fn print_graphql_conversation(index: usize, conv: &serde_json::Value) {
    let backend_urn = conv
        .get("backendUrn")
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let conv_id = backend_urn
        .strip_prefix("urn:li:messagingThread:")
        .unwrap_or(backend_urn);

    let read = conv.get("read").and_then(|r| r.as_bool()).unwrap_or(true);
    let unread_marker = if read { " " } else { "*" };

    let unread_count = conv
        .get("unreadCount")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    // Extract participant names from conversationParticipants.
    let mut names = Vec::new();
    if let Some(participants) = conv
        .get("conversationParticipants")
        .and_then(|p| p.as_array())
    {
        for p in participants {
            let name = p
                .get("participantType")
                .and_then(|pt| pt.get("member"))
                .and_then(|member| {
                    let first = member
                        .get("firstName")
                        .and_then(|f| f.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let last = member
                        .get("lastName")
                        .and_then(|l| l.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if first.is_empty() && last.is_empty() {
                        None
                    } else {
                        Some(format!("{} {}", first, last).trim().to_string())
                    }
                });
            if let Some(n) = name {
                names.push(n);
            }
        }
    }

    // Last message from inline messages.elements.
    let last_message = conv
        .get("messages")
        .and_then(|m| m.get("elements"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|msg| msg.get("body"))
        .and_then(|b| {
            if b.is_string() {
                b.as_str().map(|s| s.to_string())
            } else {
                b.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            }
        })
        .unwrap_or_default();

    let last_msg_display = truncate_with_ellipsis(&last_message, 80);

    let title = conv.get("title").and_then(|n| n.as_str()).unwrap_or("");
    let display_name = if !title.is_empty() {
        title.to_string()
    } else if !names.is_empty() {
        names.join(", ")
    } else {
        "(unknown)".to_string()
    };

    println!(
        "[{}]{} {} (id: {})",
        index, unread_marker, display_name, conv_id
    );
    if unread_count > 0 {
        println!("    unread: {}", unread_count);
    }
    if !last_msg_display.is_empty() {
        println!("    last: {}", last_msg_display);
    }
}

/// Print a message from the GraphQL `messengerMessagesByConversation` response.
///
/// Supports threading: if a message is a reply, shows the parent message
/// context indented above the reply.
pub fn print_graphql_message(msg: &serde_json::Value) {
    // Check for reply/thread context first.
    let reply_to = msg
        .get("replyMessage")
        .or_else(|| msg.get("parentMessage"))
        .or_else(|| msg.get("quotedMessage"));

    if let Some(parent) = reply_to {
        let parent_sender = parent
            .get("sender")
            .and_then(|s| s.get("hostIdentityUrn"))
            .and_then(|u| u.as_str())
            .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
            .unwrap_or("unknown");
        let parent_body = parent
            .get("body")
            .and_then(|b| {
                if b.is_string() {
                    b.as_str().map(|s| s.to_string())
                } else {
                    b.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                }
            })
            .unwrap_or_default();
        if !parent_body.is_empty() {
            println!(
                "  > {} said: {}",
                parent_sender,
                truncate_with_ellipsis(&parent_body, 80)
            );
        }
    }

    // Timestamp from deliveredAt.
    let delivered_at = msg.get("deliveredAt").and_then(|c| c.as_u64()).unwrap_or(0);
    let time_str = if delivered_at > 0 {
        let secs = (delivered_at / 1000) as i64;
        let nanos = ((delivered_at % 1000) * 1_000_000) as u32;
        chrono::DateTime::from_timestamp(secs, nanos)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| delivered_at.to_string())
    } else {
        String::new()
    };

    // Sender -- try to get the name first, fall back to URN.
    let sender = msg.get("sender");
    let sender_name = sender.and_then(|s| {
        // Try member.firstName + lastName via participantType path.
        s.get("participantType")
            .and_then(|pt| pt.get("member"))
            .and_then(|m| {
                let first = m
                    .get("firstName")
                    .and_then(|f| {
                        f.get("text")
                            .and_then(|v| v.as_str())
                            .or_else(|| f.as_str())
                    })
                    .unwrap_or("");
                let last = m
                    .get("lastName")
                    .and_then(|l| {
                        l.get("text")
                            .and_then(|v| v.as_str())
                            .or_else(|| l.as_str())
                    })
                    .unwrap_or("");
                if first.is_empty() && last.is_empty() {
                    None
                } else {
                    Some(format!("{} {}", first, last).trim().to_string())
                }
            })
    });

    let sender_display = sender_name.unwrap_or_else(|| {
        sender
            .and_then(|s| s.get("hostIdentityUrn"))
            .and_then(|u| u.as_str())
            .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
            .unwrap_or("unknown")
            .to_string()
    });

    // Message body.
    let body = msg
        .get("body")
        .and_then(|b| {
            if b.is_string() {
                b.as_str().map(|s| s.to_string())
            } else {
                b.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            }
        })
        .unwrap_or_default();

    let subject = msg.get("subject").and_then(|s| s.as_str()).unwrap_or("");

    // Render content attachments (shared links, etc.).
    let render_content = msg
        .get("renderContentUnions")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|rc| {
                    rc.get("externalMedia").and_then(|em| {
                        let title = em.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let url = em.get("url").and_then(|v| v.as_str()).unwrap_or("");
                        if !url.is_empty() {
                            Some(format!("  [link] {} {}", title, url))
                        } else {
                            None
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !time_str.is_empty() {
        print!("[{}] ", time_str);
    }
    println!("{}", sender_display);
    if !subject.is_empty() {
        println!("  Subject: {}", subject);
    }
    if !body.is_empty() {
        println!("  {}", body);
    }
    for content in &render_content {
        println!("{}", content);
    }
}
