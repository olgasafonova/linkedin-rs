/// Spam-detection patterns for recruiter messages and invitations.
///
/// Returns `true` if the text looks like recruiter spam. Patterns are
/// case-insensitive substring matches against common recruiter template
/// phrases and headline keywords.
pub fn looks_like_spam(text: &str) -> bool {
    let lower = text.to_lowercase();

    // Recruiter headline patterns (invitation subtitles / sender occupations).
    const HEADLINE_PATTERNS: &[&str] = &[
        "recruiter",
        "talent acquisition",
        "staffing",
        "headhunter",
        "hiring manager",
        "sourcer",
        "recruitment",
        "people partner",
    ];

    // Message body template phrases.
    const BODY_PATTERNS: &[&str] = &[
        "exciting opportunity",
        "perfect fit",
        "open role",
        "open position",
        "i came across your profile",
        "i found your profile",
        "your background caught",
        "impressive profile",
        "impressive background",
        "great match",
        "thriving team",
        "i'd love to connect",
        "competitive salary",
        "competitive compensation",
        "base salary",
        "reach out to discuss",
        "we are currently hiring",
        "we're currently hiring",
        "looking for a",
        "we are looking",
        "we're looking",
        "wonderful opportunity",
        "are you open to",
    ];

    for pat in HEADLINE_PATTERNS.iter().chain(BODY_PATTERNS.iter()) {
        if lower.contains(pat) {
            return true;
        }
    }
    false
}

/// Check if a conversation element looks like recruiter spam.
///
/// Checks participant names/headlines and the last message body.
pub fn is_spam_conversation(conv: &serde_json::Value) -> bool {
    // Check title field.
    if let Some(title) = conv.get("title").and_then(|v| v.as_str()) {
        if looks_like_spam(title) {
            return true;
        }
    }

    // Check participant headlines/occupations.
    if let Some(participants) = conv
        .get("conversationParticipants")
        .and_then(|p| p.as_array())
    {
        for p in participants {
            if let Some(member) = p.get("participantType").and_then(|pt| pt.get("member")) {
                if let Some(headline) = member.get("headline").and_then(|h| {
                    h.get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| h.as_str())
                }) {
                    if looks_like_spam(headline) {
                        return true;
                    }
                }
                if let Some(occ) = member.get("occupation").and_then(|v| v.as_str()) {
                    if looks_like_spam(occ) {
                        return true;
                    }
                }
            }
        }
    }

    // Check last message body.
    let last_msg_text = conv
        .get("messages")
        .and_then(|m| m.get("elements"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|msg| {
            msg.get("body").and_then(|b| {
                if b.is_string() {
                    b.as_str().map(|s| s.to_string())
                } else {
                    b.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                }
            })
        })
        .unwrap_or_default();

    looks_like_spam(&last_msg_text)
}

/// Check if an invitation element looks like recruiter spam.
pub fn is_spam_invitation(inv: &serde_json::Value) -> bool {
    if let Some(headline) = inv
        .get("subtitle")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
    {
        if looks_like_spam(headline) {
            return true;
        }
    }
    if let Some(msg) = inv
        .get("invitation")
        .and_then(|i| i.get("message"))
        .and_then(|v| v.as_str())
    {
        if looks_like_spam(msg) {
            return true;
        }
    }
    false
}
