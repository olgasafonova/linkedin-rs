use linkedin_api::models::SearchResponse;

use crate::connections::cmd_connections_invite;
use crate::feed::extract_activity_urn;
use crate::profile::cmd_profile_view;
use crate::session::load_session_client;
use crate::util::{print_paging_header, truncate_with_ellipsis};

/// Handle `search people <keywords> [--count N] [--start N] [--json]`.
///
/// Loads the session, calls the Voyager GraphQL `searchDashClustersByAll`
/// endpoint and prints the results.
pub async fn cmd_search_people(
    keywords: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .search_people(keywords, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    // Cache results for index-based profile view.
    if let Err(e) = save_search_cache("people", &value) {
        eprintln!("warning: failed to cache search results: {e}");
    }

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // The response (after GraphQL unwrapping) has: elements (clusters),
    // paging, metadata. Parse paging for the header line.
    let resp: SearchResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse search response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header(&format!("Search results for '{}'", keywords), paging);
    }
    println!("---");

    // Flatten clusters -> items -> entityResult for display.
    // Each cluster element contains an `items` array with search results.
    let mut result_idx = start as usize;
    let mut any_results = false;
    for cluster in &resp.elements {
        let items = cluster
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            let entity = item_wrapper.get("item").and_then(|i| i.get("entityResult"));
            if let Some(entity) = entity {
                result_idx += 1;
                any_results = true;
                print_search_entity(result_idx, entity);
                println!();
            }
        }
    }

    if !any_results {
        println!("(no results)");
    }

    Ok(())
}

/// Print a brief human-readable summary of a single search entity result.
///
/// The GraphQL `searchDashClustersByAll` response returns `entityResult`
/// objects with structured text fields rather than the legacy
/// `SearchProfile.miniProfile` format.
///
/// Fields used:
/// - `title.text`: person's name
/// - `primarySubtitle.text`: headline / occupation
/// - `secondarySubtitle.text`: location
/// - `navigationUrl`: profile link
/// - `badgeText.text`: connection degree badge (e.g., "2nd")
fn print_search_entity(index: usize, entity: &serde_json::Value) {
    let name = entity
        .get("title")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");

    let headline = entity
        .get("primarySubtitle")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let location = entity
        .get("secondarySubtitle")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let badge = entity
        .get("badgeText")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Extract public profile slug from navigationUrl if available.
    let profile_slug = entity
        .get("navigationUrl")
        .and_then(|v| v.as_str())
        .and_then(|url| {
            // URL format: https://www.linkedin.com/in/<slug>?...
            url.strip_prefix("https://www.linkedin.com/in/")
                .map(|rest| rest.split('?').next().unwrap_or(rest))
        })
        .unwrap_or("");

    print!("[{}] {}", index, name);
    if !badge.is_empty() {
        print!(" {}", badge);
    }
    if !profile_slug.is_empty() {
        print!(" ({})", profile_slug);
    }
    println!();

    if !headline.is_empty() {
        println!("    {}", headline);
    }
    if !location.is_empty() {
        println!("    location: {}", location);
    }
}

/// Handle `search jobs <keywords> [--count N] [--start N] [--json]`.
///
/// Loads the session, calls the Voyager GraphQL `searchDashClustersByAll`
/// endpoint with `resultType:List(JOBS)` and prints the results.
pub async fn cmd_search_jobs(
    keywords: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .search_jobs(keywords, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // The jobsDashJobCardsByJobSearch response has the standard collection
    // shape: elements (array of job cards), paging, metadata.
    let resp: SearchResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse search response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header(&format!("Job search results for '{}'", keywords), paging);
    }
    println!("---");

    let mut result_idx = start as usize;
    let mut any_results = false;
    for element in &resp.elements {
        // Each element has jobCard.jobPostingCard with the display fields.
        let card = element
            .get("jobCard")
            .and_then(|jc| jc.get("jobPostingCard"));
        if let Some(card) = card {
            result_idx += 1;
            any_results = true;
            print_job_card(result_idx, card);
            println!();
        }
    }

    if !any_results {
        println!("(no results)");
    }

    Ok(())
}

/// Print a brief human-readable summary of a single job search card.
///
/// The GraphQL `jobsDashJobCardsByJobSearch` response returns elements
/// with `jobCard.jobPostingCard` objects containing:
/// - `jobPosting.title`: job title
/// - `primaryDescription.text`: company name
/// - `secondaryDescription.text`: location
/// - `cardActionV2.navigationAction.actionTarget`: job URL
/// - `footerItems[].timeAt`: posted date (epoch millis)
fn print_job_card(index: usize, card: &serde_json::Value) {
    let title = card
        .get("jobPosting")
        .and_then(|jp| jp.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");

    let company = card
        .get("primaryDescription")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let location = card
        .get("secondaryDescription")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Extract job URL from the card action.
    let job_url = card
        .get("cardActionV2")
        .and_then(|a| a.get("navigationAction"))
        .and_then(|na| na.get("actionTarget"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    print!("[{}] {}", index, title);
    println!();

    if !company.is_empty() {
        println!("    company: {}", company);
    }
    if !location.is_empty() {
        println!("    location: {}", location);
    }
    if !job_url.is_empty() {
        // Show just the job view path, not the full URL with tracking params
        if let Some(path) = job_url.strip_prefix("https://www.linkedin.com") {
            let clean = path.split('?').next().unwrap_or(path);
            println!("    url: {}", clean);
        }
    }
}

fn search_cache_path(kind: &str) -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir
        .join("linkedin")
        .join(format!("last_search_{}.json", kind)))
}

fn save_search_cache(kind: &str, value: &serde_json::Value) -> Result<(), String> {
    let path = search_cache_path(kind)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json = serde_json::to_string(value)
        .map_err(|e| format!("failed to serialize search cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write search cache: {e}"))?;
    Ok(())
}

fn load_search_cache(kind: &str) -> Result<serde_json::Value, String> {
    let path = search_cache_path(kind)?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| format!("no cached {} search. Run `search {}` first.", kind, kind))?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse search cache: {e}"))
}

/// Extract the Nth post's activity URN from cached search results.
fn resolve_search_post_urn(index: usize) -> Result<String, String> {
    let cache = load_search_cache("posts")?;
    let resp: SearchResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached search: {e}"))?;

    let mut result_idx = 0usize;
    for cluster in &resp.elements {
        let items = cluster
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            let sfu = item_wrapper
                .get("item")
                .and_then(|i| i.get("searchFeedUpdate"));
            if let Some(sfu) = sfu {
                result_idx += 1;
                if result_idx == index {
                    let update = sfu.get("update").unwrap_or(sfu);
                    return update
                        .get("metadata")
                        .and_then(|m| m.get("backendUrn"))
                        .and_then(|u| u.as_str())
                        .and_then(|u| {
                            if u.starts_with("urn:li:activity:") {
                                Some(u.to_string())
                            } else {
                                extract_activity_urn(u)
                            }
                        })
                        .ok_or_else(|| format!("search result {} has no activity URN", index));
                }
            }
        }
    }

    Err(format!(
        "index {} out of range (search has {} results)",
        index, result_idx
    ))
}

/// Extract the Nth person's profile slug from cached people search results.
fn resolve_search_person_slug(index: usize) -> Result<String, String> {
    let cache = load_search_cache("people")?;
    let resp: SearchResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached search: {e}"))?;

    let mut result_idx = 0usize;
    for cluster in &resp.elements {
        let items = cluster
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            let entity = item_wrapper.get("item").and_then(|i| i.get("entityResult"));
            if let Some(entity) = entity {
                result_idx += 1;
                if result_idx == index {
                    return entity
                        .get("navigationUrl")
                        .and_then(|v| v.as_str())
                        .and_then(|url| {
                            url.strip_prefix("https://www.linkedin.com/in/")
                                .map(|rest| rest.split('?').next().unwrap_or(rest).to_string())
                        })
                        .ok_or_else(|| format!("search result {} has no profile URL", index));
                }
            }
        }
    }

    Err(format!(
        "index {} out of range (search has {} results)",
        index, result_idx
    ))
}

/// Handle `search posts <keywords> [--count N] [--start N] [--json]`.
pub async fn cmd_search_posts(
    keywords: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .search_content(keywords, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    // Cache results for index-based react/comment.
    if let Err(e) = save_search_cache("posts", &value) {
        eprintln!("warning: failed to cache search results: {e}");
    }

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let resp: SearchResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse search response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header(&format!("Post results for '{}'", keywords), paging);
    }
    println!("---");

    // Content search returns `searchFeedUpdate` items (not `entityResult`).
    let mut result_idx = start as usize;
    let mut any_results = false;
    for cluster in &resp.elements {
        let items = cluster
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            let sfu = item_wrapper
                .get("item")
                .and_then(|i| i.get("searchFeedUpdate"));
            if let Some(sfu) = sfu {
                result_idx += 1;
                any_results = true;
                print_search_content(result_idx, sfu);
                println!();
            }
        }
    }

    if !any_results {
        println!("(no results)");
    }

    Ok(())
}

/// Handle `search react <index> [--type LIKE] [--json]`.
///
/// Reacts to a post from cached search results.
pub async fn cmd_search_react(index: usize, reaction_type: &str, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let activity_urn = resolve_search_post_urn(index)?;
    let rt_upper = reaction_type.to_uppercase();
    let (client, _path) = load_session_client()?;

    eprintln!("Reacting to {} with {}...", activity_urn, rt_upper);
    let result = client
        .react_to_post(&activity_urn, &rt_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Reacted with {} to search result [{}]", rt_upper, index);
    }

    Ok(())
}

/// Handle `search view <index> [--json]`.
///
/// Views a profile from cached people search results.
pub async fn cmd_search_view(index: usize, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let slug = resolve_search_person_slug(index)?;
    eprintln!("Loading profile '{}'...", slug);
    cmd_profile_view(&slug, raw_json, false).await
}

/// Handle `search invite <index> [--message ...] [--json]`.
///
/// Sends a connection invitation to the Nth person in the most recent
/// `search people` results. Pulls the entity URN out of the cached search
/// response so the request never has to resolve a slug — bypasses the
/// flaky GraphQL profile resolver entirely. Falls back to slug-based
/// invite if the cached entry doesn't carry a usable URN (older cache
/// shapes, or LinkedIn omitting the field for a particular result).
pub async fn cmd_search_invite(
    index: usize,
    message: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let target = resolve_search_person_target(index)?;
    eprintln!(
        "Inviting {} via cached search result #{}",
        target.label(),
        index
    );
    cmd_connections_invite(&target.invite_arg(), message, raw_json).await
}

/// Resolved target for a cached search result.
///
/// Carries both the URN (preferred for invites) and the slug (fallback +
/// human-readable label). One of these is always present; the other may
/// be absent depending on the cached payload shape.
struct SearchPersonTarget {
    urn: Option<String>,
    slug: Option<String>,
    name: Option<String>,
}

impl SearchPersonTarget {
    /// Pick the best argument to pass to `cmd_connections_invite`. Prefers
    /// URN (skips the resolver) but falls through to slug when needed.
    fn invite_arg(&self) -> String {
        self.urn
            .clone()
            .or_else(|| self.slug.clone())
            .expect("SearchPersonTarget must have either urn or slug")
    }

    fn label(&self) -> String {
        match (&self.name, &self.slug) {
            (Some(n), Some(s)) => format!("{} ({})", n, s),
            (Some(n), None) => n.clone(),
            (None, Some(s)) => s.clone(),
            (None, None) => self.urn.clone().unwrap_or_default(),
        }
    }
}

/// Resolve a target struct from the cached people-search response by index.
///
/// The cache mirrors the GraphQL `searchDashClustersByAll` envelope:
/// each cluster has an `items` array; entries with an `item.entityResult`
/// represent a person hit. Sponsored, query-clarification, and feedback
/// cards are skipped so the index lines up with what the user sees in the
/// printed list.
fn resolve_search_person_target(index: usize) -> Result<SearchPersonTarget, String> {
    let cache = load_search_cache("people")?;
    let resp: SearchResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached search: {e}"))?;

    let mut result_idx = 0usize;
    for cluster in &resp.elements {
        let items = cluster
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            let entity = match item_wrapper.get("item").and_then(|i| i.get("entityResult")) {
                Some(e) => e,
                None => continue,
            };
            result_idx += 1;
            if result_idx != index {
                continue;
            }

            let urn = entity
                .get("entityUrn")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("urn:li:fsd_profile:"))
                .map(|s| s.to_string())
                .or_else(|| {
                    entity
                        .get("trackingUrn")
                        .and_then(|v| v.as_str())
                        .filter(|s| s.starts_with("urn:li:fsd_profile:"))
                        .map(|s| s.to_string())
                });

            let slug = entity
                .get("navigationUrl")
                .and_then(|v| v.as_str())
                .and_then(|url| {
                    url.strip_prefix("https://www.linkedin.com/in/")
                        .map(|rest| rest.split('?').next().unwrap_or(rest).to_string())
                });

            let name = entity
                .get("title")
                .and_then(|t| t.get("text"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if urn.is_none() && slug.is_none() {
                return Err(format!(
                    "search result {} has neither entityUrn nor navigationUrl",
                    index
                ));
            }

            return Ok(SearchPersonTarget { urn, slug, name });
        }
    }

    Err(format!(
        "index {} out of range (search has {} results)",
        index, result_idx
    ))
}

/// Print a human-readable summary of a content search result.
///
/// Content search results use `searchFeedUpdate` which contains an `update`
/// field with the same structure as feed `UpdateV2` items.
fn print_search_content(index: usize, sfu: &serde_json::Value) {
    let update = sfu.get("update").unwrap_or(sfu);

    let actor_name = update
        .get("actor")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(unknown)");

    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let commentary_display = truncate_with_ellipsis(commentary, 200);

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

    // Extract the activity URN for react/comment support.
    // Primary source: update.metadata.backendUrn (contains urn:li:activity:XXX).
    // Fallback: update.entityUrn, socialDetail.entityUrn, or permalink.
    let activity_urn = update
        .get("metadata")
        .and_then(|m| m.get("backendUrn"))
        .and_then(|u| u.as_str())
        .and_then(|u| {
            if u.starts_with("urn:li:activity:") {
                Some(u.to_string())
            } else {
                extract_activity_urn(u)
            }
        })
        .or_else(|| {
            update
                .get("entityUrn")
                .and_then(|u| u.as_str())
                .and_then(extract_activity_urn)
        })
        .or_else(|| {
            update
                .get("socialDetail")
                .and_then(|sd| sd.get("entityUrn"))
                .and_then(|u| u.as_str())
                .and_then(extract_activity_urn)
        })
        .unwrap_or_default();

    let permalink = sfu.get("permalink").and_then(|p| p.as_str()).unwrap_or("");

    println!("[{}] {}", index, actor_name);
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    println!("    likes: {}  comments: {}", likes, comments);
    if !activity_urn.is_empty() {
        // Generate a LinkedIn post URL from the activity URN.
        let activity_id = activity_urn
            .strip_prefix("urn:li:activity:")
            .unwrap_or(&activity_urn);
        println!(
            "    https://www.linkedin.com/feed/update/urn:li:activity:{}",
            activity_id
        );
        println!("    URN: {}", activity_urn);
    }
    if !permalink.is_empty() && activity_urn.is_empty() {
        println!("    {}", permalink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_target_prefers_entity_urn() {
        let target = SearchPersonTarget {
            urn: Some("urn:li:fsd_profile:ACoAAA111".to_string()),
            slug: Some("jane-doe".to_string()),
            name: Some("Jane Doe".to_string()),
        };
        assert_eq!(target.invite_arg(), "urn:li:fsd_profile:ACoAAA111");
        assert_eq!(target.label(), "Jane Doe (jane-doe)");
    }

    #[test]
    fn search_target_falls_back_to_slug() {
        let target = SearchPersonTarget {
            urn: None,
            slug: Some("jane-doe".to_string()),
            name: Some("Jane Doe".to_string()),
        };
        assert_eq!(target.invite_arg(), "jane-doe");
    }

    #[test]
    fn search_target_label_with_only_slug() {
        let target = SearchPersonTarget {
            urn: None,
            slug: Some("jane-doe".to_string()),
            name: None,
        };
        assert_eq!(target.label(), "jane-doe");
    }
}
