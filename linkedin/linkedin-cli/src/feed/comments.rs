//! `feed comments` command.

use serde_json::Value;

use crate::session::load_session_client;

use super::cache::cached_feed_element;
use super::helpers::{print_json, unwrap_update_v2};

pub async fn cmd_feed_comments(index: usize, count: u32, raw_json: bool) -> Result<(), String> {
    let element = cached_feed_element(index)?;
    let social_detail_urn = social_detail_urn(&element)?;

    let (client, _path) = load_session_client()?;
    let value = client
        .get_comments(&social_detail_urn, 0, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        return print_json(&value);
    }

    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    if elements.is_empty() {
        println!("(no comments)");
        return Ok(());
    }

    println!("Comments on post [{}]:", index);
    println!("---");
    for (i, comment) in elements.iter().enumerate() {
        print_comment(i + 1, comment);
    }
    Ok(())
}

fn social_detail_urn(element: &Value) -> Result<String, String> {
    let update = unwrap_update_v2(element);
    update
        .get("socialDetail")
        .and_then(|sd| sd.get("dashEntityUrn").or_else(|| sd.get("entityUrn")))
        .and_then(|u| u.as_str())
        .map(str::to_string)
        .ok_or_else(|| "feed item has no socialDetail URN".to_string())
}

fn print_comment(index: usize, comment: &Value) {
    let commenter = comment_author(comment);
    let text = comment_text(comment);
    let likes = comment
        .get("socialDetail")
        .and_then(|sd| sd.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numLikes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    println!("[{}] {}", index, commenter);
    println!("    {}", text);
    if likes > 0 {
        println!("    likes: {}", likes);
    }
    println!();
}

fn comment_author(comment: &Value) -> &str {
    comment
        .get("commenter")
        .and_then(|c| {
            c.get("title")
                .and_then(|t| t.get("text"))
                .and_then(|v| v.as_str())
                .or_else(|| c.get("accessibilityText").and_then(|v| v.as_str()))
        })
        .unwrap_or("(unknown)")
}

fn comment_text(comment: &Value) -> &str {
    comment
        .get("commentary")
        .and_then(|c| {
            c.get("text").and_then(|t| {
                t.as_str()
                    .or_else(|| t.get("text").and_then(|v| v.as_str()))
            })
        })
        .unwrap_or("")
}
