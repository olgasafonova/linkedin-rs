use serde_json::Value;

use linkedin_api::models::SearchResponse;
use linkedin_api::urn::extract_activity_urn;

use crate::connections::cmd_connections_invite;
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

    if let Err(e) = save_search_cache("people", &value) {
        eprintln!("warning: failed to cache search results: {e}");
    }

    if raw_json {
        return print_json(&value);
    }

    let resp = parse_search_response(&value)?;
    print_search_header(&resp, &format!("Search results for '{}'", keywords));

    let entities: Vec<&Value> = iter_cluster_items(&resp, "entityResult").collect();
    if entities.is_empty() {
        println!("(no results)");
        return Ok(());
    }
    for (i, entity) in entities.iter().enumerate() {
        print_search_entity(start as usize + i + 1, entity);
        println!();
    }

    Ok(())
}

/// Print a brief human-readable summary of a single search entity result.
///
/// The GraphQL `searchDashClustersByAll` response returns `entityResult`
/// objects with structured text fields rather than the legacy
/// `SearchProfile.miniProfile` format.
fn print_search_entity(index: usize, entity: &Value) {
    let name = text_field(entity, "title").unwrap_or("(unknown)");
    let headline = text_field(entity, "primarySubtitle").unwrap_or("");
    let location = text_field(entity, "secondarySubtitle").unwrap_or("");
    let badge = text_field(entity, "badgeText").unwrap_or("");
    let profile_slug = entity
        .get("navigationUrl")
        .and_then(|v| v.as_str())
        .and_then(profile_slug_from_url)
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
        return print_json(&value);
    }

    let resp = parse_search_response(&value)?;
    print_search_header(&resp, &format!("Job search results for '{}'", keywords));

    let cards: Vec<&Value> = resp
        .elements
        .iter()
        .filter_map(|el| el.get("jobCard").and_then(|jc| jc.get("jobPostingCard")))
        .collect();
    if cards.is_empty() {
        println!("(no results)");
        return Ok(());
    }
    for (i, card) in cards.iter().enumerate() {
        print_job_card(start as usize + i + 1, card);
        println!();
    }

    Ok(())
}

/// Print a brief human-readable summary of a single job search card.
fn print_job_card(index: usize, card: &Value) {
    let title = card
        .get("jobPosting")
        .and_then(|jp| jp.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    let company = text_field(card, "primaryDescription").unwrap_or("");
    let location = text_field(card, "secondaryDescription").unwrap_or("");
    let job_url = card
        .get("cardActionV2")
        .and_then(|a| a.get("navigationAction"))
        .and_then(|na| na.get("actionTarget"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    println!("[{}] {}", index, title);
    if !company.is_empty() {
        println!("    company: {}", company);
    }
    if !location.is_empty() {
        println!("    location: {}", location);
    }
    if let Some(path) = job_url.strip_prefix("https://www.linkedin.com") {
        let clean = path.split('?').next().unwrap_or(path);
        println!("    url: {}", clean);
    }
}

fn search_cache_path(kind: &str) -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir
        .join("linkedin")
        .join(format!("last_search_{}.json", kind)))
}

fn save_search_cache(kind: &str, value: &Value) -> Result<(), String> {
    let path = search_cache_path(kind)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json = serde_json::to_string(value)
        .map_err(|e| format!("failed to serialize search cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write search cache: {e}"))?;
    Ok(())
}

fn load_search_cache(kind: &str) -> Result<Value, String> {
    let path = search_cache_path(kind)?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| format!("no cached {} search. Run `search {}` first.", kind, kind))?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse search cache: {e}"))
}

/// Pretty-print a JSON value to stdout.
fn print_json(value: &Value) -> Result<(), String> {
    let pretty =
        serde_json::to_string_pretty(value).map_err(|e| format!("JSON format error: {e}"))?;
    println!("{}", pretty);
    Ok(())
}

/// Decode a raw search payload into the typed envelope.
fn parse_search_response(value: &Value) -> Result<SearchResponse, String> {
    serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse search response: {e}"))
}

/// Print the standard "label … --- " header used by every search command.
fn print_search_header(resp: &SearchResponse, label: &str) {
    if let Some(ref paging) = resp.paging {
        print_paging_header(label, paging);
    }
    println!("---");
}

/// Iterate over all `item.<kind>` values across every cluster's `items`
/// array. Skips wrappers that don't carry the requested kind, so the
/// resulting index lines up with what users see in the printed list.
fn iter_cluster_items<'a>(
    resp: &'a SearchResponse,
    kind: &'a str,
) -> impl Iterator<Item = &'a Value> + 'a {
    resp.elements
        .iter()
        .flat_map(|cluster| {
            cluster
                .get("items")
                .and_then(|v| v.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[])
                .iter()
        })
        .filter_map(move |wrapper| wrapper.get("item").and_then(|i| i.get(kind)))
}

/// Find the Nth (1-based) item of `kind` from a cached search payload.
fn nth_cached_item(cache_kind: &str, item_kind: &str, index: usize) -> Result<Value, String> {
    let cache = load_search_cache(cache_kind)?;
    let resp = parse_search_response(&cache)?;
    let items: Vec<Value> = iter_cluster_items(&resp, item_kind).cloned().collect();
    if index == 0 || index > items.len() {
        return Err(format!(
            "index {} out of range (search has {} results)",
            index,
            items.len()
        ));
    }
    Ok(items[index - 1].clone())
}

/// Extract the Nth post's activity URN from cached search results.
fn resolve_search_post_urn(index: usize) -> Result<String, String> {
    let sfu = nth_cached_item("posts", "searchFeedUpdate", index)?;
    let update = sfu.get("update").unwrap_or(&sfu);
    activity_urn_from_update(update)
        .ok_or_else(|| format!("search result {} has no activity URN", index))
}

/// Extract an activity URN (`urn:li:activity:N`) from an update value.
/// Tries `metadata.backendUrn`, then `entityUrn`, then `socialDetail.entityUrn`.
fn activity_urn_from_update(update: &Value) -> Option<String> {
    let from_metadata = update
        .get("metadata")
        .and_then(|m| m.get("backendUrn"))
        .and_then(|u| u.as_str())
        .and_then(coerce_activity_urn);
    let from_entity = update
        .get("entityUrn")
        .and_then(|u| u.as_str())
        .and_then(extract_activity_urn);
    let from_social = update
        .get("socialDetail")
        .and_then(|sd| sd.get("entityUrn"))
        .and_then(|u| u.as_str())
        .and_then(extract_activity_urn);
    from_metadata.or(from_entity).or(from_social)
}

/// Pass-through for `urn:li:activity:*` strings; otherwise delegate to
/// `extract_activity_urn` which handles `urn:li:fs_updateV2:*` etc.
fn coerce_activity_urn(s: &str) -> Option<String> {
    if s.starts_with("urn:li:activity:") {
        Some(s.to_string())
    } else {
        extract_activity_urn(s)
    }
}

/// Extract the Nth person's profile slug from cached people search results.
fn resolve_search_person_slug(index: usize) -> Result<String, String> {
    let entity = nth_cached_item("people", "entityResult", index)?;
    entity
        .get("navigationUrl")
        .and_then(|v| v.as_str())
        .and_then(profile_slug_from_url)
        .map(str::to_string)
        .ok_or_else(|| format!("search result {} has no profile URL", index))
}

/// Strip the "/in/" prefix and trailing query string from a navigation URL.
fn profile_slug_from_url(url: &str) -> Option<&str> {
    url.strip_prefix("https://www.linkedin.com/in/")
        .map(|rest| rest.split('?').next().unwrap_or(rest))
}

/// Read a `<key>.text` string field, the standard shape used by GraphQL
/// search responses for display strings.
fn text_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
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

    if let Err(e) = save_search_cache("posts", &value) {
        eprintln!("warning: failed to cache search results: {e}");
    }

    if raw_json {
        return print_json(&value);
    }

    let resp = parse_search_response(&value)?;
    print_search_header(&resp, &format!("Post results for '{}'", keywords));

    let posts: Vec<&Value> = iter_cluster_items(&resp, "searchFeedUpdate").collect();
    if posts.is_empty() {
        println!("(no results)");
        return Ok(());
    }
    for (i, sfu) in posts.iter().enumerate() {
        print_search_content(start as usize + i + 1, sfu);
        println!();
    }

    Ok(())
}

/// Handle `search react <index> [--type LIKE] [--json]`.
pub async fn cmd_search_react(
    index: usize,
    reaction_type: &str,
    raw_json: bool,
) -> Result<(), String> {
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
        print_json(&result)?;
    } else {
        println!("Reacted with {} to search result [{}]", rt_upper, index);
    }
    Ok(())
}

/// Handle `search view <index> [--json]`.
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
struct SearchPersonTarget {
    urn: Option<String>,
    slug: Option<String>,
    name: Option<String>,
}

impl SearchPersonTarget {
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

fn resolve_search_person_target(index: usize) -> Result<SearchPersonTarget, String> {
    let entity = nth_cached_item("people", "entityResult", index)?;
    let urn = profile_urn_from_entity(&entity);
    let slug = entity
        .get("navigationUrl")
        .and_then(|v| v.as_str())
        .and_then(profile_slug_from_url)
        .map(str::to_string);
    let name = text_field(&entity, "title").map(str::to_string);

    if urn.is_none() && slug.is_none() {
        return Err(format!(
            "search result {} has neither entityUrn nor navigationUrl",
            index
        ));
    }

    Ok(SearchPersonTarget { urn, slug, name })
}

/// Pull a `urn:li:fsd_profile:*` URN from an entity. Prefers `entityUrn`,
/// falls back to `trackingUrn`.
fn profile_urn_from_entity(entity: &Value) -> Option<String> {
    let take = |key: &str| {
        entity
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| s.starts_with("urn:li:fsd_profile:"))
            .map(|s| s.to_string())
    };
    take("entityUrn").or_else(|| take("trackingUrn"))
}

/// Print a human-readable summary of a content search result.
fn print_search_content(index: usize, sfu: &Value) {
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
    let (likes, comments) = social_counts(update);
    let activity_urn = activity_urn_from_update(update).unwrap_or_default();
    let permalink = sfu.get("permalink").and_then(|p| p.as_str()).unwrap_or("");

    println!("[{}] {}", index, actor_name);
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    println!("    likes: {}  comments: {}", likes, comments);
    if !activity_urn.is_empty() {
        let activity_id = activity_urn
            .strip_prefix("urn:li:activity:")
            .unwrap_or(&activity_urn);
        println!(
            "    https://www.linkedin.com/feed/update/urn:li:activity:{}",
            activity_id
        );
        println!("    URN: {}", activity_urn);
    } else if !permalink.is_empty() {
        println!("    {}", permalink);
    }
}

fn social_counts(update: &Value) -> (u64, u64) {
    let counts = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"));
    let likes = counts
        .and_then(|c| c.get("numLikes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let comments = counts
        .and_then(|c| c.get("numComments"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    (likes, comments)
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
