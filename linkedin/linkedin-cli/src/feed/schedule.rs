//! Schedule, get, and delete scheduled posts.

use std::path::Path;

use crate::error::{CliError, CliResult};
use crate::session::load_session_client;

use super::helpers::print_json;

pub async fn cmd_feed_schedule(
    text: Option<&str>,
    caption_file: Option<&Path>,
    media: Option<&Path>,
    title: Option<&str>,
    schedule_str: &str,
    timezone: &str,
    visibility: &str,
    media_ready_timeout: u64,
    confirmed: bool,
    raw_json: bool,
) -> CliResult<()> {
    let vis_upper = visibility.to_uppercase();
    if vis_upper != "ANYONE" && vis_upper != "CONNECTIONS_ONLY" {
        return Err(CliError::Other(format!(
            "invalid visibility '{}'. Must be ANYONE or CONNECTIONS_ONLY",
            visibility
        )));
    }

    // Resolve text content
    let post_text = match (text, caption_file) {
        (Some(t), _) => t.to_string(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map_err(|e| CliError::Other(format!("failed to read {}: {}", path.display(), e)))?,
        (None, None) => {
            return Err(CliError::Input(
                "provide --text or --caption-file with post content".to_string(),
            ));
        }
    };

    // Parse schedule time
    let tz: chrono_tz::Tz = timezone
        .parse()
        .map_err(|_| CliError::Other(format!("invalid timezone '{}'", timezone)))?;
    let naive = chrono::NaiveDateTime::parse_from_str(schedule_str, "%Y-%m-%d %H:%M")
        .map_err(|e| CliError::Other(format!("invalid schedule time '{}': {}", schedule_str, e)))?;
    let scheduled = naive.and_local_timezone(tz).single().ok_or_else(|| {
        CliError::Other(format!(
            "ambiguous or invalid time {} in timezone {}",
            schedule_str, timezone
        ))
    })?;
    let scheduled_ms = scheduled.timestamp_millis();

    if !confirmed {
        eprintln!("WARNING: This will create a REAL scheduled post on LinkedIn!");
        eprintln!();
        eprintln!("  Visibility: {}", vis_upper);
        eprintln!("  Scheduled: {} ({})", schedule_str, timezone);
        eprintln!("  Text: {}...", &post_text[..post_text.len().min(100)]);
        if media.is_some() {
            eprintln!("  Media: {}", media.unwrap().display());
        }
        eprintln!();
        eprintln!("Use --yes to confirm.");
        return Err(CliError::Other(
            "schedule not confirmed (use --yes)".to_string(),
        ));
    }

    let (client, _path) = load_session_client()?;

    let result = if let Some(media_path) = media {
        eprintln!("Uploading media...");
        client
            .schedule_post_with_media(
                &post_text,
                &vis_upper,
                scheduled_ms,
                media_path,
                Some(title.unwrap_or("Document")),
                media_ready_timeout,
            )
            .await?
    } else {
        client
            .schedule_post(&post_text, &vis_upper, scheduled_ms)
            .await?
    };

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
        println!("Post scheduled successfully!");
        println!("  URN: {}", urn);
        println!("  Scheduled: {} ({})", schedule_str, timezone);
        println!("  Visibility: {}", vis_upper);
    }
    Ok(())
}

pub async fn cmd_feed_schedule_get(share_urn: &str, raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;
    eprintln!("Fetching share {}...", share_urn);
    let result = client.get_share(share_urn).await?;

    if raw_json {
        print_json(&result)?;
    } else {
        println!("Share: {}", share_urn);
        if let Some(text) = result
            .get("value")
            .and_then(|v| v.get("com.linkedin.voyager.feed.render.FeedUpdate"))
            .and_then(|v| v.get("updateMetadata"))
            .and_then(|v| v.get("updateActions"))
        {
            println!("  Actions: {}", text);
        }
    }
    Ok(())
}

pub async fn cmd_feed_schedule_delete(share_urn: &str, confirmed: bool) -> CliResult<()> {
    if !confirmed {
        return Err(CliError::Input(
            "this will DELETE a REAL share from LinkedIn. Pass --yes to confirm.".to_string(),
        ));
    }

    let (client, _path) = load_session_client()?;
    eprintln!("Deleting share {}...", share_urn);
    client.delete_share(share_urn).await?;
    println!("Share {} deleted.", share_urn);
    Ok(())
}
