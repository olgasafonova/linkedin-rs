//! `feed list` command.

use serde_json::Value;

use linkedin_api::models::FeedResponse;

use crate::session::load_session_client;
use crate::util::{print_paging_header, truncate_with_ellipsis};

use super::cache::save_feed_cache;
use super::helpers::{
    actor_name, commentary_text, field_str, print_json, social_count, unwrap_update_v2,
};
use linkedin_api::feed_extract::extract_media_type_label;

/// Options for `feed list`. Bundles the five-argument call into a struct.
pub struct FeedListOptions<'a> {
    pub start: u32,
    pub count: u32,
    pub author_filter: Option<&'a str>,
    pub keyword_filter: Option<&'a str>,
    pub raw_json: bool,
}

/// Handle `feed list [--count N] [--start N] [--json]`.
pub async fn cmd_feed_list(opts: FeedListOptions<'_>) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_feed(opts.start, opts.count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if let Err(e) = save_feed_cache(&value) {
        eprintln!("warning: failed to cache feed: {e}");
    }

    if opts.raw_json {
        return print_json(&value);
    }

    let feed: FeedResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse feed response: {e}"))?;

    print_feed_list_header(&feed, opts.author_filter, opts.keyword_filter);

    if feed.elements.is_empty() {
        println!("(no feed items)");
        return Ok(());
    }

    let author_lower = opts.author_filter.map(str::to_lowercase);
    let keyword_lower = opts.keyword_filter.map(str::to_lowercase);

    let mut shown = 0;
    for (i, element) in feed.elements.iter().enumerate() {
        if !feed_item_passes_filters(element, author_lower.as_deref(), keyword_lower.as_deref()) {
            continue;
        }
        shown += 1;
        print_feed_item(opts.start as usize + i + 1, element);
        println!();
    }

    if shown == 0 {
        println!("(no matching feed items)");
    }
    Ok(())
}

fn print_feed_list_header(
    feed: &FeedResponse,
    author_filter: Option<&str>,
    keyword_filter: Option<&str>,
) {
    if let Some(ref paging) = feed.paging {
        print_paging_header("Feed updates", paging);
    }
    if let Some(author) = author_filter {
        println!("  filter: author contains \"{}\"", author);
    }
    if let Some(keyword) = keyword_filter {
        println!("  filter: text contains \"{}\"", keyword);
    }
    println!("---");
}

fn feed_item_passes_filters(
    element: &Value,
    author_lower: Option<&str>,
    keyword_lower: Option<&str>,
) -> bool {
    if author_lower.is_none() && keyword_lower.is_none() {
        return true;
    }
    let update = unwrap_update_v2(element);
    matches_lowercase(actor_name(update), author_lower)
        && matches_lowercase(commentary_text(update), keyword_lower)
}

/// True when `needle` is None, or `haystack` contains `needle` (both
/// compared lowercase).
fn matches_lowercase(haystack: &str, needle: Option<&str>) -> bool {
    match needle {
        None => true,
        Some(q) => haystack.to_lowercase().contains(q),
    }
}

/// Print a brief human-readable summary of a single feed item.
fn print_feed_item(index: usize, item: &Value) {
    let update = unwrap_update_v2(item);
    let actor = actor_name(update);
    let commentary_display = truncate_with_ellipsis(commentary_text(update), 120);
    let urn = field_str(item, "entityUrn");
    let likes = social_count(update, "numLikes");
    let comments = social_count(update, "numComments");
    let media_label = extract_media_type_label(update);

    println!("[{}] {} {}", index, actor, urn);
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    if !media_label.is_empty() {
        print!("    [{}]  ", media_label);
    } else {
        print!("    ");
    }
    println!("likes: {}  comments: {}", likes, comments);
}
