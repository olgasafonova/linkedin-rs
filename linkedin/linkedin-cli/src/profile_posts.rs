//! `profile posts` subcommand: per-member posts harvest with
//! first-comment link extraction. Split out of `profile.rs`; the JSON
//! output shape is the load-bearing contract with `linkedin-curator-sift`.

use serde_json::{json, Value};

use linkedin_api::client::LinkedInClient;
use linkedin_api::urn::SocialDetailUrn;

use crate::error::CliResult;
use crate::profile::{field_str, nested_text, print_json};
use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

/// Handle `profile posts <public_id> [--count N] [--with-first-comment] [--json]`.
///
/// Resolves the slug to a `fsd_profile` URN, calls `get_member_posts` for the
/// most-recent N posts, and (when `--with-first-comment`) follows up with one
/// `get_comments` call per post to surface URLs the author dropped in their
/// own first comment — the canonical LinkedIn "link in first comment" pattern.
///
/// The CLI does no time-window filtering; the relative time string ("2d",
/// "1w") is exposed under `postedAtRelative` for callers to filter on. Date
/// arithmetic lives in the consuming skill.
pub async fn cmd_profile_posts(
    public_id: &str,
    count: u32,
    with_first_comment: bool,
    raw_json: bool,
) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    eprintln!("Resolving profile URN for '{}'...", public_id);
    let profile_urn = client.resolve_profile_urn(public_id).await?;

    eprintln!("Fetching up to {} posts...", count);
    let response = client.get_member_posts(&profile_urn, 0, count).await?;

    let elements_owned: Vec<Value> = response
        .get("elements")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let queried_urn = profile_urn.as_str();
    let mut posts: Vec<Value> = elements_owned.iter().map(extract_post_data).collect();

    if with_first_comment {
        attach_first_comments(&client, &mut posts).await;
    }

    if raw_json {
        let output = json!({
            "author": {
                "slug": public_id,
                "urn": queried_urn,
            },
            "fetchedAt": chrono::Utc::now().to_rfc3339(),
            "count": posts.len(),
            "withFirstComment": with_first_comment,
            "posts": posts,
        });
        return print_json(&output);
    }

    print_posts_human(public_id, &posts);
    Ok(())
}

/// Fetch each post's author first-comment and attach it under
/// `firstCommentByAuthor`. Posts with no fetchable comment are left untouched.
async fn attach_first_comments(client: &LinkedInClient, posts: &mut [Value]) {
    for post in posts.iter_mut() {
        let Some(comment_data) = fetch_first_comment_by_author(client, post).await else {
            continue;
        };
        if let Some(obj) = post.as_object_mut() {
            obj.insert("firstCommentByAuthor".to_string(), comment_data);
        }
    }
}

/// Pull the canonical fields the skill needs out of a single UpdateV2
/// element. The output JSON shape is the load-bearing contract between
/// `li profile posts --json` and `linkedin-curator-sift`; keep it stable.
fn extract_post_data(element: &Value) -> Value {
    let update = unwrap_update_v2_local(element);
    let entity_urn = field_str(element, "entityUrn");
    let activity_urn = activity_urn_from_fs_update(entity_urn);

    let actor_urn = nested_text(update, &["actor", "urn"])
        .or_else(|| nested_text(update, &["actor", "entityUrn"]))
        .unwrap_or("")
        .to_string();

    // `resharedUpdate` is populated (non-empty object) when this post is a
    // reshare of another post; absent or null on originals. Cross-namespace
    // URN comparison was tempting but fragile — actor.urn comes back as
    // urn:li:member:N (legacy) while queried URNs are urn:li:fsd_profile:...
    // (Dash). resharedUpdate is shape-independent and canonical.
    let is_reshare = is_reshare_update(update);

    let actor_name = nested_text(update, &["actor", "name", "text"])
        .unwrap_or("")
        .to_string();
    let text = nested_text(update, &["commentary", "text", "text"])
        .unwrap_or("")
        .to_string();
    let posted_relative = extract_posted_relative(update);
    let permalink = build_permalink(&activity_urn);
    let article_url = extract_component_action_target(
        update,
        "com.linkedin.voyager.feed.render.ArticleComponent",
    );
    let external_video_url = extract_component_action_target(
        update,
        "com.linkedin.voyager.feed.render.ExternalVideoComponent",
    );
    let inline_urls = extract_hyperlink_attrs(update.get("commentary").and_then(|c| c.get("text")));
    let social_detail_urn = extract_social_detail_urn(update);

    json!({
        "activityUrn": activity_urn,
        "permalink": permalink,
        "postedAtRelative": posted_relative,
        "text": text,
        "isReshare": is_reshare,
        "actorUrn": actor_urn,
        "actorName": actor_name,
        "inlineUrls": inline_urls,
        "articleUrl": optional_string(article_url),
        "externalVideoUrl": optional_string(external_video_url),
        "socialDetailUrn": social_detail_urn,
        "firstCommentByAuthor": Value::Null,
    })
}

/// Extract the "posted N ago" relative timestamp, stripping LinkedIn's
/// trailing bullet/non-breaking-space decorations.
fn extract_posted_relative(update: &Value) -> String {
    nested_text(update, &["actor", "subDescription", "text"])
        .unwrap_or("")
        .trim()
        .trim_end_matches("• \u{a0}\u{a0}")
        .trim_end_matches('•')
        .trim()
        .to_string()
}

/// Build the canonical feed permalink, or an empty string when no activity URN.
fn build_permalink(activity_urn: &str) -> String {
    if activity_urn.is_empty() {
        String::new()
    } else {
        format!("https://www.linkedin.com/feed/update/{}", activity_urn)
    }
}

/// Pull `content.<component>.navigationContext.actionTarget` out of an update.
fn extract_component_action_target(update: &Value, component: &str) -> String {
    nested_text(
        update,
        &["content", component, "navigationContext", "actionTarget"],
    )
    .unwrap_or("")
    .to_string()
}

/// Resolve the social-detail URN used by the comments API.
///
/// Priority: dashEntityUrn (urn:li:fsd_socialDetail:..., the modern Dash URN
/// that socialDashCommentsBySocialDetail accepts) → entityUrn (legacy
/// fs_socialDetail) → urn (the underlying ugcPost/activity URN; LinkedIn
/// rejects this with "Deserializing failed").
fn extract_social_detail_urn(update: &Value) -> String {
    nested_text(update, &["socialDetail", "dashEntityUrn"])
        .or_else(|| nested_text(update, &["socialDetail", "entityUrn"]))
        .or_else(|| nested_text(update, &["socialDetail", "urn"]))
        .unwrap_or("")
        .to_string()
}

/// Best-effort fetch of the post-author's own first comment on their post.
/// Returns `None` if any link in the chain is missing (no social-detail URN,
/// no actor URN, comments call fails, no comment by the author).
///
/// "First" here means earliest by `createdAt`, not LinkedIn's display order
/// (which is RELEVANCE by default). The author's intent is the *first* drop,
/// so chronological order matches the link-in-first-comment pattern.
async fn fetch_first_comment_by_author(client: &LinkedInClient, post: &Value) -> Option<Value> {
    let social_detail_urn = post.get("socialDetailUrn").and_then(|v| v.as_str())?;
    let author_urn = post.get("actorUrn").and_then(|v| v.as_str())?;
    if social_detail_urn.is_empty() || author_urn.is_empty() {
        return None;
    }

    let urn = SocialDetailUrn::new(social_detail_urn);
    let comments = match client.get_comments(&urn, 0, 20).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "warning: failed to fetch comments for {}: {}",
                social_detail_urn, e
            );
            return None;
        }
    };

    let elements = comments.get("elements").and_then(|e| e.as_array())?;
    let mut author_comments: Vec<&Value> = elements
        .iter()
        .filter(|c| comment_author_urn(c) == Some(author_urn))
        .collect();
    author_comments.sort_by_key(|c| {
        c.get("createdAt")
            .and_then(|v| v.as_i64())
            .unwrap_or(i64::MAX)
    });

    let first = author_comments.first()?;
    let comment_urn = first
        .get("urn")
        .and_then(|v| v.as_str())
        .or_else(|| first.get("entityUrn").and_then(|v| v.as_str()))
        .unwrap_or("");
    let comment_text = comment_commentary_text(first);
    let extracted_urls = comment_hyperlinks(first);

    Some(json!({
        "commentUrn": comment_urn,
        "text": comment_text,
        "extractedUrls": extracted_urls,
    }))
}

// --- Pure helpers (testable without network) ---

/// True when `resharedUpdate` carries a non-empty object. Absent, null, and
/// `{}` all count as "not a reshare".
fn is_reshare_update(update: &Value) -> bool {
    match update.get("resharedUpdate") {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Object(map)) => !map.is_empty(),
        Some(_) => false,
    }
}

/// Navigate `value["com.linkedin.voyager.feed.render.UpdateV2"]`, or fall
/// back to the element itself when the wrapper is absent (some endpoints
/// flatten this layer).
fn unwrap_update_v2_local(element: &Value) -> &Value {
    element
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(element)
}

/// Pull the `urn:li:activity:N` out of the composite `fs_updateV2` entity
/// URN. Returns the input unchanged if it already looks like an activity
/// URN; returns empty string otherwise.
fn activity_urn_from_fs_update(entity_urn: &str) -> String {
    if entity_urn.starts_with("urn:li:activity:") {
        return entity_urn.to_string();
    }
    let Some(paren_open) = entity_urn.find('(') else {
        return String::new();
    };
    let rest = &entity_urn[paren_open + 1..];
    let end = rest.find(',').unwrap_or(rest.len());
    let candidate = &rest[..end];
    if candidate.starts_with("urn:li:activity:") {
        candidate.to_string()
    } else {
        String::new()
    }
}

/// Pull commenter URN from a `socialDashCommentsBySocialDetail` element.
/// Tolerates two known shapes (`commenter.actor.urn` and `commenter.urn`).
fn comment_author_urn(comment: &Value) -> Option<&str> {
    comment
        .get("commenter")
        .and_then(|c| c.get("actor"))
        .and_then(|a| a.get("urn"))
        .and_then(|u| u.as_str())
        .or_else(|| {
            comment
                .get("commenter")
                .and_then(|c| c.get("urn"))
                .and_then(|u| u.as_str())
        })
}

fn comment_commentary_text(comment: &Value) -> String {
    nested_text(comment, &["commentary", "text", "text"])
        .or_else(|| nested_text(comment, &["commentary", "text"]))
        .unwrap_or("")
        .to_string()
}

/// Extract URLs from a comment, preferring HYPERLINK attributes (the
/// structured channel LinkedIn populates when a real URL was pasted) and
/// falling back to a plain-text URL scan when attributes are absent (older
/// comments or text-only mobile composers).
fn comment_hyperlinks(comment: &Value) -> Vec<String> {
    let attr_urls = extract_hyperlink_attrs(comment.get("commentary").and_then(|c| c.get("text")));
    if !attr_urls.is_empty() {
        return attr_urls;
    }
    plaintext_urls(&comment_commentary_text(comment))
}

/// Attribute `type` values that carry an outbound URL. `TEXT_LINK` is what
/// Voyager actually emits on both `feed/updates` and
/// `identity/profileUpdatesV2`; `HYPERLINK` is kept because older captured
/// fixtures use it and it costs nothing to accept.
const LINK_ATTR_TYPES: &[&str] = &["TEXT_LINK", "HYPERLINK"];

/// Pull outbound URLs out of a commentary `text` object's attribute list.
///
/// The URL lives at `attributes[].textLink.url` on live responses. A flat
/// `attributes[].url` is also accepted for the older shape.
///
/// Verified 28-07-2026 against 20 live posts on `identity/profileUpdatesV2`
/// (`li feed my-posts --json`): 6 link attributes, all `TEXT_LINK` with the
/// URL under `textLink.url`, and zero `HYPERLINK`. Matching only `HYPERLINK`
/// with a flat `url` — as this did previously — returned an empty vec for
/// every real post, which is why `inlineUrls` was always `[]`.
fn extract_hyperlink_attrs(text_obj: Option<&Value>) -> Vec<String> {
    text_obj
        .and_then(|t| t.get("attributes"))
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(link_attr_url).collect())
        .unwrap_or_default()
}

/// Return the URL carried by a single commentary attribute, or `None` when
/// the attribute is not a link type (mentions, company tags) or has no URL.
fn link_attr_url(attr: &Value) -> Option<String> {
    let attr_type = attr.get("type").and_then(|t| t.as_str())?;
    if !LINK_ATTR_TYPES.contains(&attr_type) {
        return None;
    }
    attr.get("textLink")
        .and_then(|l| l.get("url"))
        .and_then(|u| u.as_str())
        .or_else(|| attr.get("url").and_then(|u| u.as_str()))
        .map(String::from)
}

fn plaintext_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|w| w.starts_with("http://") || w.starts_with("https://"))
        .map(|w| {
            w.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '-' && c != '_')
                .to_string()
        })
        .collect()
}

fn optional_string(s: String) -> Value {
    if s.is_empty() {
        Value::Null
    } else {
        Value::String(s)
    }
}

fn print_posts_human(public_id: &str, posts: &[Value]) {
    println!("Posts by {}", public_id);
    println!("===");
    if posts.is_empty() {
        println!("(no posts)");
        return;
    }
    for (i, post) in posts.iter().enumerate() {
        print_one_post(i, post);
    }
}

/// Render a single post entry (zero-based `index`) in the human-readable format.
fn print_one_post(index: usize, post: &Value) {
    let activity_urn = post
        .get("activityUrn")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let posted = post
        .get("postedAtRelative")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let is_reshare = post
        .get("isReshare")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let actor_name = post.get("actorName").and_then(|v| v.as_str()).unwrap_or("");
    let text = post.get("text").and_then(|v| v.as_str()).unwrap_or("");

    let reshare_tag = if is_reshare { " [reshare]" } else { "" };
    let posted_tag = if posted.is_empty() {
        String::new()
    } else {
        format!(" ({})", posted)
    };
    println!(
        "\n[{}] {}{}{}",
        index + 1,
        activity_urn,
        posted_tag,
        reshare_tag
    );
    if is_reshare && !actor_name.is_empty() {
        println!("    original author: {}", actor_name);
    }
    if !text.is_empty() {
        println!("    {}", truncate_with_ellipsis(text, 200));
    }
    print_labeled_url(post, "articleUrl", "article");
    print_labeled_url(post, "externalVideoUrl", "video");
    print_url_array(post.get("inlineUrls"), "inline");
    print_first_comment_links(post);
}

/// Print `    <label>: <url>` when `post[key]` is a non-empty string.
fn print_labeled_url(post: &Value, key: &str, label: &str) {
    if let Some(url) = post.get(key).and_then(|v| v.as_str()) {
        println!("    {}: {}", label, url);
    }
}

/// Print `    <label>: <url>` for each string in the JSON array `value`.
fn print_url_array(value: Option<&Value>, label: &str) {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return;
    };
    for url in arr.iter().filter_map(|u| u.as_str()) {
        println!("    {}: {}", label, url);
    }
}

/// Print the author's first-comment links, when present.
fn print_first_comment_links(post: &Value) {
    let comment = post
        .get("firstCommentByAuthor")
        .filter(|v| !v.is_null())
        .and_then(|v| v.as_object());
    let Some(comment) = comment else { return };
    print_url_array(comment.get("extractedUrls"), "first-comment-link");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_activity_urn_strips_fs_updatev2_wrapper() {
        let entity =
            "urn:li:fs_updateV2:(urn:li:activity:7447168805107032064,MEMBER_SHARES,DEBUG_REASON,DEFAULT,false)";
        assert_eq!(
            activity_urn_from_fs_update(entity),
            "urn:li:activity:7447168805107032064"
        );
    }

    #[test]
    fn extract_activity_urn_handles_bare_activity() {
        let entity = "urn:li:activity:7447168805107032064";
        assert_eq!(
            activity_urn_from_fs_update(entity),
            "urn:li:activity:7447168805107032064"
        );
    }

    #[test]
    fn extract_activity_urn_returns_empty_for_unrecognized() {
        assert_eq!(activity_urn_from_fs_update(""), "");
        assert_eq!(activity_urn_from_fs_update("garbage"), "");
    }

    #[test]
    fn is_reshare_update_detects_non_empty_resharedupdate() {
        let update = json!({
            "resharedUpdate": {"actor": {"name": {"text": "Original Author"}}}
        });
        assert!(is_reshare_update(&update));
    }

    #[test]
    fn is_reshare_update_returns_false_when_field_absent() {
        let update = json!({"commentary": {"text": {"text": "original post"}}});
        assert!(!is_reshare_update(&update));
    }

    #[test]
    fn is_reshare_update_returns_false_when_field_is_empty_object() {
        let update = json!({"resharedUpdate": {}});
        assert!(!is_reshare_update(&update));
    }

    #[test]
    fn is_reshare_update_returns_false_when_field_is_null() {
        let update = json!({"resharedUpdate": null});
        assert!(!is_reshare_update(&update));
    }

    #[test]
    fn extract_post_data_marks_reshare_via_resharedupdate_presence() {
        // Live shape from memberShareFeed: top-level flat (no value-wrapping),
        // actor.urn in legacy member namespace, resharedUpdate populated.
        let element = json!({
            "entityUrn": "urn:li:fs_updateV2:(urn:li:activity:7447168805107032064,MEMBER_SHARES,DEBUG,DEFAULT,false)",
            "actor": {
                "urn": "urn:li:member:672494610",
                "name": {"text": "Eric Vyacheslav"},
                "subDescription": {"text": "2d • "}
            },
            "commentary": {"text": {"text": "Reshare with commentary"}},
            "resharedUpdate": {
                "actor": {"name": {"text": "Some Original Author"}}
            },
            "socialDetail": {
                "dashEntityUrn": "urn:li:fsd_socialDetail:(urn:li:activity:7447168805107032064,...)",
                "entityUrn": "urn:li:fs_socialDetail:urn:li:activity:7447168805107032064",
                "urn": "urn:li:activity:7447168805107032064"
            }
        });
        let post = extract_post_data(&element);
        assert_eq!(post["isReshare"], true);
        assert_eq!(post["actorName"], "Eric Vyacheslav");
        assert_eq!(post["postedAtRelative"], "2d");
        assert_eq!(post["activityUrn"], "urn:li:activity:7447168805107032064");
    }

    #[test]
    fn extract_post_data_marks_original_when_resharedupdate_absent() {
        let element = json!({
            "entityUrn": "urn:li:fs_updateV2:(urn:li:activity:1234,MEMBER_SHARES,DEBUG,DEFAULT,false)",
            "actor": {
                "urn": "urn:li:member:672494610",
                "name": {"text": "Eric Vyacheslav"}
            },
            "commentary": {
                "text": {
                    "text": "Check https://example.com/cool",
                    "attributes": [
                        {"type": "HYPERLINK", "url": "https://example.com/cool"}
                    ]
                }
            }
        });
        let post = extract_post_data(&element);
        assert_eq!(post["isReshare"], false);
        assert_eq!(post["inlineUrls"], json!(["https://example.com/cool"]));
    }

    #[test]
    fn extract_post_data_picks_dash_socialdetail_urn() {
        let element = json!({
            "entityUrn": "urn:li:fs_updateV2:(urn:li:activity:99,MEMBER_SHARES,DEBUG,DEFAULT,false)",
            "actor": {"urn": "urn:li:member:1", "name": {"text": "X"}},
            "commentary": {"text": {"text": "x"}},
            "socialDetail": {
                "dashEntityUrn": "urn:li:fsd_socialDetail:(urn:li:activity:99,urn:li:activity:99,-)",
                "entityUrn": "urn:li:fs_socialDetail:urn:li:activity:99",
                "urn": "urn:li:activity:99"
            }
        });
        let post = extract_post_data(&element);
        assert_eq!(
            post["socialDetailUrn"],
            "urn:li:fsd_socialDetail:(urn:li:activity:99,urn:li:activity:99,-)"
        );
    }

    #[test]
    fn extract_post_data_reads_live_profile_updates_v2_element() {
        // Verbatim element shape from `identity/profileUpdatesV2?q=memberShareFeed`
        // captured 28-07-2026 (`li feed my-posts --json`, element 16), with the
        // miniCompany logo blob trimmed. Three things this pins down:
        //   1. Elements are FLAT -- no `value.<UpdateV2>` wrapper on this finder,
        //      so `unwrap_update_v2_local` must fall through to the element.
        //   2. Link attributes are `TEXT_LINK` with the URL at `textLink.url`,
        //      never `HYPERLINK` with a flat `url`.
        //   3. COMPANY_NAME attributes sit alongside and carry no URL.
        let element = json!({
            "entityUrn": "urn:li:fs_updateV2:(urn:li:activity:7470215905017401344,MEMBER_SHARES,DEBUG_REASON,DEFAULT,false)",
            "actor": {
                "urn": "urn:li:member:4963014",
                "name": {"text": "Olga Safonova"},
                "subDescription": {"text": "1mo •   "}
            },
            "commentary": {
                "text": {
                    "text": "So Anthropic just got #Fable out https://lnkd.in/eDCv4Bwp",
                    "attributes": [
                        {
                            "type": "COMPANY_NAME",
                            "start": 3,
                            "length": 9,
                            "miniCompany": {"name": "Anthropic"}
                        },
                        {
                            "type": "TEXT_LINK",
                            "start": 96,
                            "length": 24,
                            "textLink": {
                                "url": "https://lnkd.in/eDCv4Bwp",
                                "viewingBehavior": "DEFAULT"
                            }
                        }
                    ]
                }
            },
            "content": {
                "com.linkedin.voyager.feed.render.ArticleComponent": {
                    "navigationContext": {"actionTarget": "https://example.com/article"}
                }
            }
        });
        let post = extract_post_data(&element);
        assert_eq!(post["inlineUrls"], json!(["https://lnkd.in/eDCv4Bwp"]));
        assert_eq!(post["articleUrl"], "https://example.com/article");
        assert_eq!(post["activityUrn"], "urn:li:activity:7470215905017401344");
        assert_eq!(post["postedAtRelative"], "1mo");
    }

    #[test]
    fn extract_hyperlink_attrs_captures_auto_linkified_bare_text() {
        // LinkedIn linkifies bare tokens like "SKILL.md" into a TEXT_LINK with
        // an http:// URL that never appears in the post text. A plain-text URL
        // scan cannot see these, so the attribute channel is the only source.
        let text_obj = json!({
            "text": "documented in SKILL.md",
            "attributes": [
                {
                    "type": "TEXT_LINK",
                    "start": 14,
                    "length": 8,
                    "textLink": {"url": "http://SKILL.md", "viewingBehavior": "DEFAULT"}
                }
            ]
        });
        assert_eq!(
            extract_hyperlink_attrs(Some(&text_obj)),
            vec!["http://SKILL.md"]
        );
    }

    #[test]
    fn extract_hyperlink_attrs_ignores_mention_attributes() {
        let text_obj = json!({
            "text": "thanks to everyone",
            "attributes": [
                {"type": "PROFILE_MENTION", "start": 0, "length": 6},
                {"type": "COMPANY_NAME", "start": 7, "length": 2}
            ]
        });
        assert!(extract_hyperlink_attrs(Some(&text_obj)).is_empty());
    }

    #[test]
    fn comment_hyperlinks_falls_back_to_plaintext_url_scan() {
        let comment = json!({
            "commentary": {
                "text": {
                    "text": "Link: https://example.com/article! and more"
                }
            }
        });
        let urls = comment_hyperlinks(&comment);
        assert_eq!(urls, vec!["https://example.com/article"]);
    }

    #[test]
    fn comment_hyperlinks_prefers_attributes_over_plaintext() {
        let comment = json!({
            "commentary": {
                "text": {
                    "text": "see https://wrong.example.com",
                    "attributes": [
                        {"type": "HYPERLINK", "url": "https://right.example.com"}
                    ]
                }
            }
        });
        let urls = comment_hyperlinks(&comment);
        assert_eq!(urls, vec!["https://right.example.com"]);
    }
}
