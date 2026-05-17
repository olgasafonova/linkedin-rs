//! `feed read` and `feed view` -- expanded single-item view.

use serde_json::Value;

use linkedin_api::models::FeedResponse;

use crate::error::{CliError, CliResult};
use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

use super::cache::load_feed_cache;
use super::helpers::{
    actor_name, commentary_text, field_str, nested_text, print_json, social_count, unwrap_update_v2,
};
use linkedin_api::feed_extract::{
    extract_article_info, extract_media_type_label, extract_media_urls,
};

use super::urn::extract_activity_urn;

pub fn cmd_feed_read(index: usize, raw_json: bool) -> CliResult<()> {
    if index == 0 {
        return Err(CliError::Other("index must be >= 1".to_string()));
    }
    let cache = load_feed_cache()?;
    let feed: FeedResponse = serde_json::from_value(cache.clone())
        .map_err(|e| CliError::Other(format!("failed to parse cached feed: {e}")))?;
    let element = feed.elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (feed has {} items)",
            index,
            feed.elements.len()
        )
    })?;
    if raw_json {
        return print_json(element);
    }
    print_feed_item_full(index, element);
    Ok(())
}

pub async fn cmd_feed_view(activity_urn: &str, raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;
    let post = client.get_post(activity_urn).await?;
    if raw_json {
        return print_json(&post);
    }
    print_feed_item_full(0, &post);
    Ok(())
}

fn print_feed_item_full(index: usize, item: &Value) {
    let update = unwrap_update_v2(item);
    print_full_header(index, update);
    print_full_body(update);
    print_full_media(update);
    print_full_footer(item, update);
}

fn print_full_header(index: usize, update: &Value) {
    println!("[{}] {}", index, actor_name(update));
    let actor_description = nested_text(update, &["actor", "description", "text"]).unwrap_or("");
    if !actor_description.is_empty() {
        println!("    {}", actor_description);
    }
    println!();
}

fn print_full_body(update: &Value) {
    let commentary = nested_text(update, &["commentary", "text", "text"]).unwrap_or("(no text)");
    println!("{}", commentary);

    let Some(reshared) = reshared_update(update) else {
        return;
    };
    let orig_author = actor_name(reshared);
    let orig_text = commentary_text(reshared);
    println!();
    println!("  [reshared from {}]", orig_author);
    if !orig_text.is_empty() {
        println!("  {}", truncate_with_ellipsis(orig_text, 300));
    }
}

fn reshared_update(update: &Value) -> Option<&Value> {
    update
        .get("resharedUpdate")
        .or_else(|| update.get("content").and_then(|c| c.get("resharedUpdate")))
        .or_else(|| {
            update.get("content").and_then(|c| {
                linkedin_api::restli::unwrap_union(c, linkedin_api::restli::UPDATE_V2_KEY)
            })
        })
}

fn print_full_media(update: &Value) {
    if let Some(article) = extract_article_info(update) {
        println!();
        if !article.title.is_empty() {
            println!("Article: {}", article.title);
        }
        if !article.url.is_empty() {
            println!("  {}", article.url);
        }
    }

    let media_label = extract_media_type_label(update);
    if !media_label.is_empty() {
        println!("Media: {}", media_label);
    }
    for url in extract_media_urls(update) {
        println!("  {}", url);
    }
}

fn print_full_footer(item: &Value, update: &Value) {
    println!();
    println!(
        "likes: {}  comments: {}  shares: {}",
        social_count(update, "numLikes"),
        social_count(update, "numComments"),
        social_count(update, "numShares"),
    );
    let entity_urn = field_str(item, "entityUrn");
    let activity_urn = extract_activity_urn(entity_urn).unwrap_or_default();
    if !activity_urn.is_empty() {
        println!("URN: {}", activity_urn);
    }
    let permalink = field_str(item, "permalink");
    if !permalink.is_empty() {
        println!("Link: {}", permalink);
    }
}
