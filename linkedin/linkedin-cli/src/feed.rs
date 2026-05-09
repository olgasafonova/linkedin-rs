use linkedin_api::models::FeedResponse;

use crate::session::load_session_client;
use crate::util::{print_paging_header, truncate_with_ellipsis};

/// Handle `feed list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/feed/updates?q=findFeed with
/// pagination params, and prints the results.
pub async fn cmd_feed_list(
    start: u32,
    count: u32,
    author_filter: Option<&str>,
    keyword_filter: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_feed(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    // Cache the raw feed response for `feed read` / `feed react` / `feed comment` by index.
    if let Err(e) = save_feed_cache(&value) {
        eprintln!("warning: failed to cache feed: {e}");
    }

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let feed: FeedResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse feed response: {e}"))?;

    print_feed_list_header(&feed, author_filter, keyword_filter);

    if feed.elements.is_empty() {
        println!("(no feed items)");
        return Ok(());
    }

    let author_lower = author_filter.map(str::to_lowercase);
    let keyword_lower = keyword_filter.map(str::to_lowercase);

    let mut shown = 0;
    for (i, element) in feed.elements.iter().enumerate() {
        if !feed_item_passes_filters(element, author_lower.as_deref(), keyword_lower.as_deref()) {
            continue;
        }
        shown += 1;
        print_feed_item(start as usize + i + 1, element);
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

/// Strip the optional `value.com.linkedin.voyager.feed.render.UpdateV2`
/// wrapper that wraps feed elements in the live response shape.
fn unwrap_update_v2(element: &serde_json::Value) -> &serde_json::Value {
    element
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(element)
}

fn feed_item_passes_filters(
    element: &serde_json::Value,
    author_lower: Option<&str>,
    keyword_lower: Option<&str>,
) -> bool {
    if author_lower.is_none() && keyword_lower.is_none() {
        return true;
    }
    let update = unwrap_update_v2(element);
    if let Some(q) = author_lower {
        let actor = update
            .get("actor")
            .and_then(|a| a.get("name"))
            .and_then(|n| n.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if !actor.to_lowercase().contains(q) {
            return false;
        }
    }
    if let Some(q) = keyword_lower {
        let commentary = update
            .get("commentary")
            .and_then(|c| c.get("text"))
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if !commentary.to_lowercase().contains(q) {
            return false;
        }
    }
    true
}

/// Print a brief human-readable summary of a single feed item.
///
/// Feed items are `UpdateV2` records. We extract what we can and skip
/// unknown fields gracefully. The real structure is deeply nested, so
/// this is best-effort until we've validated against live data.
fn print_feed_item(index: usize, item: &serde_json::Value) {
    // The real feed response wraps the UpdateV2 payload inside:
    //   element.value["com.linkedin.voyager.feed.render.UpdateV2"]
    // This is LinkedIn's Rest.li union encoding. Unwrap it first,
    // falling back to the element itself for forward-compatibility.
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    // Try to extract actor name.
    let actor_name = update
        .get("actor")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(unknown author)");

    // Try to extract commentary text.
    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Truncate long commentary for the summary view.
    let commentary_display = truncate_with_ellipsis(commentary, 120);

    // Entity URN -- lives at the top-level element, not inside the UpdateV2.
    let urn = item.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");

    // Social counts are inside the UpdateV2 payload.
    let likes = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numLikes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let comments = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numComments"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let media_label = extract_media_type_label(update);

    println!(
        "[{}] {} {}",
        index,
        actor_name,
        if !urn.is_empty() { urn } else { "" }
    );
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

/// Handle `feed my-posts [--start N] [--count N] [--json]`.
///
/// Fetches the authenticated user's own posts with engagement metrics.
/// Uses `identity/profileUpdatesV2?q=memberShareFeed` which returns
/// standard UpdateV2 records with full social detail.
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
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
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
        let idx = start as usize + i + 1;
        print_my_post(idx, element);
        println!();
    }

    Ok(())
}

/// Print a human-readable summary of one of the user's own posts.
///
/// Same UpdateV2 format as general feed items, but we also show view count
/// since it's available for own posts.
fn print_my_post(index: usize, item: &serde_json::Value) {
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    // Commentary text.
    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let commentary_display = truncate_with_ellipsis(commentary, 120);

    // Activity URN -- extract from the composite entityUrn.
    // The profileUpdatesV2 response uses MEMBER_SHARES context, so we need to
    // trim at the first comma to get just the activity URN.
    let entity_urn = item.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");
    let activity_urn = extract_activity_urn(entity_urn)
        .map(|u| u.split(',').next().unwrap_or(&u).to_string())
        .unwrap_or_default();

    // Social counts.
    let social = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"));

    let likes = social
        .and_then(|c| c.get("numLikes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let comments = social
        .and_then(|c| c.get("numComments"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let shares = social
        .and_then(|c| c.get("numShares"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let views = social
        .and_then(|c| c.get("numViews"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    // Timestamp from actor subDescription (e.g. "3d • ").
    let time_desc = update
        .get("actor")
        .and_then(|a| a.get("subDescription"))
        .and_then(|s| s.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .trim_end_matches("• \u{a0}\u{a0}")
        .trim_end_matches("•")
        .trim();

    println!(
        "[{}] {}",
        index,
        if !activity_urn.is_empty() {
            &activity_urn
        } else {
            entity_urn
        }
    );
    if !time_desc.is_empty() {
        println!("    posted: {}", time_desc);
    }
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    println!(
        "    views: {}  reactions: {}  comments: {}  reposts: {}",
        views, likes, comments, shares
    );

    // Reaction type breakdown (if available).
    if let Some(rtc) = social
        .and_then(|c| c.get("reactionTypeCounts"))
        .and_then(|r| r.as_array())
    {
        if !rtc.is_empty() {
            let parts: Vec<String> = rtc
                .iter()
                .filter_map(|r| {
                    let rtype = r.get("reactionType").and_then(|t| t.as_str())?;
                    let count = r.get("count").and_then(|c| c.as_u64())?;
                    Some(format!("{}: {}", reaction_emoji(rtype), count))
                })
                .collect();
            println!("    {}", parts.join("  "));
        }
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

/// Handle `feed reactions <post_urn> [--start N] [--count N] [--json]`.
///
/// Fetches and displays the list of people who reacted to a specific post.
pub async fn cmd_feed_reactions(
    post_urn: Option<&str>,
    from_list: Option<usize>,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let urn = match (post_urn, from_list) {
        (Some(u), None) => normalize_reactions_urn(u),
        (None, Some(index)) => reactions_urn_from_cache(index)?,
        (None, None) => {
            return Err(
                "provide a post URN or use --from-list N after `feed list`/`feed my-posts`"
                    .to_string(),
            )
        }
        (Some(_), Some(_)) => unreachable!("clap conflicts_with guards this"),
    };

    let (client, _path) = load_session_client()?;
    let value = client
        .get_post_reactions(&urn, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let total = value
        .get("paging")
        .and_then(|p| p.get("total"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    println!("Reactions on {} ({} total)", urn, total);
    println!("---");

    if elements.is_empty() {
        println!("(no reactions)");
        if from_list.is_none() && urn.starts_with("urn:li:activity:") {
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
        return Ok(());
    }

    for element in &elements {
        let lockup = element.get("reactorLockup").unwrap_or(element);
        let name = lockup
            .get("title")
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("(unknown)");
        let subtitle = lockup
            .get("subtitle")
            .and_then(|s| s.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        let rtype = element
            .get("reactionType")
            .and_then(|r| r.as_str())
            .unwrap_or("?");

        let subtitle_display = truncate_with_ellipsis(subtitle, 60);

        println!(
            "  {:12} {} {}",
            reaction_emoji(rtype),
            name,
            if subtitle_display.is_empty() {
                String::new()
            } else {
                format!("- {}", subtitle_display)
            }
        );
    }

    Ok(())
}

/// Handle `feed react <post_urn> [--type LIKE] [--json]`.
///
/// Reacts to a feed post with the specified reaction type.
/// Reaction type validation is handled by the API layer.
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
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Reacted with {} to {}", rt_upper, resolved_urn);
    }

    Ok(())
}

/// Handle `feed unreact <post_urn> [--type LIKE] [--json]`.
///
/// Removes a reaction from a feed post.
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
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Removed {} reaction from {}", rt_upper, resolved_urn);
    }

    Ok(())
}

/// Handle `feed comment <post_urn> <text> [--yes] [--json]`.
///
/// Creates a comment on a feed post. Requires `--yes` to confirm,
/// since this creates a REAL COMMENT on a LinkedIn post.
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
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Commented on {}", resolved_urn);
    }

    Ok(())
}

/// Handle `feed stats [--json]`.
///
/// Aggregates engagement across the user's last 20 posts by summing the
/// per-post counts embedded in `socialDetail.totalSocialActivityCounts` on
/// the `identity/profileUpdatesV2?q=memberShareFeed` response.
///
/// The legacy `identity/socialUpdateAnalytics{,Header}` endpoints return
/// HTTP 400 and are not used -- see `re/my_posts.md` for details.
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
        .cloned()
        .unwrap_or_default();

    let posts: Vec<PostStat> = elements.iter().map(extract_post_stat).collect();
    let totals = PostStat::sum(&posts);
    let n = posts.len() as u64;

    if raw_json {
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
        let pretty =
            serde_json::to_string_pretty(&out).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    if n == 0 {
        println!("(no posts found)");
        return Ok(());
    }

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

    // Show top 5 posts by views as context.
    let mut ranked = posts.clone();
    ranked.sort_by_key(|p| std::cmp::Reverse(p.views));
    let top = ranked.iter().take(5);
    println!("Top posts by views:");
    for (i, p) in top.enumerate() {
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

    Ok(())
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

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "views": self.views,
            "likes": self.likes,
            "comments": self.comments,
            "shares": self.shares,
            "preview": self.preview,
        })
    }
}

fn extract_post_stat(item: &serde_json::Value) -> PostStat {
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    let counts = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"));

    let get_u64 = |key: &str| -> u64 {
        counts
            .and_then(|c| c.get(key))
            .and_then(|n| n.as_u64())
            .unwrap_or(0)
    };

    let preview = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    PostStat {
        views: get_u64("numViews"),
        likes: get_u64("numLikes"),
        comments: get_u64("numComments"),
        shares: get_u64("numShares"),
        preview,
    }
}

/// Handle `feed post <text> [--visibility ANYONE] [--yes] [--json]`.
///
/// Creates a new text-only post on the authenticated user's LinkedIn feed.
/// Requires `--yes` to confirm, since this creates a REAL PUBLIC post.
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
        // Show what would be posted and require confirmation.
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
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        // Try to extract the share URN from the response.
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

/// Returns the path to the feed cache file.
fn feed_cache_path() -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir.join("linkedin").join("last_feed.json"))
}

/// Save raw feed JSON to cache for `feed read` / index-based react/comment.
fn save_feed_cache(value: &serde_json::Value) -> Result<(), String> {
    let path = feed_cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json =
        serde_json::to_string(value).map_err(|e| format!("failed to serialize feed cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write feed cache: {e}"))?;
    Ok(())
}

/// Load cached feed JSON. Returns an error if no cache exists.
fn load_feed_cache() -> Result<serde_json::Value, String> {
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
/// element. LinkedIn's `socialDashReactionsByReactionType` is URN-type-picky:
/// ugcPost-backed posts require the ugcPost URN, share-backed posts require
/// the activity URN. `updateMetadata.shareUrn` carries the underlying object
/// URN, so we use that when it's a `ugcPost:`; otherwise we fall back to the
/// activity URN. See `re/reactions.md` for the investigation.
fn extract_reactions_urn(element: &serde_json::Value) -> Option<String> {
    let update = element
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(element);
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

/// Resolve a 1-based index from the last cached feed listing into a reactions
/// thread URN.
fn reactions_urn_from_cache(index: usize) -> Result<String, String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }
    let cache = load_feed_cache()?;
    let elements = cache
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "cached feed has no elements array".to_string())?;
    let element = elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (cached feed has {} items)",
            index,
            elements.len()
        )
    })?;
    extract_reactions_urn(element).ok_or_else(|| {
        format!(
            "could not extract a reactions URN from cached item {}",
            index
        )
    })
}

/// Extract the inner `urn:li:activity:XXXXX` from a feed element's entityUrn.
///
/// Feed entityUrns have formats like:
///   `urn:li:fs_feedUpdate:(V2&FOLLOW_FEED,urn:li:activity:7312345678901234567)`
///   `urn:li:activity:7312345678901234567`
pub fn extract_activity_urn(feed_entity_urn: &str) -> Option<String> {
    if let Some(start) = feed_entity_urn.find("urn:li:activity:") {
        let rest = &feed_entity_urn[start..];
        let end = rest.find(')').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

/// Resolve a post URN from either a literal URN string or a 1-based feed index.
///
/// If `post_urn_or_index` parses as a positive integer, loads the feed cache
/// and extracts the activity URN. Otherwise returns the string as-is.
fn resolve_post_urn(post_urn_or_index: &str) -> Result<String, String> {
    if let Ok(index) = post_urn_or_index.parse::<usize>() {
        if index == 0 {
            return Err("index must be >= 1".to_string());
        }
        let cache = load_feed_cache()?;
        let feed: FeedResponse = serde_json::from_value(cache)
            .map_err(|e| format!("failed to parse cached feed: {e}"))?;
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
    } else {
        Ok(post_urn_or_index.to_string())
    }
}

/// Handle `feed read <index> [--json]`.
///
/// Shows full post details for item N from the last `feed list`.
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
        let pretty =
            serde_json::to_string_pretty(element).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    print_feed_item_full(index, element);
    Ok(())
}

/// Handle `feed view <activity_urn> [--json]`.
///
/// Fetches a single post by activity URN from the API and displays it.
pub async fn cmd_feed_view(activity_urn: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let post = client
        .get_post(activity_urn)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&post).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    print_feed_item_full(0, &post);
    Ok(())
}

/// Print full details of a single feed item (expanded view for `feed read`).
fn print_feed_item_full(index: usize, item: &serde_json::Value) {
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    let actor_name = update
        .get("actor")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(unknown author)");

    let actor_description = update
        .get("actor")
        .and_then(|a| a.get("description"))
        .and_then(|d| d.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(no text)");

    let likes = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numLikes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let comments = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numComments"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let shares = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numShares"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let entity_urn = item.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");
    let permalink = item.get("permalink").and_then(|u| u.as_str()).unwrap_or("");
    let activity_urn = extract_activity_urn(entity_urn).unwrap_or_default();

    println!("[{}] {}", index, actor_name);
    if !actor_description.is_empty() {
        println!("    {}", actor_description);
    }
    println!();
    println!("{}", commentary);

    // Reshared post content: LinkedIn puts reshared content in several locations.
    let reshared_update = update
        .get("resharedUpdate")
        .or_else(|| update.get("content").and_then(|c| c.get("resharedUpdate")))
        .or_else(|| {
            // Also check inside the UpdateV2 union value
            update
                .get("content")
                .and_then(|c| c.get("com.linkedin.voyager.feed.render.UpdateV2"))
        });
    if let Some(reshared) = reshared_update {
        let orig_author = reshared
            .get("actor")
            .and_then(|a| a.get("name"))
            .and_then(|n| n.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("(unknown)");
        let orig_text = reshared
            .get("commentary")
            .and_then(|c| c.get("text"))
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        println!();
        println!("  [reshared from {}]", orig_author);
        if !orig_text.is_empty() {
            println!("  {}", truncate_with_ellipsis(orig_text, 300));
        }
    }

    // Article links: extract from content component.
    if let Some(article) = extract_article_info(update) {
        println!();
        if !article.title.is_empty() {
            println!("Article: {}", article.title);
        }
        if !article.url.is_empty() {
            println!("  {}", article.url);
        }
    }

    // Media type labels + URLs.
    let media_label = extract_media_type_label(update);
    if !media_label.is_empty() {
        println!("Media: {}", media_label);
    }
    let media_urls = extract_media_urls(update);
    for url in &media_urls {
        println!("  {}", url);
    }

    println!();
    println!(
        "likes: {}  comments: {}  shares: {}",
        likes, comments, shares
    );
    if !activity_urn.is_empty() {
        println!("URN: {}", activity_urn);
    }
    if !permalink.is_empty() {
        println!("Link: {}", permalink);
    }
}

/// Article info extracted from a feed item's content component.
struct ArticleInfo {
    title: String,
    url: String,
}

/// Extract article title and URL from a feed item's content component.
///
/// LinkedIn wraps article content in several possible locations:
/// - `content.articleComponent` (standard articles)
/// - `content.navigationContext` (link previews)
/// - `content["com.linkedin.voyager.feed.render.ArticleComponent"]` (Rest.li union)
fn extract_article_info(update: &serde_json::Value) -> Option<ArticleInfo> {
    let content = update.get("content")?;

    // Try articleComponent first (most common for shared articles).
    if let Some(article) = content
        .get("articleComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.ArticleComponent"))
    {
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
        if !title.is_empty() || !url.is_empty() {
            return Some(ArticleInfo { title, url });
        }
    }

    // Try top-level navigationContext on the content node.
    if let Some(nav) = content.get("navigationContext") {
        let title = nav
            .get("accessibilityText")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let url = nav
            .get("actionTarget")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !url.is_empty() {
            return Some(ArticleInfo { title, url });
        }
    }

    None
}

/// Determine the media type label for a feed item's content.
///
/// Returns a label like "image", "video", "document", "poll", "article",
/// or an empty string if no media is detected.
fn extract_media_type_label(update: &serde_json::Value) -> String {
    let content = match update.get("content") {
        Some(c) => c,
        None => return String::new(),
    };

    // Check for specific component types (Rest.li union keys or direct fields).
    let type_checks: &[(&str, &str)] = &[
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

    for (key, label) in type_checks {
        if content.get(*key).is_some() {
            return label.to_string();
        }
    }

    // Check for $type field (some responses use this).
    if let Some(type_str) = content.get("$type").and_then(|t| t.as_str()) {
        if type_str.contains("Image") {
            return "image".to_string();
        }
        if type_str.contains("Video") {
            return "video".to_string();
        }
        if type_str.contains("Document") {
            return "document".to_string();
        }
        if type_str.contains("Poll") {
            return "poll".to_string();
        }
        if type_str.contains("Article") {
            return "article".to_string();
        }
    }

    String::new()
}

/// Extract media URLs (images, videos, documents) from a feed item's content.
///
/// LinkedIn stores media URLs in various nested locations depending on type.
/// Returns a vec of URL strings found.
fn extract_media_urls(update: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();
    let content = match update.get("content") {
        Some(c) => c,
        None => return urls,
    };

    // Image URLs: look in imageComponent or the union variant.
    let image_comp = content
        .get("imageComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.ImageComponent"));
    if let Some(img) = image_comp {
        // Images are in images[].attributes[].imageUrl or
        // images[].attributes[].vectorImage.rootUrl + artifacts[].fileIdentifyingUrlPathSegment
        collect_image_urls(img, &mut urls);
    }

    // Video URLs: look in videoComponent or the union variant.
    let video_comp = content
        .get("videoComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.LinkedInVideoComponent"));
    if let Some(vid) = video_comp {
        // progressiveStreams[].streamingLocations[].url or videoPlayMetadata.progressiveStreams
        if let Some(play_meta) = vid
            .get("videoPlayMetadata")
            .or_else(|| vid.get("videoPlay"))
        {
            if let Some(streams) = play_meta
                .get("progressiveStreams")
                .and_then(|s| s.as_array())
            {
                for stream in streams {
                    if let Some(url) = stream
                        .get("streamingLocations")
                        .and_then(|sl| sl.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|loc| loc.get("url"))
                        .and_then(|v| v.as_str())
                    {
                        urls.push(url.to_string());
                        break; // One stream URL is enough.
                    }
                }
            }
            // Also try mediaUrl directly.
            if let Some(url) = play_meta.get("media").and_then(|v| v.as_str()) {
                urls.push(url.to_string());
            }
        }
        // Try thumbnail/poster.
        if let Some(poster) = vid.get("thumbnail").and_then(|t| {
            t.get("url")
                .and_then(|v| v.as_str())
                .or_else(|| t.get("rootUrl").and_then(|v| v.as_str()))
        }) {
            if urls.is_empty() {
                urls.push(format!("(thumbnail) {}", poster));
            }
        }
    }

    // Document URLs: look in documentComponent or the union variant.
    let doc_comp = content
        .get("documentComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.DocumentComponent"));
    if let Some(doc) = doc_comp {
        if let Some(url) = doc
            .get("document")
            .and_then(|d| d.get("transcribedDocumentUrl").and_then(|v| v.as_str()))
            .or_else(|| {
                doc.get("document")
                    .and_then(|d| d.get("downloadUrl").and_then(|v| v.as_str()))
            })
        {
            urls.push(url.to_string());
        }
    }

    // Carousel images.
    let carousel_comp = content
        .get("carouselComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.CarouselComponent"));
    if let Some(carousel) = carousel_comp {
        if let Some(pages) = carousel.get("pages").and_then(|p| p.as_array()) {
            for page in pages.iter().take(5) {
                // Each page may have an imageComponent.
                if let Some(img) = page.get("imageComponent") {
                    collect_image_urls(img, &mut urls);
                }
            }
        }
    }

    urls
}

/// Collect image URLs from an image component into the urls vec.
fn collect_image_urls(img: &serde_json::Value, urls: &mut Vec<String>) {
    // Try images[].attributes[].imageUrl first.
    if let Some(images) = img.get("images").and_then(|i| i.as_array()) {
        for image in images {
            if let Some(attrs) = image.get("attributes").and_then(|a| a.as_array()) {
                for attr in attrs {
                    if let Some(url) = attr.get("imageUrl").and_then(|v| v.as_str()) {
                        urls.push(url.to_string());
                        return;
                    }
                    // Try vectorImage: rootUrl + largest artifact.
                    if let Some(vi) = attr.get("vectorImage") {
                        if let Some(root) = vi.get("rootUrl").and_then(|v| v.as_str()) {
                            let segment = vi
                                .get("artifacts")
                                .and_then(|a| a.as_array())
                                .and_then(|arr| arr.last())
                                .and_then(|a| {
                                    a.get("fileIdentifyingUrlPathSegment")
                                        .and_then(|v| v.as_str())
                                })
                                .unwrap_or("");
                            urls.push(format!("{}{}", root, segment));
                            return;
                        }
                    }
                }
            }
        }
    }
    // Fallback: try a direct url field.
    if let Some(url) = img.get("url").and_then(|v| v.as_str()) {
        urls.push(url.to_string());
    }
}

/// Handle `feed comments <index> [--count N] [--json]`.
///
/// Fetches comments on a post by index from the cached feed.
pub async fn cmd_feed_comments(index: usize, count: u32, raw_json: bool) -> Result<(), String> {
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

    // Extract the socialDetail URN needed for the comments API.
    let update = element
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(element);

    let social_detail_urn = update
        .get("socialDetail")
        .and_then(|sd| sd.get("dashEntityUrn").or_else(|| sd.get("entityUrn")))
        .and_then(|u| u.as_str())
        .ok_or_else(|| "feed item has no socialDetail URN".to_string())?;

    let (client, _path) = load_session_client()?;

    let value = client
        .get_comments(social_detail_urn, 0, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
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
        let commenter = comment
            .get("commenter")
            .and_then(|c| {
                // Try title.text first, then fall back to accessibilityText
                c.get("title")
                    .and_then(|t| t.get("text"))
                    .and_then(|v| v.as_str())
                    .or_else(|| c.get("accessibilityText").and_then(|v| v.as_str()))
            })
            .unwrap_or("(unknown)");

        // Commentary text is directly at commentary.text (not nested)
        let text = comment
            .get("commentary")
            .and_then(|c| {
                c.get("text").and_then(|t| {
                    // Could be a string directly or a nested {text: "..."} object
                    t.as_str()
                        .or_else(|| t.get("text").and_then(|v| v.as_str()))
                })
            })
            .unwrap_or("");

        let likes = comment
            .get("socialDetail")
            .and_then(|sd| sd.get("totalSocialActivityCounts"))
            .and_then(|c| c.get("numLikes"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0);

        println!("[{}] {}", i + 1, commenter);
        println!("    {}", text);
        if likes > 0 {
            println!("    likes: {}", likes);
        }
        println!();
    }

    Ok(())
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

    #[test]
    fn extract_reactions_urn_prefers_ugcpost_sharureurn() {
        // UGC-backed post (e.g. an own text post): reactions endpoint
        // requires the ugcPost URN, not the activity URN.
        let element = json!({
            "value": {
                "com.linkedin.voyager.feed.render.UpdateV2": {
                    "updateMetadata": {
                        "urn": "urn:li:activity:7450891298279972864",
                        "shareUrn": "urn:li:ugcPost:7450888187100639232"
                    }
                }
            }
        });
        assert_eq!(
            extract_reactions_urn(&element),
            Some("urn:li:ugcPost:7450888187100639232".to_string())
        );
    }

    #[test]
    fn extract_reactions_urn_uses_activity_for_share_backed() {
        // Share-backed post (reshare): reactions endpoint requires the
        // activity URN; passing the share URN silently returns total=0.
        let element = json!({
            "value": {
                "com.linkedin.voyager.feed.render.UpdateV2": {
                    "updateMetadata": {
                        "urn": "urn:li:activity:7450895500045598720",
                        "shareUrn": "urn:li:share:7450895499496202240"
                    }
                }
            }
        });
        assert_eq!(
            extract_reactions_urn(&element),
            Some("urn:li:activity:7450895500045598720".to_string())
        );
    }

    #[test]
    fn extract_reactions_urn_handles_unwrapped_update_element() {
        // `feed my-posts` returns elements without the `value.UpdateV2`
        // wrapper — the updateMetadata sits at the root.
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
