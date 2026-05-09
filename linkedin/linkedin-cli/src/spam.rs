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
/// Checks the title, participant names/headlines, and the last message body.
pub fn is_spam_conversation(conv: &serde_json::Value) -> bool {
    title_looks_spammy(conv)
        || any_participant_looks_spammy(conv)
        || last_message_looks_spammy(conv)
}

fn title_looks_spammy(conv: &serde_json::Value) -> bool {
    conv.get("title")
        .and_then(|v| v.as_str())
        .is_some_and(looks_like_spam)
}

fn any_participant_looks_spammy(conv: &serde_json::Value) -> bool {
    conv.get("conversationParticipants")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .any(participant_looks_spammy)
}

fn participant_looks_spammy(p: &serde_json::Value) -> bool {
    let Some(member) = p.get("participantType").and_then(|pt| pt.get("member")) else {
        return false;
    };
    let headline_match = member
        .get("headline")
        .and_then(|h| {
            h.get("text")
                .and_then(|v| v.as_str())
                .or_else(|| h.as_str())
        })
        .is_some_and(looks_like_spam);
    if headline_match {
        return true;
    }
    member
        .get("occupation")
        .and_then(|v| v.as_str())
        .is_some_and(looks_like_spam)
}

fn last_message_looks_spammy(conv: &serde_json::Value) -> bool {
    let Some(text) = last_message_body(conv) else {
        return false;
    };
    looks_like_spam(&text)
}

/// Message bodies arrive as either a bare string or `{text: "..."}`.
fn last_message_body(conv: &serde_json::Value) -> Option<String> {
    let body = conv
        .get("messages")
        .and_then(|m| m.get("elements"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())?
        .get("body")?;
    if body.is_string() {
        return body.as_str().map(str::to_string);
    }
    body.get("text")
        .and_then(|t| t.as_str())
        .map(str::to_string)
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
