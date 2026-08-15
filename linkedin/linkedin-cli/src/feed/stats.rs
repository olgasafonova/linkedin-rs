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
            "engagement_rate_pct": totals.engagement_rate().map(round2),
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
    println!(
        "  engagement rate: {}",
        format_rate(totals.engagement_rate())
    );
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
            "[{}] {} views, {} likes, {} comments, {} shares ({} engagement)",
            i + 1,
            p.views,
            p.likes,
            p.comments,
            p.shares,
            format_rate(p.engagement_rate())
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
    /// Total interactions: likes + comments + shares. Views are reach, not
    /// an interaction, so they are excluded.
    fn interactions(&self) -> u64 {
        self.likes + self.comments + self.shares
    }

    /// Engagement rate as a percentage of views (`interactions / views *
    /// 100`). `None` when views is zero, so the caller can render "n/a"
    /// rather than divide by zero or imply a real 0% engagement.
    fn engagement_rate(&self) -> Option<f64> {
        (self.views > 0).then(|| self.interactions() as f64 / self.views as f64 * 100.0)
    }

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
            "engagement_rate_pct": self.engagement_rate().map(round2),
            "preview": self.preview,
        })
    }
}

/// Round a percentage to two decimals for stable display and JSON output.
fn round2(pct: f64) -> f64 {
    (pct * 100.0).round() / 100.0
}

/// Render an optional engagement rate as `"3.42%"`, or `"n/a"` when views
/// were zero.
fn format_rate(rate: Option<f64>) -> String {
    match rate {
        Some(pct) => format!("{:.2}%", pct),
        None => "n/a".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(views: u64, likes: u64, comments: u64, shares: u64) -> PostStat {
        PostStat {
            views,
            likes,
            comments,
            shares,
            preview: String::new(),
        }
    }

    #[test]
    fn engagement_rate_is_interactions_over_views() {
        // 3 + 5 + 2 = 10 interactions over 200 views = 5%.
        let rate = stat(200, 3, 5, 2).engagement_rate().unwrap();
        assert!((rate - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn engagement_rate_is_none_when_no_views() {
        assert_eq!(stat(0, 4, 1, 0).engagement_rate(), None);
        assert_eq!(format_rate(None), "n/a");
    }

    #[test]
    fn interactions_exclude_views() {
        assert_eq!(stat(999, 1, 2, 3).interactions(), 6);
    }

    #[test]
    fn format_rate_renders_two_decimals() {
        assert_eq!(format_rate(Some(3.333)), "3.33%");
        assert_eq!(format_rate(Some(5.0)), "5.00%");
    }

    #[test]
    fn round2_stabilizes_display_precision() {
        assert_eq!(round2(3.3349), 3.33);
        assert_eq!(round2(3.3361), 3.34);
    }
}
