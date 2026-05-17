//! `feed stats` command.

use serde_json::Value;

use crate::error::CliResult;
use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

use super::helpers::{commentary_text, print_json, social_count, unwrap_update_v2};

pub async fn cmd_feed_stats(raw_json: bool) -> CliResult<()> {
    const POST_COUNT: u32 = 20;

    let (client, _path) = load_session_client()?;
    let value = client.get_my_posts(0, POST_COUNT).await?;

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

fn print_stats_json(posts: &[PostStat], totals: &PostStat, n: u64) -> CliResult<()> {
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
