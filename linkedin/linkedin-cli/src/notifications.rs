use linkedin_api::models::NotificationCardsResponse;
use linkedin_api::urn::{extract_activity_urn_from_url, find_fsd_profile_urn};

use crate::session::load_session_client;
use crate::util::{print_paging_header, truncate_with_ellipsis};

/// Handle `notifications list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls the Voyager GraphQL notifications endpoint
/// (`identityDashNotificationCardsByFilterVanityName`) with pagination
/// params, and prints the results.
pub async fn cmd_notifications_list(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_notifications(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    save_notifications_cache(&value)?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let resp: NotificationCardsResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse notifications response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header("Notifications", paging);
    }
    println!("---");

    if resp.elements.is_empty() {
        println!("(no notifications)");
        return Ok(());
    }

    for (i, element) in resp.elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_notification_card(idx, element);
        println!();
    }

    Ok(())
}

/// Handle `notifications mentions <index> [--json]`.
///
/// Indexes into the most recent `notifications list` cache, extracts the
/// underlying activity URN from `cardAction.actionTarget`, fetches the post
/// body, and walks the commentary's `attributesV2` array for entries whose
/// type carries an `fsd_profile` URN. The shape mirrors LinkedIn's Pemberly
/// attributed-text format used elsewhere in the API: each attribute has a
/// `start`, `length`, and a discriminator under `type` containing the
/// mentioned member's URN. The exact discriminator key is unverified
/// against a captured response, so the handler walks all entries
/// permissively and surfaces any URN it finds.
pub async fn cmd_notifications_mentions(index: usize, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let cache = load_notifications_cache()?;
    let card = cached_notification_at(&cache, index)?;
    let activity_urn = activity_urn_for_card(card, index)?;

    let (client, _path) = load_session_client()?;
    let post = client
        .get_post(&activity_urn)
        .await
        .map_err(|e| format!("failed to fetch post {}: {e}", activity_urn))?;
    let mentions = collect_post_mentions(&post);

    if raw_json {
        return print_mentions_json(&activity_urn, &mentions);
    }
    print_mentions_report(&activity_urn, &mentions);
    Ok(())
}

fn cached_notification_at(
    cache: &serde_json::Value,
    index: usize,
) -> Result<&serde_json::Value, String> {
    let elements = cache
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "cached notifications response has no elements array".to_string())?;
    elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (cached notifications has {} items)",
            index,
            elements.len()
        )
    })
}

fn activity_urn_for_card(card: &serde_json::Value, index: usize) -> Result<String, String> {
    let action_target = card
        .get("cardAction")
        .and_then(|a| a.get("actionTarget"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    extract_activity_urn_from_url(action_target).ok_or_else(|| {
        format!(
            "notification {} has no activity URN in cardAction.actionTarget ({})",
            index, action_target
        )
    })
}

fn print_mentions_json(activity_urn: &str, mentions: &[serde_json::Value]) -> Result<(), String> {
    let payload = serde_json::json!({
        "activityUrn": activity_urn,
        "mentions": mentions,
    });
    let pretty =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("JSON format error: {e}"))?;
    println!("{}", pretty);
    Ok(())
}

fn print_mentions_report(activity_urn: &str, mentions: &[serde_json::Value]) {
    if mentions.is_empty() {
        println!("(no mentions found in {})", activity_urn);
        return;
    }
    println!("Mentions in {}:", activity_urn);
    for m in mentions {
        let urn = m.get("urn").and_then(|u| u.as_str()).unwrap_or("(no urn)");
        let name = m.get("text").and_then(|t| t.as_str()).unwrap_or("");
        if name.is_empty() {
            println!("  {}", urn);
        } else {
            println!("  {}  ({})", name, urn);
        }
    }
}

/// Walk a post's commentary attributes and pull any fsd_profile URNs out
/// of the structured mention data. The Pemberly attributed-text format
/// puts mentions under `commentary.text.attributesV2[].type.*.urn`; we
/// scan permissively because the discriminator key (e.g.
/// `com.linkedin.pemberly.text.Profile`, `MEMBER_MENTION`) varies by
/// payload version. Returns one JSON object per mention with `urn` and
/// optional `text` (the @mention literal as it appears in the post).
fn collect_post_mentions(post: &serde_json::Value) -> Vec<serde_json::Value> {
    let update = linkedin_api::restli::unwrap_update_v2(post);

    let commentary = match update.get("commentary").and_then(|c| c.get("text")) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let body_text = commentary
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let attrs = commentary
        .get("attributesV2")
        .or_else(|| commentary.get("attributes"))
        .and_then(|a| a.as_array());
    let attrs = match attrs {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    for attr in attrs {
        // Walk every leaf string under this attribute looking for an
        // fsd_profile URN. The exact key path varies; this catches:
        //   type.com.linkedin.pemberly.text.Profile.urn
        //   type.MEMBER_MENTION.profileUrn
        //   miniProfile.entityUrn
        let urn = find_fsd_profile_urn(attr);
        if let Some(urn) = urn {
            // Try to splice the mention text out of the body using start+length.
            let start = attr.get("start").and_then(|s| s.as_u64()).unwrap_or(0) as usize;
            let length = attr.get("length").and_then(|l| l.as_u64()).unwrap_or(0) as usize;
            let snippet = body_text
                .get(start..start.saturating_add(length))
                .unwrap_or("");
            out.push(serde_json::json!({
                "urn": urn,
                "text": snippet,
            }));
        }
    }
    out
}

/// Print a brief human-readable summary of a single notification card.
///
/// Notification cards use TextViewModel wrappers for text fields; we extract
/// the inner `text` string from `headline`, `subHeadline`, and `kicker`.
fn print_notification_card(index: usize, card: &serde_json::Value) {
    let unread_marker = if card_is_read(card) { " " } else { "*" };
    let headline = card_text(card, "headline").unwrap_or("(no headline)");

    println!(
        "[{}]{} {}",
        index,
        unread_marker,
        truncate_with_ellipsis(headline, 120)
    );

    if let Some(preview) = content_preview(card) {
        println!("    \"{}\"", preview);
    }
    if let Some(sub) = card_text(card, "subHeadline").filter(|s| !s.is_empty()) {
        println!("    {}", sub);
    }
    let meta = card_meta_line(card);
    if !meta.is_empty() {
        println!("    {}", meta);
    }
    if let Some(url) = card_post_link(card) {
        println!("    {}", url);
    }
}

fn card_is_read(card: &serde_json::Value) -> bool {
    card.get("read").and_then(|r| r.as_bool()).unwrap_or(true)
}

/// Extract the inner string from a TextViewModel-wrapped field.
fn card_text<'a>(card: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    card.get(field)
        .and_then(|h| h.get("text"))
        .and_then(|t| t.as_str())
}

fn content_preview(card: &serde_json::Value) -> Option<String> {
    let text = card
        .get("contentPrimaryText")
        .and_then(|arr| arr.as_array())
        .and_then(|a| a.first())
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())?;
    let first_line = text.lines().next().unwrap_or("");
    if first_line.is_empty() {
        return None;
    }
    Some(truncate_with_ellipsis(first_line, 100))
}

fn card_meta_line(card: &serde_json::Value) -> String {
    let social = card.get("socialActivityCounts");
    let num_likes = social
        .and_then(|s| s.get("numLikes").and_then(|n| n.as_u64()))
        .unwrap_or(0);
    let num_comments = social
        .and_then(|s| s.get("numComments").and_then(|n| n.as_u64()))
        .unwrap_or(0);
    let content_type = card
        .get("contentType")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let published_at = card
        .get("publishedAt")
        .and_then(|p| p.as_i64())
        .and_then(|millis| {
            chrono::DateTime::from_timestamp(millis / 1000, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        })
        .unwrap_or_default();

    let mut parts = Vec::new();
    if num_likes > 0 || num_comments > 0 {
        parts.push(format!("likes: {}  comments: {}", num_likes, num_comments));
    }
    if !content_type.is_empty() {
        parts.push(format!("type: {}", content_type));
    }
    if !published_at.is_empty() {
        parts.push(published_at);
    }
    parts.join("  |  ")
}

fn card_post_link(card: &serde_json::Value) -> Option<String> {
    let target = card
        .get("cardAction")
        .and_then(|a| a.get("actionTarget"))
        .and_then(|t| t.as_str())?;
    if target.is_empty() || !target.contains("/feed/") {
        return None;
    }
    Some(format!(
        "https://www.linkedin.com{}",
        target.replace("%3A", ":").replace("%2F", "/")
    ))
}

fn notifications_cache_path() -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir.join("linkedin").join("last_notifications.json"))
}

fn save_notifications_cache(value: &serde_json::Value) -> Result<(), String> {
    let path = notifications_cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json = serde_json::to_string(value)
        .map_err(|e| format!("failed to serialize notifications cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write notifications cache: {e}"))?;
    Ok(())
}

fn load_notifications_cache() -> Result<serde_json::Value, String> {
    let path = notifications_cache_path()?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| "no cached notifications. Run `notifications list` first.".to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse notifications cache: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collect_post_mentions_walks_attributesv2_for_fsd_profile_urns() {
        let post = json!({
            "commentary": {
                "text": {
                    "text": "Great work @Alice and @Bob!",
                    "attributesV2": [
                        {
                            "start": 11,
                            "length": 6,
                            "type": {
                                "com.linkedin.pemberly.text.Profile": {
                                    "urn": "urn:li:fsd_profile:ACoAAAAAAAAAAA"
                                }
                            }
                        },
                        {
                            "start": 22,
                            "length": 4,
                            "type": {
                                "com.linkedin.pemberly.text.Profile": {
                                    "urn": "urn:li:fsd_profile:ACoAAABBBBBBBB"
                                }
                            }
                        }
                    ]
                }
            }
        });
        let mentions = collect_post_mentions(&post);
        assert_eq!(mentions.len(), 2);
        assert_eq!(
            mentions[0].get("urn").and_then(|v| v.as_str()),
            Some("urn:li:fsd_profile:ACoAAAAAAAAAAA")
        );
        assert_eq!(
            mentions[0].get("text").and_then(|v| v.as_str()),
            Some("@Alice")
        );
        assert_eq!(
            mentions[1].get("text").and_then(|v| v.as_str()),
            Some("@Bob")
        );
    }

    #[test]
    fn collect_post_mentions_returns_empty_when_no_attributes() {
        let post = json!({
            "commentary": { "text": { "text": "Plain post, no mentions" } }
        });
        assert!(collect_post_mentions(&post).is_empty());
    }
}
