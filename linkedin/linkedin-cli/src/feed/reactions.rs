//! `feed reactions` command.

use serde_json::Value;

use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

use super::cache::cached_feed_element;
use super::helpers::{print_json, reaction_emoji, text_field, unwrap_update_v2};
use super::urn::normalize_reactions_urn;

/// Options for `feed reactions`. Bundles the five-argument call into a struct.
pub struct FeedReactionsOptions<'a> {
    pub post_urn: Option<&'a str>,
    pub from_list: Option<usize>,
    pub start: u32,
    pub count: u32,
    pub raw_json: bool,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
