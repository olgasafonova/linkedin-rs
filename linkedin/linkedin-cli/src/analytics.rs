//! Analytics commands: content, post, profile-viewers, search-appearances.

use serde_json::Value;

use crate::error::CliResult;
use crate::session::load_session_client;

fn print_json(value: &Value) -> CliResult<()> {
    let pretty = serde_json::to_string_pretty(value)?;
    println!("{}", pretty);
    Ok(())
}

/// Show your recent post analytics with engagement metrics.
///
/// Uses `get_feed_analytics` and `get_my_posts` from the API layer.
pub async fn cmd_analytics_content(count: u32, days: u32, raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    eprintln!(
        "Fetching analytics for {} posts (last {} days)...",
        count, days
    );
    let posts = client.get_my_posts(0, count).await?;

    if raw_json {
        print_json(&posts)?;
        return Ok(());
    }

    let elements = posts
        .get("included")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut num_posts = 0u32;
    let mut total_impressions = 0u64;
    let mut total_reactions = 0u64;
    let mut total_comments = 0u64;
    let mut total_reposts = 0u64;

    for elem in &elements {
        let _urn = elem.get("urn").and_then(|v| v.as_str());
        num_posts += 1;
    }

    let mut shown = 0u32;
    for elem in &elements {
        if let Some(urn) = elem.get("urn").and_then(|v| v.as_str()) {
            if urn.contains(":activity:") || urn.contains(":share:") {
                if shown >= count {
                    break;
                }
                let text = elem
                    .get("value")
                    .and_then(|v| v.get("comment"))
                    .and_then(|v| v.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no text)");
                let social = elem
                    .get("value")
                    .and_then(|v| v.get("com.linkedin.voyager.feed.render.FeedUpdate"))
                    .and_then(|v| v.get("updateMetadata"))
                    .and_then(|v| v.get("socialDetail"));
                let reactions = social
                    .as_ref()
                    .and_then(|s| s.get("numLikes"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let comments = social
                    .as_ref()
                    .and_then(|s| s.get("numComments"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let reposts = social
                    .as_ref()
                    .and_then(|s| s.get("numShares"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                total_reactions += reactions;
                total_comments += comments;
                total_reposts += reposts;
                shown += 1;

                let preview = if text.len() > 80 {
                    format!("{}...", &text[..80])
                } else {
                    text.to_string()
                };
                println!(
                    "[{}] ❤️ {}  💬 {}  🔁 {}  | {}",
                    shown, reactions, comments, reposts, preview
                );
            }
        }
    }

    if shown > 0 {
        println!();
        println!(
            "Totals: {} posts | ❤️ {}  💬 {}  🔁 {}",
            shown, total_reactions, total_comments, total_reposts
        );
    } else {
        println!("No posts found in the specified range.");
    }

    Ok(())
}

/// Show analytics for a single post.
pub async fn cmd_analytics_post(
    post_urn: Option<&str>,
    days: u32,
    raw_json: bool,
) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    let urn = match post_urn {
        Some(u) => u.to_string(),
        None => {
            // Fetch latest post and use its URN
            eprintln!("No post URN provided, fetching latest post...");
            let posts = client.get_my_posts(0, 1).await?;
            let elements = posts
                .get("included")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let mut found_urn = None;
            for elem in &elements {
                if let Some(urn) = elem.get("urn").and_then(|v| v.as_str()) {
                    if urn.contains(":activity:") || urn.contains(":share:") {
                        found_urn = Some(urn.to_string());
                        break;
                    }
                }
            }
            found_urn.ok_or_else(|| {
                crate::error::CliError::Other("no posts found to analyze".to_string())
            })?
        }
    };

    eprintln!("Fetching analytics for {} (last {} days)...", urn, days);
    let posts = client.get_my_posts(0, 10).await?;

    if raw_json {
        print_json(&posts)?;
    } else {
        println!("Post: {}", urn);
        let elements = posts
            .get("included")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for elem in &elements {
            if let Some(elem_urn) = elem.get("urn").and_then(|v| v.as_str()) {
                if elem_urn == urn || elem_urn.contains(&urn.replace("urn:li:", "")) {
                    if let Some(social) = elem.get("value").and_then(|v| v.get("socialDetail")) {
                        let reactions =
                            social.get("numLikes").and_then(|v| v.as_u64()).unwrap_or(0);
                        let comments = social
                            .get("numComments")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let reposts = social
                            .get("numShares")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        println!("  Reactions: {}", reactions);
                        println!("  Comments: {}", comments);
                        println!("  Reposts: {}", reposts);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Show profile viewers for a time period.
pub async fn cmd_analytics_profile_viewers(
    days: u32,
    interesting_viewers: bool,
    raw_json: bool,
) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    eprintln!("Fetching profile viewers (last {} days)...", days);
    let viewers = client
        .get_profile_viewers_for_period(days, interesting_viewers)
        .await?;

    if raw_json {
        print_json(&viewers)?;
    } else {
        let elements = viewers
            .get("included")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut count = 0u32;
        for elem in &elements {
            if let Some(name) = elem
                .get("title")
                .and_then(|v| v.get("text"))
                .and_then(|v| v.as_str())
            {
                count += 1;
                let headline = elem
                    .get("value")
                    .and_then(|v| v.get("headline"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                println!("{}. {} — {}", count, name, headline);
            }
        }

        if count == 0 {
            println!("No profile viewers found for the last {} days.", days);
        } else {
            println!();
            println!("{} profile viewer(s) in the last {} days.", count, days);
        }
    }

    Ok(())
}

/// Show search appearance analytics.
pub async fn cmd_analytics_search_appearances(raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    eprintln!("Fetching search appearance data...");
    let data = client.get_search_appearances().await?;

    if raw_json {
        print_json(&data)?;
    } else {
        println!("Search Appearances:");
        if let Some(total) = data
            .pointer("/data/profileDashSearchAppearances/numSearchAppearances")
            .and_then(|v| v.as_u64())
        {
            println!("  Total appearances: {}", total);
        }
        if let Some(by_company) = data
            .pointer("/data/profileDashSearchAppearances/searchAppearancesByCompany")
            .and_then(|v| v.as_array())
        {
            println!("  Top companies:");
            for entry in by_company.iter().take(10) {
                let name = entry
                    .get("companyName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let count = entry.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("    {} — {} appearances", name, count);
            }
        }
    }

    Ok(())
}
