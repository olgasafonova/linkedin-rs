use serde_json::Value;

use linkedin_api::models::FeedResponse;

use crate::session::load_session_client;
use crate::util::{print_paging_header, truncate_with_ellipsis};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Pretty-print a JSON value to stdout.
fn print_json(value: &Value) -> Result<(), String> {
    let pretty =
        serde_json::to_string_pretty(value).map_err(|e| format!("JSON format error: {e}"))?;
    println!("{}", pretty);
    Ok(())
}

/// Walk a nested object path, returning the final string if every step
/// resolves and the leaf is a string.
fn nested_text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    current.as_str()
}

/// Read a string field directly under `value`, or "" if missing.
fn field_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

/// Read `<key>.text` -- the GraphQL TextViewModel shape.
fn text_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    nested_text(value, &[key, "text"])
}

/// Strip the optional `value.com.linkedin.voyager.feed.render.UpdateV2`
/// wrapper that wraps feed elements in the live response shape.
fn unwrap_update_v2(element: &Value) -> &Value {
    element
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(element)
}

/// Read commentary text. The shape is `commentary.text.text`.
fn commentary_text(update: &Value) -> &str {
    nested_text(update, &["commentary", "text", "text"]).unwrap_or("")
}

/// Read the actor's display name from `actor.name.text`.
fn actor_name(update: &Value) -> &str {
    nested_text(update, &["actor", "name", "text"]).unwrap_or("(unknown author)")
}

/// Read a `socialDetail.totalSocialActivityCounts.<key>` count.
fn social_count(update: &Value, key: &str) -> u64 {
    update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get(key))
        .and_then(|n| n.as_u64())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// feed list
// ---------------------------------------------------------------------------

/// Options for `feed list`. Bundles the five-argument call into a struct.
pub struct FeedListOptions<'a> {
    pub start: u32,
    pub count: u32,
    pub author_filter: Option<&'a str>,
    pub keyword_filter: Option<&'a str>,
    pub raw_json: bool,
}

/// Handle `feed list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/feed/updates?q=findFeed with
/// pagination params, and prints the results.
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

// ---------------------------------------------------------------------------
// feed my-posts
// ---------------------------------------------------------------------------

/// Handle `feed my-posts [--start N] [--count N] [--json]`.
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

/// Pull the timestamp string from `actor.subDescription.text`, stripping
/// the trailing bullet/spacer LinkedIn appends.
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

/// Map reaction type strings to compact display labels.
fn reaction_emoji(reaction_type: &str) -> &str {
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

// ---------------------------------------------------------------------------
// feed reactions
// ---------------------------------------------------------------------------

/// Options for `feed reactions`. Bundles the five-argument call into a struct.
pub struct FeedReactionsOptions<'a> {
    pub post_urn: Option<&'a str>,
    pub from_list: Option<usize>,
    pub start: u32,
    pub count: u32,
    pub raw_json: bool,
}

/// Handle `feed reactions <post_urn> [--start N] [--count N] [--json]`.
pub async fn cmd_feed_reactions(opts: FeedReactionsOptions<'_>) -> Result<(), String> {
    let urn = match (opts.post_urn, opts.from_list) {
        (Some(u), None) => normalize_reactions_urn(u),
        (None, Some(index)) => reactions_urn_from_cache(index)?,
        (None, None) => {
            return Err(
                "provide a post URN or use --from-list N after `feed list`/`feed my-posts`"
                    .to_string(),
            );
        }
        (Some(_), Some(_)) => unreachable!("clap conflicts_with guards this"),
    };

    let (client, _path) = load_session_client()?;
    let value = client
        .get_post_reactions(&urn, opts.start, opts.count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if opts.raw_json {
        return print_json(&value);
    }

    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let total = value
        .get("paging")
        .and_then(|p| p.get("total"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    println!("Reactions on {} ({} total)", urn, total);
    println!("---");

    if elements.is_empty() {
        println!("(no reactions)");
        print_reactions_empty_hint(&urn, opts.from_list);
        return Ok(());
    }

    for element in elements {
        print_reaction_row(element);
    }
    Ok(())
}

fn print_reaction_row(element: &Value) {
    let lockup = element.get("reactorLockup").unwrap_or(element);
    let name = text_field(lockup, "title").unwrap_or("(unknown)");
    let subtitle = text_field(lockup, "subtitle").unwrap_or("");
    let rtype = element
        .get("reactionType")
        .and_then(|r| r.as_str())
        .unwrap_or("?");

    let subtitle_display = truncate_with_ellipsis(subtitle, 60);
    let suffix = if subtitle_display.is_empty() {
        String::new()
    } else {
        format!("- {}", subtitle_display)
    };
    println!("  {:12} {} {}", reaction_emoji(rtype), name, suffix);
}

fn print_reactions_empty_hint(urn: &str, from_list: Option<usize>) {
    if from_list.is_some() || !urn.starts_with("urn:li:activity:") {
        return;
    }
    println!();
    println!(
        "Hint: LinkedIn's reactions endpoint uses different URN types \
         depending on the post backing (ugcPost for most posts, \
         activity for reshares). If you expected reactions here, try \
         `feed list` or `feed my-posts` first, then run \
         `feed reactions --from-list N` — the CLI will pick the \
         right URN. See re/reactions.md."
    );
}

// ---------------------------------------------------------------------------
// feed react / unreact / comment / post
// ---------------------------------------------------------------------------

pub async fn cmd_feed_react(
    post_urn: &str,
    reaction_type: &str,
    confirmed: bool,
    raw_json: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err(format!(
            "this will place a REAL {} reaction visible to the post's author. \
             Pass --yes to confirm.",
            reaction_type.to_uppercase()
        ));
    }

    let resolved_urn = resolve_post_urn(post_urn)?;
    let rt_upper = reaction_type.to_uppercase();
    let (client, _path) = load_session_client()?;

    eprintln!("Reacting to {} with {}...", resolved_urn, rt_upper);
    let result = client
        .react_to_post(&resolved_urn, &rt_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        print_json(&result)?;
    } else {
        println!("Reacted with {} to {}", rt_upper, resolved_urn);
    }
    Ok(())
}

pub async fn cmd_feed_unreact(
    post_urn: &str,
    reaction_type: &str,
    raw_json: bool,
) -> Result<(), String> {
    let resolved_urn = resolve_post_urn(post_urn)?;
    let rt_upper = reaction_type.to_uppercase();
    let (client, _path) = load_session_client()?;

    eprintln!("Removing {} reaction from {}...", resolved_urn, rt_upper);
    let result = client
        .unreact_from_post(&resolved_urn, &rt_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        print_json(&result)?;
    } else {
        println!("Removed {} reaction from {}", rt_upper, resolved_urn);
    }
    Ok(())
}

pub async fn cmd_feed_comment(
    post_urn: &str,
    text: &str,
    confirmed: bool,
    raw_json: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err("this will create a REAL COMMENT on a LinkedIn post. \
             Pass --yes to confirm."
            .to_string());
    }

    let resolved_urn = resolve_post_urn(post_urn)?;
    let (client, _path) = load_session_client()?;

    eprintln!("Commenting on {}...", resolved_urn);
    let result = client
        .comment_on_post(&resolved_urn, text)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        print_json(&result)?;
    } else {
        println!("Commented on {}", resolved_urn);
    }
    Ok(())
}

pub async fn cmd_feed_post(
    text: &str,
    visibility: &str,
    confirmed: bool,
    raw_json: bool,
) -> Result<(), String> {
    let vis_upper = visibility.to_uppercase();
    if vis_upper != "ANYONE" && vis_upper != "CONNECTIONS_ONLY" {
        return Err(format!(
            "invalid visibility '{}'. Must be ANYONE or CONNECTIONS_ONLY",
            visibility
        ));
    }

    if !confirmed {
        eprintln!("WARNING: This will create a REAL post on your LinkedIn account!");
        eprintln!();
        eprintln!("  Visibility: {}", vis_upper);
        eprintln!("  Text: {}", truncate_with_ellipsis(text, 200));
        eprintln!();
        eprintln!("Use --yes to confirm and publish this post.");
        return Err("post not confirmed (use --yes to publish)".to_string());
    }

    let (client, _path) = load_session_client()?;

    eprintln!("Creating post (visibility: {})...", vis_upper);
    let result = client
        .create_post(text, &vis_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        print_json(&result)?;
    } else {
        let urn = result
            .get("data")
            .and_then(|d| d.get("createContentcreationDashShares"))
            .and_then(|c| c.get("entityUrn"))
            .and_then(|v| v.as_str())
            .or_else(|| result.get("entityUrn").and_then(|v| v.as_str()))
            .unwrap_or("(unknown)");
        println!("Post created successfully!");
        println!("  URN: {}", urn);
        println!("  Visibility: {}", vis_upper);
        println!("  Text: {}", truncate_with_ellipsis(text, 100));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// feed stats
// ---------------------------------------------------------------------------

/// Handle `feed stats [--json]`.
pub async fn cmd_feed_stats(raw_json: bool) -> Result<(), String> {
    const POST_COUNT: u32 = 20;

    let (client, _path) = load_session_client()?;
    let value = client
        .get_my_posts(0, POST_COUNT)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let posts: Vec<PostStat> = elements.iter().map(extract_post_stat).collect();
    let totals = PostStat::sum(&posts);
    let n = posts.len() as u64;

    if raw_json {
        return print_stats_json(&posts, &totals, n);
    }
    if n == 0 {
        println!("(no posts found)");
        return Ok(());
    }
    print_stats_report(&posts, &totals, n);
    Ok(())
}

fn print_stats_json(posts: &[PostStat], totals: &PostStat, n: u64) -> Result<(), String> {
    let averages = match (
        totals.views.checked_div(n),
        totals.likes.checked_div(n),
        totals.comments.checked_div(n),
        totals.shares.checked_div(n),
    ) {
        (Some(v), Some(l), Some(c), Some(s)) => serde_json::json!({
            "views": v,
            "likes": l,
            "comments": c,
            "shares": s,
        }),
        _ => serde_json::json!({}),
    };
    let out = serde_json::json!({
        "post_count": n,
        "totals": {
            "views": totals.views,
            "likes": totals.likes,
            "comments": totals.comments,
            "shares": totals.shares,
        },
        "averages": averages,
        "posts": posts.iter().map(PostStat::to_json).collect::<Vec<_>>(),
    });
    print_json(&out)
}

fn print_stats_report(posts: &[PostStat], totals: &PostStat, n: u64) {
    println!("Engagement across last {} posts", n);
    println!("---");
    println!("Totals:");
    println!("  views:    {}", totals.views);
    println!("  likes:    {}", totals.likes);
    println!("  comments: {}", totals.comments);
    println!("  shares:   {}", totals.shares);
    println!();
    println!("Averages per post:");
    println!("  views:    {}", totals.views / n);
    println!("  likes:    {}", totals.likes / n);
    println!("  comments: {}", totals.comments / n);
    println!("  shares:   {}", totals.shares / n);
    println!();
    print_top_posts(posts);
}

fn print_top_posts(posts: &[PostStat]) {
    let mut ranked = posts.to_vec();
    ranked.sort_by_key(|p| std::cmp::Reverse(p.views));
    println!("Top posts by views:");
    for (i, p) in ranked.iter().take(5).enumerate() {
        println!(
            "[{}] {} views, {} likes, {} comments, {} shares",
            i + 1,
            p.views,
            p.likes,
            p.comments,
            p.shares
        );
        if !p.preview.is_empty() {
            println!("    {}", truncate_with_ellipsis(&p.preview, 100));
        }
    }
}

#[derive(Clone)]
struct PostStat {
    views: u64,
    likes: u64,
    comments: u64,
    shares: u64,
    preview: String,
}

impl PostStat {
    fn sum(posts: &[PostStat]) -> PostStat {
        posts.iter().fold(
            PostStat {
                views: 0,
                likes: 0,
                comments: 0,
                shares: 0,
                preview: String::new(),
            },
            |mut acc, p| {
                acc.views += p.views;
                acc.likes += p.likes;
                acc.comments += p.comments;
                acc.shares += p.shares;
                acc
            },
        )
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "views": self.views,
            "likes": self.likes,
            "comments": self.comments,
            "shares": self.shares,
            "preview": self.preview,
        })
    }
}

fn extract_post_stat(item: &Value) -> PostStat {
    let update = unwrap_update_v2(item);
    PostStat {
        views: social_count(update, "numViews"),
        likes: social_count(update, "numLikes"),
        comments: social_count(update, "numComments"),
        shares: social_count(update, "numShares"),
        preview: commentary_text(update).to_string(),
    }
}

// ---------------------------------------------------------------------------
// URN helpers + feed cache
// ---------------------------------------------------------------------------

/// Pull the activity URN out of a notification cardAction URL like
/// `/feed/update/urn:li:activity:7312345/?...` (URL- or percent-encoded).
pub fn extract_activity_urn_from_url(url: &str) -> Option<String> {
    let decoded = url.replace("%3A", ":").replace("%2F", "/");
    let prefix = "urn:li:activity:";
    let start = decoded.find(prefix)?;
    let tail = &decoded[start..];
    let end = tail
        .find(|c: char| !c.is_ascii_digit() && c != ':' && !c.is_ascii_alphabetic())
        .unwrap_or(tail.len());
    let urn = &tail[..end];
    if urn.len() > prefix.len() {
        Some(urn.to_string())
    } else {
        None
    }
}

fn feed_cache_path() -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir.join("linkedin").join("last_feed.json"))
}

fn save_feed_cache(value: &Value) -> Result<(), String> {
    let path = feed_cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json =
        serde_json::to_string(value).map_err(|e| format!("failed to serialize feed cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write feed cache: {e}"))?;
    Ok(())
}

fn load_feed_cache() -> Result<Value, String> {
    let path = feed_cache_path()?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| "no cached feed. Run `feed list` or `feed my-posts` first.".to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse feed cache: {e}"))
}

/// Normalize a user-supplied reactions URN. Accepts full URNs (any type) or a
/// bare activity ID (digits only), which is wrapped as `urn:li:activity:...`.
fn normalize_reactions_urn(input: &str) -> String {
    if input.starts_with("urn:li:") {
        input.to_string()
    } else {
        format!("urn:li:activity:{}", input)
    }
}

/// Extract the right thread URN for the reactions endpoint from a cached feed
/// element. ugcPost-backed posts require the ugcPost URN, share-backed posts
/// require the activity URN.
fn extract_reactions_urn(element: &Value) -> Option<String> {
    let update = unwrap_update_v2(element);
    let metadata = update.get("updateMetadata")?;
    let activity_urn = metadata.get("urn").and_then(|u| u.as_str());
    let share_urn = metadata.get("shareUrn").and_then(|u| u.as_str());

    match (share_urn, activity_urn) {
        (Some(s), _) if s.starts_with("urn:li:ugcPost:") => Some(s.to_string()),
        (_, Some(a)) => Some(a.to_string()),
        (Some(s), None) => Some(s.to_string()),
        (None, None) => None,
    }
}

fn reactions_urn_from_cache(index: usize) -> Result<String, String> {
    let element = cached_feed_element(index)?;
    extract_reactions_urn(&element).ok_or_else(|| {
        format!(
            "could not extract a reactions URN from cached item {}",
            index
        )
    })
}

/// Load a 1-based feed-cache element by index. Used by reactions + comments.
fn cached_feed_element(index: usize) -> Result<Value, String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }
    let cache = load_feed_cache()?;
    let elements = cache
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "cached feed has no elements array".to_string())?;
    elements.get(index - 1).cloned().ok_or_else(|| {
        format!(
            "index {} out of range (cached feed has {} items)",
            index,
            elements.len()
        )
    })
}

/// Extract the inner `urn:li:activity:XXXXX` from a feed element's entityUrn.
pub fn extract_activity_urn(feed_entity_urn: &str) -> Option<String> {
    if let Some(start) = feed_entity_urn.find("urn:li:activity:") {
        let rest = &feed_entity_urn[start..];
        let end = rest.find(')').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

fn resolve_post_urn(post_urn_or_index: &str) -> Result<String, String> {
    let Ok(index) = post_urn_or_index.parse::<usize>() else {
        return Ok(post_urn_or_index.to_string());
    };
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }
    let cache = load_feed_cache()?;
    let feed: FeedResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached feed: {e}"))?;
    let element = feed.elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (feed has {} items)",
            index,
            feed.elements.len()
        )
    })?;
    let entity_urn = element
        .get("entityUrn")
        .or_else(|| element.get("urn"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| "feed item has no entityUrn".to_string())?;
    extract_activity_urn(entity_urn)
        .ok_or_else(|| format!("could not extract activity URN from: {}", entity_urn))
}

// ---------------------------------------------------------------------------
// feed read / view (full view)
// ---------------------------------------------------------------------------

pub fn cmd_feed_read(index: usize, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }
    let cache = load_feed_cache()?;
    let feed: FeedResponse = serde_json::from_value(cache.clone())
        .map_err(|e| format!("failed to parse cached feed: {e}"))?;
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

pub async fn cmd_feed_view(activity_urn: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;
    let post = client
        .get_post(activity_urn)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;
    if raw_json {
        return print_json(&post);
    }
    print_feed_item_full(0, &post);
    Ok(())
}

/// Print full details of a single feed item (expanded view for `feed read`).
fn print_feed_item_full(index: usize, item: &Value) {
    let update = unwrap_update_v2(item);
    print_full_header(index, item, update);
    print_full_body(update);
    print_full_media(update);
    print_full_footer(item, update);
}

fn print_full_header(index: usize, _item: &Value, update: &Value) {
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

/// Find the reshared UpdateV2, checking each known location LinkedIn uses.
fn reshared_update(update: &Value) -> Option<&Value> {
    update
        .get("resharedUpdate")
        .or_else(|| update.get("content").and_then(|c| c.get("resharedUpdate")))
        .or_else(|| {
            update
                .get("content")
                .and_then(|c| c.get("com.linkedin.voyager.feed.render.UpdateV2"))
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

// ---------------------------------------------------------------------------
// Article + media extraction
// ---------------------------------------------------------------------------

struct ArticleInfo {
    title: String,
    url: String,
}

fn extract_article_info(update: &Value) -> Option<ArticleInfo> {
    let content = update.get("content")?;
    extract_article_from_component(content).or_else(|| extract_article_from_nav(content))
}

fn extract_article_from_component(content: &Value) -> Option<ArticleInfo> {
    let article = content
        .get("articleComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.ArticleComponent"))?;
    let title = article
        .get("title")
        .and_then(|t| {
            t.get("text")
                .and_then(|v| v.as_str())
                .or_else(|| t.as_str())
        })
        .unwrap_or("")
        .to_string();
    let url = article
        .get("navigationContext")
        .and_then(|n| n.get("actionTarget"))
        .and_then(|v| v.as_str())
        .or_else(|| article.get("url").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    if title.is_empty() && url.is_empty() {
        return None;
    }
    Some(ArticleInfo { title, url })
}

fn extract_article_from_nav(content: &Value) -> Option<ArticleInfo> {
    let nav = content.get("navigationContext")?;
    let url = field_str(nav, "actionTarget").to_string();
    if url.is_empty() {
        return None;
    }
    let title = field_str(nav, "accessibilityText").to_string();
    Some(ArticleInfo { title, url })
}

/// Component union keys that map directly to a media-type label.
const MEDIA_COMPONENT_LABELS: &[(&str, &str)] = &[
    ("com.linkedin.voyager.feed.render.ImageComponent", "image"),
    (
        "com.linkedin.voyager.feed.render.LinkedInVideoComponent",
        "video",
    ),
    (
        "com.linkedin.voyager.feed.render.DocumentComponent",
        "document",
    ),
    ("com.linkedin.voyager.feed.render.PollComponent", "poll"),
    (
        "com.linkedin.voyager.feed.render.ArticleComponent",
        "article",
    ),
    (
        "com.linkedin.voyager.feed.render.CelebrationComponent",
        "celebration",
    ),
    (
        "com.linkedin.voyager.feed.render.CarouselComponent",
        "carousel",
    ),
    ("imageComponent", "image"),
    ("videoComponent", "video"),
    ("documentComponent", "document"),
    ("pollComponent", "poll"),
    ("articleComponent", "article"),
    ("celebrationComponent", "celebration"),
    ("carouselComponent", "carousel"),
];

/// Substring tokens used as a fallback when the response only carries
/// `$type` rather than a typed component.
const MEDIA_TYPE_TOKENS: &[(&str, &str)] = &[
    ("Image", "image"),
    ("Video", "video"),
    ("Document", "document"),
    ("Poll", "poll"),
    ("Article", "article"),
];

fn extract_media_type_label(update: &Value) -> String {
    let Some(content) = update.get("content") else {
        return String::new();
    };
    label_from_components(content)
        .or_else(|| label_from_type_token(content))
        .unwrap_or_default()
}

fn label_from_components(content: &Value) -> Option<String> {
    MEDIA_COMPONENT_LABELS
        .iter()
        .find(|(key, _)| content.get(*key).is_some())
        .map(|(_, label)| (*label).to_string())
}

fn label_from_type_token(content: &Value) -> Option<String> {
    let type_str = content.get("$type").and_then(|t| t.as_str())?;
    MEDIA_TYPE_TOKENS
        .iter()
        .find(|(token, _)| type_str.contains(token))
        .map(|(_, label)| (*label).to_string())
}

/// Extract media URLs (images, videos, documents) from a feed item's content.
fn extract_media_urls(update: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    let Some(content) = update.get("content") else {
        return urls;
    };
    extract_image_urls_into(content, &mut urls);
    extract_video_urls_into(content, &mut urls);
    extract_document_urls_into(content, &mut urls);
    extract_carousel_urls_into(content, &mut urls);
    urls
}

fn extract_image_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(img) = component(content, "imageComponent", "ImageComponent") else {
        return;
    };
    collect_image_urls(img, urls);
}

fn extract_video_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(vid) = component(content, "videoComponent", "LinkedInVideoComponent") else {
        return;
    };
    if let Some(url) = first_video_stream_url(vid) {
        urls.push(url);
    }
    if let Some(media) = vid
        .get("videoPlayMetadata")
        .or_else(|| vid.get("videoPlay"))
        .and_then(|m| m.get("media"))
        .and_then(|v| v.as_str())
    {
        urls.push(media.to_string());
    }
    if urls.is_empty() {
        if let Some(thumb) = video_thumbnail_url(vid) {
            urls.push(format!("(thumbnail) {}", thumb));
        }
    }
}

fn first_video_stream_url(vid: &Value) -> Option<String> {
    let play_meta = vid
        .get("videoPlayMetadata")
        .or_else(|| vid.get("videoPlay"))?;
    let streams = play_meta
        .get("progressiveStreams")
        .and_then(|s| s.as_array())?;
    streams.iter().find_map(|stream| {
        stream
            .get("streamingLocations")
            .and_then(|sl| sl.as_array())
            .and_then(|arr| arr.first())
            .and_then(|loc| loc.get("url"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    })
}

fn video_thumbnail_url(vid: &Value) -> Option<&str> {
    let thumbnail = vid.get("thumbnail")?;
    thumbnail
        .get("url")
        .and_then(|v| v.as_str())
        .or_else(|| thumbnail.get("rootUrl").and_then(|v| v.as_str()))
}

fn extract_document_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(doc) = component(content, "documentComponent", "DocumentComponent") else {
        return;
    };
    let url = doc.get("document").and_then(|d| {
        d.get("transcribedDocumentUrl")
            .and_then(|v| v.as_str())
            .or_else(|| d.get("downloadUrl").and_then(|v| v.as_str()))
    });
    if let Some(u) = url {
        urls.push(u.to_string());
    }
}

fn extract_carousel_urls_into(content: &Value, urls: &mut Vec<String>) {
    let Some(carousel) = component(content, "carouselComponent", "CarouselComponent") else {
        return;
    };
    let Some(pages) = carousel.get("pages").and_then(|p| p.as_array()) else {
        return;
    };
    for page in pages.iter().take(5) {
        if let Some(img) = page.get("imageComponent") {
            collect_image_urls(img, urls);
        }
    }
}

/// Look up a feed content component by its short name, falling back to the
/// fully-qualified Rest.li union key.
fn component<'a>(content: &'a Value, short: &str, type_name: &str) -> Option<&'a Value> {
    content
        .get(short)
        .or_else(|| content.get(format!("com.linkedin.voyager.feed.render.{}", type_name).as_str()))
}

/// Collect image URLs from an image component into the urls vec.
fn collect_image_urls(img: &Value, urls: &mut Vec<String>) {
    if let Some(url) = first_image_attribute_url(img) {
        urls.push(url);
        return;
    }
    if let Some(url) = img.get("url").and_then(|v| v.as_str()) {
        urls.push(url.to_string());
    }
}

/// Walk `images[].attributes[]` and return the first imageUrl or
/// vectorImage-derived URL found.
fn first_image_attribute_url(img: &Value) -> Option<String> {
    let images = img.get("images").and_then(|i| i.as_array())?;
    images
        .iter()
        .filter_map(|image| image.get("attributes").and_then(|a| a.as_array()))
        .flatten()
        .find_map(image_attribute_url)
}

fn image_attribute_url(attr: &Value) -> Option<String> {
    if let Some(url) = attr.get("imageUrl").and_then(|v| v.as_str()) {
        return Some(url.to_string());
    }
    let vi = attr.get("vectorImage")?;
    let root = vi.get("rootUrl").and_then(|v| v.as_str())?;
    let segment = vi
        .get("artifacts")
        .and_then(|a| a.as_array())
        .and_then(|arr| arr.last())
        .and_then(|a| {
            a.get("fileIdentifyingUrlPathSegment")
                .and_then(|v| v.as_str())
        })
        .unwrap_or("");
    Some(format!("{}{}", root, segment))
}

// ---------------------------------------------------------------------------
// feed comments
// ---------------------------------------------------------------------------

/// Handle `feed comments <index> [--count N] [--json]`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_reactions_urn_passes_full_urn_through() {
        assert_eq!(
            normalize_reactions_urn("urn:li:activity:7450808005048094720"),
            "urn:li:activity:7450808005048094720"
        );
        assert_eq!(
            normalize_reactions_urn("urn:li:ugcPost:7450808001881534464"),
            "urn:li:ugcPost:7450808001881534464"
        );
    }

    #[test]
    fn extract_activity_urn_from_url_handles_plain_path() {
        assert_eq!(
            extract_activity_urn_from_url("/feed/update/urn:li:activity:7312345/"),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_from_url_handles_percent_encoded() {
        assert_eq!(
            extract_activity_urn_from_url(
                "/feed/update/urn%3Ali%3Aactivity%3A7312345/?trackingId=foo"
            ),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_from_url_returns_none_for_unrelated() {
        assert_eq!(extract_activity_urn_from_url("/in/some-profile/"), None);
    }

    #[test]
    fn normalize_reactions_urn_wraps_bare_id_as_activity() {
        assert_eq!(
            normalize_reactions_urn("7450808005048094720"),
            "urn:li:activity:7450808005048094720"
        );
    }

    /// Build a wrapped UpdateV2 element with the given activity + share URNs
    /// in `updateMetadata`. Mirrors the live `feed list` response shape.
    fn wrapped_metadata_element(activity_urn: &str, share_urn: &str) -> Value {
        json!({
            "value": {
                "com.linkedin.voyager.feed.render.UpdateV2": {
                    "updateMetadata": {
                        "urn": activity_urn,
                        "shareUrn": share_urn,
                    }
                }
            }
        })
    }

    #[test]
    fn extract_reactions_urn_picks_correct_urn_per_post_backing() {
        // (activity, share, expected) — exercises the URN-type-picky branch
        // of `extract_reactions_urn`. ugcPost shareUrn wins over activity;
        // share-backed posts fall through to the activity URN.
        let cases = [
            (
                "urn:li:activity:7450891298279972864",
                "urn:li:ugcPost:7450888187100639232",
                "urn:li:ugcPost:7450888187100639232",
            ),
            (
                "urn:li:activity:7450895500045598720",
                "urn:li:share:7450895499496202240",
                "urn:li:activity:7450895500045598720",
            ),
        ];
        for (activity, share, expected) in cases {
            let element = wrapped_metadata_element(activity, share);
            assert_eq!(
                extract_reactions_urn(&element),
                Some(expected.to_string()),
                "activity={activity} share={share}"
            );
        }
    }

    #[test]
    fn extract_reactions_urn_handles_unwrapped_update_element() {
        let element = json!({
            "updateMetadata": {
                "urn": "urn:li:activity:7450808005048094720",
                "shareUrn": "urn:li:ugcPost:7450808001881534464"
            }
        });
        assert_eq!(
            extract_reactions_urn(&element),
            Some("urn:li:ugcPost:7450808001881534464".to_string())
        );
    }

    #[test]
    fn extract_reactions_urn_returns_none_when_metadata_missing() {
        let element = json!({"foo": "bar"});
        assert_eq!(extract_reactions_urn(&element), None);
    }
}
