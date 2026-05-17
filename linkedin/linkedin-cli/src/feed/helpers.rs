//! Shared helpers used across feed submodules.

use serde_json::Value;

/// Pretty-print a JSON value to stdout.
pub(super) fn print_json(value: &Value) -> Result<(), String> {
    let pretty =
        serde_json::to_string_pretty(value).map_err(|e| format!("JSON format error: {e}"))?;
    println!("{}", pretty);
    Ok(())
}

/// Walk a nested object path, returning the final string if every step
/// resolves and the leaf is a string.
pub(super) fn nested_text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    current.as_str()
}

/// Read a string field directly under `value`, or "" if missing.
pub(super) fn field_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

/// Read `<key>.text` -- the GraphQL TextViewModel shape.
pub(super) fn text_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    nested_text(value, &[key, "text"])
}

pub(super) use linkedin_api::restli::unwrap_update_v2;

/// Read commentary text. The shape is `commentary.text.text`.
pub(super) fn commentary_text(update: &Value) -> &str {
    nested_text(update, &["commentary", "text", "text"]).unwrap_or("")
}

/// Read the actor's display name from `actor.name.text`.
pub(super) fn actor_name(update: &Value) -> &str {
    nested_text(update, &["actor", "name", "text"]).unwrap_or("(unknown author)")
}

/// Read a `socialDetail.totalSocialActivityCounts.<key>` count.
pub(super) fn social_count(update: &Value, key: &str) -> u64 {
    update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get(key))
        .and_then(|n| n.as_u64())
        .unwrap_or(0)
}

/// Map reaction type strings to compact display labels.
pub(super) fn reaction_emoji(reaction_type: &str) -> &str {
    match reaction_type {
        "LIKE" => "like",
        "PRAISE" | "CELEBRATION" => "celebrate",
        "EMPATHY" => "love",
        "INTEREST" | "CURIOSITY" => "insightful",
        "APPRECIATION" => "support",
        "ENTERTAINMENT" => "funny",
        _ => reaction_type,
    }
}
