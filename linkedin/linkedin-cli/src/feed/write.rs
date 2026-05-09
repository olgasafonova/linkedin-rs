//! Mutating feed commands: react, unreact, comment, post.

use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

use super::helpers::print_json;
use super::urn::resolve_post_urn;

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
