use chrono::Datelike;
use serde_json::{json, Value};

use linkedin_api::client::LinkedInClient;
use linkedin_api::urn::SocialDetailUrn;

use crate::error::{CliError, CliResult};
use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

/// Pretty-print a JSON value to stdout.
fn print_json(value: &Value) -> CliResult<()> {
    let pretty = serde_json::to_string_pretty(value)?;
    println!("{}", pretty);
    Ok(())
}

/// Walk a nested object path, returning the final string if every step
/// resolves and the leaf is a string.
fn nested_text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    current.as_str()
}

/// Read a string field directly under `value`.
fn field_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

/// Handle `profile me [--json]`.
///
/// Loads the session, creates a client, calls GET /voyager/api/me, and
/// prints the result. With `--json`, outputs raw pretty-printed JSON.
/// Without `--json`, outputs a human-readable summary.
pub async fn cmd_profile_me(raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    let me = client.get_me().await?;

    if raw_json {
        print_json(&me)?;
    } else {
        print_me_summary(&me);
    }

    Ok(())
}

/// Handle `profile view <public_id> [--json|--summary]`.
///
/// Loads the session, creates a client, calls the identity/profiles endpoint
/// with decoration for full field projection, and prints the result.
pub async fn cmd_profile_view(public_id: &str, raw_json: bool, summary: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    let profile = client.get_profile(public_id).await?;

    if summary {
        print_json(&extract_profile_summary(&profile))?;
    } else if raw_json {
        print_json(&profile)?;
    } else {
        print_profile_summary(&profile);
    }

    Ok(())
}

/// Extract the high-signal fields from a `profile view` response.
///
/// Targets: machine-readable scripting (`--summary` flag) where the caller
/// only needs the identity + the connection-degree state. Returns a stable
/// shape regardless of which optional sub-objects are missing in the raw
/// response.
///
/// The relationship state is the key inside
/// `memberRelationship.memberRelationship` (typically `connection`,
/// `noConnection`, `invitationPending`, etc.). When the field is absent it
/// is reported as `null`.
fn extract_profile_summary(profile: &serde_json::Value) -> serde_json::Value {
    let first = profile
        .get("firstName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let last = profile
        .get("lastName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let full_name = match (first.is_empty(), last.is_empty()) {
        (true, true) => String::new(),
        (false, true) => first.to_string(),
        (true, false) => last.to_string(),
        (false, false) => format!("{} {}", first, last),
    };

    let relationship_state = profile
        .get("memberRelationship")
        .and_then(|v| v.get("memberRelationship"))
        .and_then(|v| v.as_object())
        .and_then(|o| o.keys().next().map(|s| s.as_str()))
        .map(|s| serde_json::Value::String(s.to_string()))
        .unwrap_or(serde_json::Value::Null);

    let follower_count = profile
        .get("profileProfileActions")
        .and_then(|v| v.get("overflowActionsResolutionResults"))
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .filter_map(|item| item.get("followingState"))
                .filter_map(|fs| fs.get("followerCount"))
                .next()
                .cloned()
        })
        .unwrap_or(serde_json::Value::Null);

    serde_json::json!({
        "publicIdentifier": profile.get("publicIdentifier").cloned().unwrap_or(serde_json::Value::Null),
        "entityUrn": profile.get("entityUrn").cloned().unwrap_or(serde_json::Value::Null),
        "objectUrn": profile.get("objectUrn").cloned().unwrap_or(serde_json::Value::Null),
        "name": full_name,
        "firstName": first,
        "lastName": last,
        "headline": profile.get("headline").cloned().unwrap_or(serde_json::Value::Null),
        "location": profile.get("locationName").cloned().unwrap_or(serde_json::Value::Null),
        "industry": profile.get("industry").cloned().unwrap_or(serde_json::Value::Null),
        "relationshipState": relationship_state,
        "followerCount": follower_count,
    })
}

/// Handle `profile visit <public_id> [--json]`.
///
/// Visits a profile so the target sees you in "who viewed my profile".
/// Uses the web client's GraphQL query ID which registers the view as a
/// side effect. See `re/profile_visit.md` for the mechanism.
pub async fn cmd_profile_visit(public_id: &str, raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    eprintln!("Visiting profile '{}'...", public_id);
    let profile = client.visit_profile(public_id).await?;

    if raw_json {
        print_json(&profile)?;
    } else {
        let first = profile
            .get("firstName")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let last = profile
            .get("lastName")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let headline = field_str(&profile, "headline");
        println!("Visited: {} {}", first, last);
        if !headline.is_empty() {
            println!("  {}", headline);
        }
        eprintln!("Profile view registered (target will see you in 'Who Viewed My Profile').");
    }

    Ok(())
}

/// Handle `profile viewers [--json]`.
///
/// Loads the session, calls GET /voyager/api/identity/wvmpCards, and prints
/// profile viewer data. The response uses deeply nested Rest.li union encoding.
pub async fn cmd_profile_viewers(raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    let value = client.get_profile_viewers().await?;

    if raw_json {
        return print_json(&value);
    }

    print_profile_viewers(&value);
    Ok(())
}

/// Handle `profile audit [--json]`.
///
/// Fetches the authenticated user's full profile and checks for common
/// staleness signals: missing headline, empty about section, stale
/// positions (no current role or current role older than 2 years without
/// updates), missing education, low connection count.
pub async fn cmd_profile_audit(raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    let me = client.get_me().await?;

    let mini = me.get("miniProfile");
    let public_id = mini
        .and_then(|m| m.get("publicIdentifier"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| CliError::Other("could not determine your public profile ID".to_string()))?;

    // Try full profile; fall back to /me data if LinkedIn's backend chokes.
    let (profile, full_profile) = match client.get_profile(public_id).await {
        Ok(p) => (p, true),
        Err(e) => {
            eprintln!(
                "warning: full profile fetch failed ({}), using basic /me data",
                e
            );
            let fallback = mini.cloned().unwrap_or(serde_json::json!({}));
            (fallback, false)
        }
    };

    let findings = collect_audit_findings(&profile, full_profile);

    if raw_json {
        return print_audit_json(public_id, &findings);
    }
    print_audit_report(public_id, &findings);
    Ok(())
}

fn collect_audit_findings(
    profile: &serde_json::Value,
    full_profile: bool,
) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    let current_year = chrono::Utc::now().year() as u64;

    if let Some(f) = check_headline(profile) {
        findings.push(f);
    }
    if let Some(f) = check_summary(profile) {
        findings.push(f);
    }

    if !full_profile {
        findings.push(serde_json::json!({
            "field": "profile",
            "severity": "low",
            "message": "Full profile unavailable; positions, education, and connections checks skipped."
        }));
        return findings;
    }

    findings.extend(check_positions(profile, current_year));
    if let Some(f) = check_education(profile) {
        findings.push(f);
    }
    if let Some(f) = check_connections(profile) {
        findings.push(f);
    }
    if let Some(f) = check_location(profile) {
        findings.push(f);
    }
    findings
}

/// Headline lives at `headline` in the full profile and `occupation` in the
/// miniProfile fallback.
fn check_headline(profile: &serde_json::Value) -> Option<serde_json::Value> {
    let headline = profile
        .get("headline")
        .and_then(|v| v.as_str())
        .or_else(|| profile.get("occupation").and_then(|v| v.as_str()))
        .unwrap_or("");
    if !headline.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "field": "headline",
        "severity": "high",
        "message": "No headline set. This is the first thing people see."
    }))
}

fn check_summary(profile: &serde_json::Value) -> Option<serde_json::Value> {
    let summary = profile
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if summary.is_empty() {
        return Some(serde_json::json!({
            "field": "summary",
            "severity": "medium",
            "message": "Empty About section. Add a brief professional summary."
        }));
    }
    if summary.len() < 50 {
        return Some(serde_json::json!({
            "field": "summary",
            "severity": "low",
            "message": format!("About section is very short ({} chars). Consider expanding.", summary.len())
        }));
    }
    None
}

fn check_positions(profile: &serde_json::Value, current_year: u64) -> Vec<serde_json::Value> {
    let positions: Vec<&serde_json::Value> = iter_positions(profile).collect();
    if positions.is_empty() {
        return vec![no_positions_finding()];
    }

    let summary = summarize_positions(&positions, current_year);
    let mut findings = summary.findings;
    if !summary.has_current_role {
        findings.push(no_current_role_finding(summary.newest_end_year));
    }
    findings
}

struct PositionsSummary {
    findings: Vec<serde_json::Value>,
    has_current_role: bool,
    newest_end_year: Option<u64>,
}

fn summarize_positions(positions: &[&serde_json::Value], current_year: u64) -> PositionsSummary {
    let mut summary = PositionsSummary {
        findings: Vec::new(),
        has_current_role: false,
        newest_end_year: None,
    };
    for pos in positions {
        ingest_position(&mut summary, pos, current_year);
    }
    summary
}

fn ingest_position(summary: &mut PositionsSummary, pos: &serde_json::Value, current_year: u64) {
    let date_range = pos.get("dateRange");
    let end_year = position_year(date_range, "end");
    let start_year = position_year(date_range, "start");

    match end_year {
        None => {
            summary.has_current_role = true;
            if let Some(f) = stale_current_role_finding(pos, start_year, current_year) {
                summary.findings.push(f);
            }
        }
        Some(ey) => {
            summary.newest_end_year =
                Some(summary.newest_end_year.map_or(ey, |prev: u64| prev.max(ey)));
        }
    }
}

fn position_year(date_range: Option<&serde_json::Value>, side: &str) -> Option<u64> {
    date_range
        .and_then(|dr| dr.get(side))
        .and_then(|d| d.get("year"))
        .and_then(|y| y.as_u64())
}

fn no_positions_finding() -> serde_json::Value {
    serde_json::json!({
        "field": "positions",
        "severity": "high",
        "message": "No positions listed. Add your work experience."
    })
}

fn no_current_role_finding(newest_end_year: Option<u64>) -> serde_json::Value {
    let msg = match newest_end_year {
        Some(ey) => format!("No current position. Most recent role ended in {}.", ey),
        None => "No current position. Your profile may look inactive.".to_string(),
    };
    serde_json::json!({
        "field": "positions",
        "severity": "high",
        "message": msg
    })
}

/// Flatten the two-level `profilePositionGroups[].profilePositionInPositionGroup[]`
/// nesting into a single iterator over individual positions.
fn iter_positions(profile: &serde_json::Value) -> impl Iterator<Item = &serde_json::Value> {
    profile
        .get("profilePositionGroups")
        .and_then(|p| p.get("elements"))
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .flat_map(|group| {
            group
                .get("profilePositionInPositionGroup")
                .and_then(|p| p.get("elements"))
                .and_then(|e| e.as_array())
                .into_iter()
                .flatten()
        })
}

fn stale_current_role_finding(
    pos: &serde_json::Value,
    start_year: Option<u64>,
    current_year: u64,
) -> Option<serde_json::Value> {
    let sy = start_year?;
    if current_year.saturating_sub(sy) <= 5 {
        return None;
    }
    let title = pos
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)");
    Some(serde_json::json!({
        "field": "positions",
        "severity": "low",
        "message": format!("Current role '{}' started {} years ago. Confirm it's still accurate.", title, current_year - sy)
    }))
}

fn check_education(profile: &serde_json::Value) -> Option<serde_json::Value> {
    let edu_list = profile
        .get("profileEducations")
        .and_then(|e| e.get("elements"))
        .and_then(|e| e.as_array());
    if edu_list.is_some_and(|l| !l.is_empty()) {
        return None;
    }
    Some(serde_json::json!({
        "field": "education",
        "severity": "low",
        "message": "No education listed."
    }))
}

fn check_connections(profile: &serde_json::Value) -> Option<serde_json::Value> {
    let count = profile
        .get("networkInfo")
        .and_then(|n| n.get("connectionsCount").and_then(|v| v.as_u64()))
        .or_else(|| profile.get("connectionsCount").and_then(|v| v.as_u64()))?;
    if count >= 50 {
        return None;
    }
    Some(serde_json::json!({
        "field": "connections",
        "severity": "low",
        "message": format!("Only {} connections. Growing your network improves visibility.", count)
    }))
}

fn check_location(profile: &serde_json::Value) -> Option<serde_json::Value> {
    let has_location = profile
        .get("geoLocation")
        .and_then(|g| g.get("geo"))
        .and_then(|g| g.get("defaultLocalizedName"))
        .and_then(|v| v.as_str())
        .is_some();
    if has_location {
        return None;
    }
    Some(serde_json::json!({
        "field": "location",
        "severity": "low",
        "message": "No location set. Recruiters and connections filter by location."
    }))
}

fn print_audit_json(public_id: &str, findings: &[Value]) -> CliResult<()> {
    let output = serde_json::json!({
        "publicId": public_id,
        "findings": findings,
        "score": if findings.is_empty() { "complete" } else { "needs_attention" }
    });
    print_json(&output)
}

fn print_audit_report(public_id: &str, findings: &[serde_json::Value]) {
    println!("Profile Audit: {}", public_id);
    println!("===");

    if findings.is_empty() {
        println!("  All clear. No staleness signals detected.");
        return;
    }

    let high_count = findings.iter().filter(|f| f["severity"] == "high").count();
    let medium_count = findings
        .iter()
        .filter(|f| f["severity"] == "medium")
        .count();
    let low_count = findings.iter().filter(|f| f["severity"] == "low").count();

    for finding in findings {
        let severity = finding["severity"].as_str().unwrap_or("?");
        let msg = finding["message"].as_str().unwrap_or("");
        let marker = match severity {
            "high" => "!!",
            "medium" => " !",
            _ => "  ",
        };
        println!("  {} {}", marker, msg);
    }

    println!();
    println!(
        "  {} issue(s): {} high, {} medium, {} low",
        findings.len(),
        high_count,
        medium_count,
        low_count
    );
}

// Rest.li union keys used by the wvmpCards response — all live in
// linkedin_api::restli so the typename strings aren't duplicated.
use linkedin_api::restli::{
    FULL_PROFILE_VIEWER, WVMP_ANON_CARD, WVMP_GENERIC_CARD, WVMP_PREMIUM_UPSELL,
    WVMP_PRIVATE_VIEWER, WVMP_PROFILE_VIEW, WVMP_SUMMARY_CARD, WVMP_VIEWERS_CARD,
};

/// Print a human-readable summary of the wvmpCards response.
///
/// The response has a deeply nested Rest.li union structure:
/// - `elements[].value["...WvmpViewersCard"].insightCards[]`
/// - Each insight card has `value["...WvmpSummaryInsightCard"]` with:
///   - `numViewsChangeInPercentage` -- week-over-week view change
///   - `cards[]` -- individual viewer entries, each with a union value
fn print_profile_viewers(data: &Value) {
    let Some(elements) = data.get("elements").and_then(|e| e.as_array()) else {
        println!("(no viewer data)");
        return;
    };

    let mut viewer_index = 0;
    for summary in iter_wvmp_summary_cards(elements) {
        print_view_change_header(summary);
        let cards = summary
            .get("cards")
            .and_then(|c| c.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for card in cards {
            let Some(card_value) = card.get("value") else {
                continue;
            };
            viewer_index += 1;
            if !print_wvmp_viewer_card(viewer_index, card_value) {
                viewer_index -= 1; // upsell or unrecognised, don't count
            }
        }
    }

    if viewer_index == 0 {
        println!("Profile viewers");
        println!("---");
        println!("(no viewers found)");
    }
}

/// Walk wvmp `elements` -> `WvmpViewersCard.insightCards[].WvmpSummaryInsightCard`.
fn iter_wvmp_summary_cards(elements: &[Value]) -> impl Iterator<Item = &Value> {
    elements
        .iter()
        .filter_map(|el| el.get("value").and_then(|v| v.get(WVMP_VIEWERS_CARD)))
        .filter_map(|vc| vc.get("insightCards").and_then(|i| i.as_array()))
        .flatten()
        .filter_map(|ic| ic.get("value").and_then(|v| v.get(WVMP_SUMMARY_CARD)))
}

fn print_view_change_header(summary: &Value) {
    let pct = summary
        .get("numViewsChangeInPercentage")
        .and_then(|n| n.as_f64());
    match pct {
        Some(p) => {
            let sign = if p >= 0.0 { "+" } else { "" };
            println!("Profile viewers (change: {}{}%)", sign, p as i64);
        }
        None => println!("Profile viewers"),
    }
    println!("---");
}

/// Render a `WvmpProfileViewCard`: a real named viewer entry.
fn render_profile_view(index: usize, profile_card: &Value) {
    let (name, occupation) = extract_viewer_profile(profile_card);
    println!("[{}] {}", index, name);
    if !occupation.is_empty() {
        println!("    {}", occupation);
    }
}

/// Render a `WvmpPrivateViewerCard`: a private viewer with only a headline.
fn render_private_viewer(index: usize, private_card: &Value) {
    let headline = private_card
        .get("headline")
        .and_then(|h| h.as_str())
        .unwrap_or("Private viewer");
    println!("[{}] (private) {}", index, headline);
}

/// Render a `WvmpGenericCard`: an aggregated/generic viewer entry.
fn render_generic(index: usize, generic_card: &Value) {
    let text = nested_text(generic_card, &["headline", "text"])
        .or_else(|| generic_card.get("text").and_then(|t| t.as_str()))
        .unwrap_or("Anonymous viewer(s)");
    println!("[{}] (aggregated) {}", index, text);
}

/// Render a `WvmpAnonCard`: a count of anonymous viewers.
fn render_anon(index: usize, anon_card: &Value) {
    let num = anon_card
        .get("numViewers")
        .and_then(|n| n.as_u64())
        .unwrap_or(1);
    let label = if num == 1 {
        "1 anonymous viewer".to_string()
    } else {
        format!("{} anonymous viewers", num)
    };
    println!("[{}] (anonymous) {}", index, label);
}

/// Render a card whose union key isn't one of the recognised WVMP variants.
/// Logs the first key when the value is a JSON object, or a generic label
/// otherwise. Used as the dispatch fallback.
fn render_unknown(index: usize, card_value: &Value) {
    if let Some(obj) = card_value.as_object() {
        let key = obj.keys().next().cloned().unwrap_or_default();
        println!("[{}] (unknown: {})", index, key);
    } else {
        println!("[{}] (unknown card)", index);
    }
}

/// Render a single viewer card. Returns true if the entry was countable
/// (a real viewer), false for upsell/skipped cards.
///
/// Dispatches to one of `render_profile_view`, `render_private_viewer`,
/// `render_generic`, `render_anon`, or `render_unknown` based on which
/// Rest.li union key the card carries. `WVMP_PREMIUM_UPSELL` is the only
/// recognised but non-countable variant.
fn print_wvmp_viewer_card(index: usize, card_value: &Value) -> bool {
    if let Some(profile_card) = card_value.get(WVMP_PROFILE_VIEW) {
        render_profile_view(index, profile_card);
        return true;
    }
    if let Some(private_card) = card_value.get(WVMP_PRIVATE_VIEWER) {
        render_private_viewer(index, private_card);
        return true;
    }
    if let Some(generic_card) = card_value.get(WVMP_GENERIC_CARD) {
        render_generic(index, generic_card);
        return true;
    }
    if let Some(anon_card) = card_value.get(WVMP_ANON_CARD) {
        render_anon(index, anon_card);
        return true;
    }
    if card_value.get(WVMP_PREMIUM_UPSELL).is_some() {
        return false;
    }
    render_unknown(index, card_value);
    true
}

/// Extract name and occupation from a WvmpProfileViewCard.
///
/// The viewer profile is nested under:
///   `viewer["com.linkedin.voyager.identity.me.FullProfileViewer"].profile.miniProfile`
/// or directly as `viewer.miniProfile`.
fn extract_viewer_profile(profile_card: &serde_json::Value) -> (String, String) {
    // Try the full union path first.
    let mini_profile = profile_card.get("viewer").and_then(|v| {
        v.get(FULL_PROFILE_VIEWER)
            .and_then(|fp| fp.get("profile"))
            .and_then(|p| p.get("miniProfile"))
            .or_else(|| v.get("miniProfile"))
            .or_else(|| v.get("profile").and_then(|p| p.get("miniProfile")))
    });

    let (name, occupation) = if let Some(mp) = mini_profile {
        let first = mp.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
        let last = mp.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
        let occ = mp.get("occupation").and_then(|v| v.as_str()).unwrap_or("");
        let full_name = format!("{} {}", first, last).trim().to_string();
        (full_name, occ.to_string())
    } else {
        // Fallback: try top-level fields.
        let name = profile_card
            .get("viewerName")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown viewer)")
            .to_string();
        let occ = profile_card
            .get("headline")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (name, occ)
    };

    let display_name = if name.is_empty() {
        "(unknown viewer)".to_string()
    } else {
        name
    };

    (display_name, occupation)
}

/// Print a human-readable summary of a Dash profile response.
///
/// The response comes from the `identityDashProfilesByMemberIdentity` GraphQL
/// query, unwrapped to the first element. Field names differ from the legacy
/// REST endpoint (e.g., `profilePositionGroups` instead of `positions`,
/// `dateRange` with `start`/`end` instead of `timePeriod` with
/// `startDate`/`endDate`).
fn print_profile_summary(profile: &Value) {
    print_summary_name(profile);
    print_summary_simple_fields(profile);
    print_summary_about(profile);
    print_summary_network(profile);
    print_summary_urn(profile);
    print_summary_positions(profile);
    print_summary_education(profile);
}

fn print_summary_name(profile: &Value) {
    let first = field_str(profile, "firstName");
    let last = field_str(profile, "lastName");
    if !first.is_empty() || !last.is_empty() {
        println!("Name: {} {}", first, last);
    }
}

/// Print the single-line, top-level fields: public ID, headline, location,
/// industry. Each prints only if present.
fn print_summary_simple_fields(profile: &Value) {
    if let Some(pub_id) = profile.get("publicIdentifier").and_then(|v| v.as_str()) {
        println!("Public ID: {}", pub_id);
    }
    if let Some(headline) = profile.get("headline").and_then(|v| v.as_str()) {
        println!("Headline: {}", headline);
    }
    if let Some(loc) = nested_text(profile, &["geoLocation", "geo", "defaultLocalizedName"]) {
        println!("Location: {}", loc);
    }
    if let Some(industry) = nested_text(profile, &["industry", "name"]) {
        println!("Industry: {}", industry);
    }
}

fn print_summary_about(profile: &Value) {
    if let Some(summary) = profile.get("summary").and_then(|v| v.as_str()) {
        println!("About: {}", truncate_with_ellipsis(summary, 200));
    }
}

fn print_summary_network(profile: &Value) {
    let connections = connection_count(profile);
    let followers = follower_count(profile);
    match (connections, followers) {
        (Some(c), Some(f)) => println!("Connections: {}  |  Followers: {}", c, f),
        (Some(c), None) => println!("Connections: {}", c),
        (None, Some(f)) => println!("Followers: {}", f),
        (None, None) => {}
    }
}

/// Try `networkInfo.connectionsCount`, then `memberRelationship.connectionsCount`,
/// then top-level `connectionsCount`.
fn connection_count(profile: &Value) -> Option<u64> {
    profile
        .get("networkInfo")
        .and_then(|n| n.get("connectionsCount"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            profile
                .get("memberRelationship")
                .and_then(|m| m.get("connectionsCount"))
                .and_then(|v| v.as_u64())
        })
        .or_else(|| profile.get("connectionsCount").and_then(|v| v.as_u64()))
}

fn follower_count(profile: &Value) -> Option<u64> {
    profile
        .get("networkInfo")
        .and_then(|n| n.get("followersCount"))
        .and_then(|v| v.as_u64())
        .or_else(|| profile.get("followersCount").and_then(|v| v.as_u64()))
}

fn print_summary_urn(profile: &Value) {
    if let Some(urn) = profile.get("entityUrn").and_then(|v| v.as_str()) {
        println!("URN: {}", urn);
    }
}

fn print_summary_positions(profile: &Value) {
    let mut printed_header = false;
    for pos in iter_positions(profile) {
        let title = field_str(pos, "title");
        let company = field_str(pos, "companyName");
        if title.is_empty() && company.is_empty() {
            continue;
        }
        if !printed_header {
            println!("\nPositions:");
            printed_header = true;
        }
        let period = format_date_range(pos.get("dateRange"));
        println!("  - {} at {}{}", title, company, period);
    }
}

fn print_summary_education(profile: &Value) {
    let educations: Vec<&Value> = iter_educations(profile).collect();
    if educations.is_empty() {
        return;
    }
    println!("\nEducation:");
    for edu in educations {
        print_education_entry(edu);
    }
}

/// Iterate over `profileEducations.elements`. Empty when missing.
fn iter_educations(profile: &Value) -> impl Iterator<Item = &Value> {
    profile
        .get("profileEducations")
        .and_then(|e| e.get("elements"))
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
}

fn print_education_entry(edu: &Value) {
    let school = field_str(edu, "schoolName");
    if school.is_empty() {
        return;
    }
    let degree = field_str(edu, "degreeName");
    let field = field_str(edu, "fieldOfStudy");
    let period = format_date_range(edu.get("dateRange"));
    let degree_field = match (degree.is_empty(), field.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!(", {}", degree),
        (true, false) => format!(", {}", field),
        (false, false) => format!(", {} in {}", degree, field),
    };
    println!("  - {}{}{}", school, degree_field, period);
}

/// Format a `dateRange` object into a human-readable string like " (2020 - 2023)".
///
/// The Dash API uses `dateRange` with shape `{ "start": { "year": N, "month": N }, "end": ... }`.
/// Also handles the legacy `timePeriod` shape with `startDate`/`endDate`.
/// Returns an empty string if the input is `None` or lacks date fields.
fn format_date_range(date_range: Option<&serde_json::Value>) -> String {
    let dr = match date_range {
        Some(v) => v,
        None => return String::new(),
    };

    // Dash format: start/end
    let start_year = dr
        .get("start")
        .and_then(|d| d.get("year"))
        .and_then(|y| y.as_u64())
        // Legacy format: startDate/endDate
        .or_else(|| {
            dr.get("startDate")
                .and_then(|d| d.get("year"))
                .and_then(|y| y.as_u64())
        });
    let end_year = dr
        .get("end")
        .and_then(|d| d.get("year"))
        .and_then(|y| y.as_u64())
        .or_else(|| {
            dr.get("endDate")
                .and_then(|d| d.get("year"))
                .and_then(|y| y.as_u64())
        });

    match (start_year, end_year) {
        (Some(s), Some(e)) => format!(" ({} - {})", s, e),
        (Some(s), None) => format!(" ({} - present)", s),
        (None, Some(e)) => format!(" (? - {})", e),
        (None, None) => String::new(),
    }
}

/// Print a human-readable summary of the /voyager/api/me response.
///
/// Extracts known fields from the response and prints them. The exact
/// response structure depends on LinkedIn's API version, so this is
/// best-effort. Unknown fields are skipped rather than causing errors.
fn print_me_summary(me: &Value) {
    if let Some(mini) = me.get("miniProfile") {
        print_me_mini(mini);
    }
    if let Some(id) = me.get("plainId").and_then(|v| v.as_i64()) {
        println!("Member ID: {}", id);
    }
    if let Some(premium) = me.get("premiumSubscriber").and_then(|v| v.as_bool()) {
        println!("Premium: {}", if premium { "yes" } else { "no" });
    }
    print_me_response_keys(me);
}

fn print_me_mini(mini: &Value) {
    let first = field_str(mini, "firstName");
    let last = field_str(mini, "lastName");
    if !first.is_empty() || !last.is_empty() {
        println!("Name: {} {}", first, last);
    }
    print_labeled_field(mini, "occupation", "Headline");
    print_labeled_field(mini, "entityUrn", "URN");
    print_labeled_field(mini, "publicIdentifier", "Public ID");
}

fn print_labeled_field(value: &Value, key: &str, label: &str) {
    if let Some(v) = value.get(key).and_then(|v| v.as_str()) {
        println!("{}: {}", label, v);
    }
}

fn print_me_response_keys(me: &Value) {
    let Some(obj) = me.as_object() else { return };
    let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    if !keys.is_empty() {
        println!("Response keys: {}", keys.join(", "));
    }
}

// `find_fsd_profile_urn` moved to `linkedin_api::urn`. Callers that used to
// import it via `crate::profile::find_fsd_profile_urn` now import directly
// from `linkedin_api::urn::find_fsd_profile_urn`.

// ===========================================================================
// `profile posts` subcommand
// ===========================================================================

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
        for post in posts.iter_mut() {
            if let Some(comment_data) = fetch_first_comment_by_author(&client, post).await {
                if let Some(obj) = post.as_object_mut() {
                    obj.insert("firstCommentByAuthor".to_string(), comment_data);
                }
            }
        }
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
    let posted_relative = nested_text(update, &["actor", "subDescription", "text"])
        .unwrap_or("")
        .trim()
        .trim_end_matches("• \u{a0}\u{a0}")
        .trim_end_matches('•')
        .trim()
        .to_string();

    let permalink = if activity_urn.is_empty() {
        String::new()
    } else {
        format!("https://www.linkedin.com/feed/update/{}", activity_urn)
    };

    let article_url = nested_text(
        update,
        &[
            "content",
            "com.linkedin.voyager.feed.render.ArticleComponent",
            "navigationContext",
            "actionTarget",
        ],
    )
    .unwrap_or("")
    .to_string();
    let external_video_url = nested_text(
        update,
        &[
            "content",
            "com.linkedin.voyager.feed.render.ExternalVideoComponent",
            "navigationContext",
            "actionTarget",
        ],
    )
    .unwrap_or("")
    .to_string();

    let inline_urls = extract_hyperlink_attrs(update.get("commentary").and_then(|c| c.get("text")));

    // Priority: dashEntityUrn (urn:li:fsd_socialDetail:..., the modern Dash
    // URN that socialDashCommentsBySocialDetail accepts) → entityUrn (legacy
    // fs_socialDetail) → urn (the underlying ugcPost/activity URN; LinkedIn
    // rejects this with "Deserializing failed").
    let social_detail_urn = nested_text(update, &["socialDetail", "dashEntityUrn"])
        .or_else(|| nested_text(update, &["socialDetail", "entityUrn"]))
        .or_else(|| nested_text(update, &["socialDetail", "urn"]))
        .unwrap_or("")
        .to_string();

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

fn extract_hyperlink_attrs(text_obj: Option<&Value>) -> Vec<String> {
    text_obj
        .and_then(|t| t.get("attributes"))
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|attr| attr.get("type").and_then(|t| t.as_str()) == Some("HYPERLINK"))
                .filter_map(|attr| attr.get("url").and_then(|u| u.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
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
            i + 1,
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
        if let Some(url) = post.get("articleUrl").and_then(|v| v.as_str()) {
            println!("    article: {}", url);
        }
        if let Some(url) = post.get("externalVideoUrl").and_then(|v| v.as_str()) {
            println!("    video: {}", url);
        }
        if let Some(arr) = post.get("inlineUrls").and_then(|v| v.as_array()) {
            for url in arr.iter().filter_map(|u| u.as_str()) {
                println!("    inline: {}", url);
            }
        }
        if let Some(comment) = post
            .get("firstCommentByAuthor")
            .filter(|v| !v.is_null())
            .and_then(|v| v.as_object())
        {
            if let Some(arr) = comment.get("extractedUrls").and_then(|v| v.as_array()) {
                for url in arr.iter().filter_map(|u| u.as_str()) {
                    println!("    first-comment-link: {}", url);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn profile_summary_extracts_first_degree_connection() {
        let profile = json!({
            "publicIdentifier": "jane-doe",
            "entityUrn": "urn:li:fsd_profile:ACoAAA111",
            "objectUrn": "urn:li:member:1",
            "firstName": "Jane",
            "lastName": "Doe",
            "headline": "Engineer",
            "locationName": "Copenhagen",
            "industry": "Software",
            "memberRelationship": {
                "memberRelationship": {
                    "connection": {"foo": "bar"}
                }
            },
            "profileProfileActions": {
                "overflowActionsResolutionResults": [
                    {"shareProfileUrl": "..."},
                    {"followingState": {"followerCount": 1234}}
                ]
            }
        });

        let s = extract_profile_summary(&profile);
        assert_eq!(s["name"], "Jane Doe");
        assert_eq!(s["firstName"], "Jane");
        assert_eq!(s["headline"], "Engineer");
        assert_eq!(s["location"], "Copenhagen");
        assert_eq!(s["relationshipState"], "connection");
        assert_eq!(s["followerCount"], 1234);
        assert_eq!(s["publicIdentifier"], "jane-doe");
    }

    #[test]
    fn profile_summary_handles_no_connection() {
        let profile = json!({
            "publicIdentifier": "stranger",
            "firstName": "S",
            "lastName": "T",
            "memberRelationship": {
                "memberRelationship": {
                    "noConnection": {}
                }
            }
        });
        let s = extract_profile_summary(&profile);
        assert_eq!(s["relationshipState"], "noConnection");
        assert_eq!(s["name"], "S T");
    }

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

    #[test]
    fn profile_summary_tolerates_missing_fields() {
        // Minimal response with only firstName -- nothing should panic; missing
        // fields should serialize as JSON null, not be silently dropped.
        let profile = json!({
            "firstName": "Solo"
        });
        let s = extract_profile_summary(&profile);
        assert_eq!(s["name"], "Solo");
        assert_eq!(s["headline"], serde_json::Value::Null);
        assert_eq!(s["relationshipState"], serde_json::Value::Null);
        assert_eq!(s["followerCount"], serde_json::Value::Null);
    }
}
