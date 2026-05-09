//! `feed my-posts` command.

use serde_json::Value;

use linkedin_api::models::FeedResponse;

use crate::session::load_session_client;
use crate::util::{print_paging_header, truncate_with_ellipsis};

use super::cache::save_feed_cache;
use super::helpers::{
    commentary_text, field_str, nested_text, print_json, reaction_emoji, social_count,
    unwrap_update_v2,
};
use super::urn::extract_activity_urn;

pub async fn cmd_feed_my_posts(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_my_posts(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if let Err(e) = save_feed_cache(&value) {
        eprintln!("warning: failed to cache feed: {}", e);
    }

    if raw_json {
        return print_json(&value);
    }

    let feed: FeedResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse response: {e}"))?;

    if let Some(ref paging) = feed.paging {
        print_paging_header("My posts", paging);
    }
    println!("---");

    if feed.elements.is_empty() {
        println!("(no posts found)");
        return Ok(());
    }

    for (i, element) in feed.elements.iter().enumerate() {
        print_my_post(start as usize + i + 1, element);
        println!();
    }
    Ok(())
}

fn print_my_post(index: usize, item: &Value) {
    let update = unwrap_update_v2(item);
    let entity_urn = field_str(item, "entityUrn");
    let activity_urn = extract_activity_urn(entity_urn)
        .map(|u| u.split(',').next().unwrap_or(&u).to_string())
        .unwrap_or_default();
    let display_urn = if !activity_urn.is_empty() {
        activity_urn.as_str()
    } else {
        entity_urn
    };
    println!("[{}] {}", index, display_urn);

    let time_desc = my_post_time_label(update);
    if !time_desc.is_empty() {
        println!("    posted: {}", time_desc);
    }
    let commentary_display = truncate_with_ellipsis(commentary_text(update), 120);
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    print_my_post_metrics(update);
    print_my_post_reactions(update);
}

fn my_post_time_label(update: &Value) -> String {
    nested_text(update, &["actor", "subDescription", "text"])
        .unwrap_or("")
        .trim()
        .trim_end_matches("• \u{a0}\u{a0}")
        .trim_end_matches('•')
        .trim()
        .to_string()
}

fn print_my_post_metrics(update: &Value) {
    println!(
        "    views: {}  reactions: {}  comments: {}  reposts: {}",
        social_count(update, "numViews"),
        social_count(update, "numLikes"),
        social_count(update, "numComments"),
        social_count(update, "numShares"),
    );
}

fn print_my_post_reactions(update: &Value) {
    let Some(rtc) = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("reactionTypeCounts"))
        .and_then(|r| r.as_array())
    else {
        return;
    };
    if rtc.is_empty() {
        return;
    }
    let parts: Vec<String> = rtc
        .iter()
        .filter_map(|r| {
            let rtype = r.get("reactionType").and_then(|t| t.as_str())?;
            let count = r.get("count").and_then(|c| c.as_u64())?;
            Some(format!("{}: {}", reaction_emoji(rtype), count))
        })
        .collect();
    if !parts.is_empty() {
        println!("    {}", parts.join("  "));
    }
}
