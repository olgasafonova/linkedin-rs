use chrono::Datelike;

use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

/// Handle `profile me [--json]`.
///
/// Loads the session, creates a client, calls GET /voyager/api/me, and
/// prints the result. With `--json`, outputs raw pretty-printed JSON.
/// Without `--json`, outputs a human-readable summary.
pub async fn cmd_profile_me(raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let me = client
        .get_me()
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&me).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        print_me_summary(&me);
    }

    Ok(())
}

/// Handle `profile view <public_id> [--json|--summary]`.
///
/// Loads the session, creates a client, calls the identity/profiles endpoint
/// with decoration for full field projection, and prints the result.
pub async fn cmd_profile_view(public_id: &str, raw_json: bool, summary: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let profile = client
        .get_profile(public_id)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if summary {
        let summary_value = extract_profile_summary(&profile);
        let pretty = serde_json::to_string_pretty(&summary_value)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else if raw_json {
        let pretty = serde_json::to_string_pretty(&profile)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
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
pub async fn cmd_profile_visit(public_id: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    eprintln!("Visiting profile '{}'...", public_id);
    let profile = client
        .visit_profile(public_id)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty = serde_json::to_string_pretty(&profile)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        // Extract basic info to confirm which profile was visited.
        let first = profile
            .get("firstName")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let last = profile
            .get("lastName")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let headline = profile
            .get("headline")
            .and_then(|v| v.as_str())
            .unwrap_or("");
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
pub async fn cmd_profile_viewers(raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_profile_viewers()
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
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
pub async fn cmd_profile_audit(raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // Get the user's public ID from /me, then fetch full profile.
    let me = client
        .get_me()
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    let mini = me.get("miniProfile");
    let public_id = mini
        .and_then(|m| m.get("publicIdentifier"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "could not determine your public profile ID".to_string())?;

    // Try full profile; fall back to /me data if LinkedIn's backend chokes.
    let (profile, full_profile) = match client.get_profile(public_id).await {
        Ok(p) => (p, true),
        Err(e) => {
            eprintln!(
                "warning: full profile fetch failed ({}), using basic /me data",
                e
            );
            // Build a minimal profile-like object from /me miniProfile.
            let fallback = mini.cloned().unwrap_or(serde_json::json!({}));
            (fallback, false)
        }
    };

    let mut findings: Vec<serde_json::Value> = Vec::new();
    let current_year = chrono::Utc::now().year() as u64;

    // Check: headline (in full profile it's "headline", in miniProfile it's "occupation")
    let headline = profile
        .get("headline")
        .and_then(|v| v.as_str())
        .or_else(|| profile.get("occupation").and_then(|v| v.as_str()))
        .unwrap_or("");
    if headline.is_empty() {
        findings.push(serde_json::json!({
            "field": "headline",
            "severity": "high",
            "message": "No headline set. This is the first thing people see."
        }));
    }

    // Check: about / summary
    let summary = profile
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if summary.is_empty() {
        findings.push(serde_json::json!({
            "field": "summary",
            "severity": "medium",
            "message": "Empty About section. Add a brief professional summary."
        }));
    } else if summary.len() < 50 {
        findings.push(serde_json::json!({
            "field": "summary",
            "severity": "low",
            "message": format!("About section is very short ({} chars). Consider expanding.", summary.len())
        }));
    }

    // The remaining checks require the full profile response.
    if !full_profile {
        findings.push(serde_json::json!({
            "field": "profile",
            "severity": "low",
            "message": "Full profile unavailable; positions, education, and connections checks skipped."
        }));
    }

    if full_profile {
        // Check: positions
        let position_groups = profile
            .get("profilePositionGroups")
            .and_then(|p| p.get("elements"))
            .and_then(|e| e.as_array());

        let mut has_current_role = false;
        let mut newest_end_year: Option<u64> = None;

        if let Some(groups) = position_groups {
            for group in groups {
                if let Some(pos_list) = group
                    .get("profilePositionInPositionGroup")
                    .and_then(|p| p.get("elements"))
                    .and_then(|e| e.as_array())
                {
                    for pos in pos_list {
                        let date_range = pos.get("dateRange");
                        let end_year = date_range
                            .and_then(|dr| dr.get("end"))
                            .and_then(|d| d.get("year"))
                            .and_then(|y| y.as_u64());
                        let start_year = date_range
                            .and_then(|dr| dr.get("start"))
                            .and_then(|d| d.get("year"))
                            .and_then(|y| y.as_u64());

                        if end_year.is_none() {
                            has_current_role = true;
                            if let Some(sy) = start_year {
                                if current_year.saturating_sub(sy) > 5 {
                                    let title = pos
                                        .get("title")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("(untitled)");
                                    findings.push(serde_json::json!({
                                        "field": "positions",
                                        "severity": "low",
                                        "message": format!("Current role '{}' started {} years ago. Confirm it's still accurate.", title, current_year - sy)
                                    }));
                                }
                            }
                        }

                        if let Some(ey) = end_year {
                            newest_end_year =
                                Some(newest_end_year.map_or(ey, |prev: u64| prev.max(ey)));
                        }
                    }
                }
            }

            if groups.is_empty() {
                findings.push(serde_json::json!({
                    "field": "positions",
                    "severity": "high",
                    "message": "No positions listed. Add your work experience."
                }));
            } else if !has_current_role {
                let stale_msg = if let Some(ey) = newest_end_year {
                    format!("No current position. Most recent role ended in {}.", ey)
                } else {
                    "No current position. Your profile may look inactive.".to_string()
                };
                findings.push(serde_json::json!({
                    "field": "positions",
                    "severity": "high",
                    "message": stale_msg
                }));
            }
        } else {
            findings.push(serde_json::json!({
                "field": "positions",
                "severity": "high",
                "message": "No positions listed. Add your work experience."
            }));
        }

        // Check: education
        let edu_list = profile
            .get("profileEducations")
            .and_then(|e| e.get("elements"))
            .and_then(|e| e.as_array());
        if edu_list.is_none_or(|l| l.is_empty()) {
            findings.push(serde_json::json!({
                "field": "education",
                "severity": "low",
                "message": "No education listed."
            }));
        }

        // Check: connection count
        let connections = profile
            .get("networkInfo")
            .and_then(|n| n.get("connectionsCount").and_then(|v| v.as_u64()))
            .or_else(|| profile.get("connectionsCount").and_then(|v| v.as_u64()));
        if let Some(count) = connections {
            if count < 50 {
                findings.push(serde_json::json!({
                    "field": "connections",
                    "severity": "low",
                    "message": format!("Only {} connections. Growing your network improves visibility.", count)
                }));
            }
        }

        // Check: location
        let has_location = profile
            .get("geoLocation")
            .and_then(|g| g.get("geo"))
            .and_then(|g| g.get("defaultLocalizedName"))
            .and_then(|v| v.as_str())
            .is_some();
        if !has_location {
            findings.push(serde_json::json!({
                "field": "location",
                "severity": "low",
                "message": "No location set. Recruiters and connections filter by location."
            }));
        }
    }

    if raw_json {
        let output = serde_json::json!({
            "publicId": public_id,
            "findings": findings,
            "score": if findings.is_empty() { "complete" } else { "needs_attention" }
        });
        let pretty =
            serde_json::to_string_pretty(&output).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // Human-readable output.
    println!("Profile Audit: {}", public_id);
    println!("===");

    if findings.is_empty() {
        println!("  All clear. No staleness signals detected.");
    } else {
        let high_count = findings.iter().filter(|f| f["severity"] == "high").count();
        let medium_count = findings
            .iter()
            .filter(|f| f["severity"] == "medium")
            .count();
        let low_count = findings.iter().filter(|f| f["severity"] == "low").count();

        for finding in &findings {
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

    Ok(())
}

/// Print a human-readable summary of the wvmpCards response.
///
/// The response has a deeply nested Rest.li union structure:
/// - `elements[].value["...WvmpViewersCard"].insightCards[]`
/// - Each insight card has `value["...WvmpSummaryInsightCard"]` with:
///   - `numViewsChangeInPercentage` -- week-over-week view change
///   - `cards[]` -- individual viewer entries, each with a union value
fn print_profile_viewers(data: &serde_json::Value) {
    let elements = match data.get("elements").and_then(|e| e.as_array()) {
        Some(arr) => arr,
        None => {
            println!("(no viewer data)");
            return;
        }
    };

    let mut viewer_index = 0;

    for element in elements {
        // Unwrap Rest.li union: value["com.linkedin.voyager.identity.me.wvmpOverview.WvmpViewersCard"]
        let viewers_card = element
            .get("value")
            .and_then(|v| v.get("com.linkedin.voyager.identity.me.wvmpOverview.WvmpViewersCard"));

        let viewers_card = match viewers_card {
            Some(c) => c,
            None => continue,
        };

        let insight_cards = match viewers_card.get("insightCards").and_then(|i| i.as_array()) {
            Some(arr) => arr,
            None => continue,
        };

        for insight_card in insight_cards {
            // Unwrap: value["...WvmpSummaryInsightCard"]
            let summary = insight_card.get("value").and_then(|v| {
                v.get("com.linkedin.voyager.identity.me.wvmpOverview.WvmpSummaryInsightCard")
            });

            let summary = match summary {
                Some(s) => s,
                None => continue,
            };

            // Print view change percentage header.
            let pct_change = summary
                .get("numViewsChangeInPercentage")
                .and_then(|n| n.as_f64());
            match pct_change {
                Some(pct) => {
                    let sign = if pct >= 0.0 { "+" } else { "" };
                    println!("Profile viewers (change: {}{}%)", sign, pct as i64);
                }
                None => {
                    println!("Profile viewers");
                }
            }
            println!("---");

            // Iterate individual viewer cards.
            let cards = match summary.get("cards").and_then(|c| c.as_array()) {
                Some(arr) => arr,
                None => continue,
            };

            for card in cards {
                let card_value = match card.get("value") {
                    Some(v) => v,
                    None => continue,
                };

                viewer_index += 1;

                // Case 1: Named viewer (WvmpProfileViewCard)
                if let Some(profile_card) =
                    card_value.get("com.linkedin.voyager.identity.me.WvmpProfileViewCard")
                {
                    let (name, occupation) = extract_viewer_profile(profile_card);
                    println!("[{}] {}", viewer_index, name);
                    if !occupation.is_empty() {
                        println!("    {}", occupation);
                    }
                    continue;
                }

                // Case 2: Private viewer (PrivateProfileViewer)
                if let Some(private_card) =
                    card_value.get("com.linkedin.voyager.identity.me.PrivateProfileViewer")
                {
                    let headline = private_card
                        .get("headline")
                        .and_then(|h| h.as_str())
                        .unwrap_or("Private viewer");
                    println!("[{}] (private) {}", viewer_index, headline);
                    continue;
                }

                // Case 3: Aggregated/generic (WvmpGenericCard)
                // The headline field is a TextViewModel with shape {text: "..."}.
                if let Some(generic_card) =
                    card_value.get("com.linkedin.voyager.identity.me.WvmpGenericCard")
                {
                    let text = generic_card
                        .get("headline")
                        .and_then(|h| h.get("text"))
                        .and_then(|t| t.as_str())
                        .or_else(|| generic_card.get("text").and_then(|t| t.as_str()))
                        .unwrap_or("Anonymous viewer(s)");
                    println!("[{}] (aggregated) {}", viewer_index, text);
                    continue;
                }

                // Case 4: Anonymous viewers (WvmpAnonymousProfileViewCard)
                if let Some(anon_card) =
                    card_value.get("com.linkedin.voyager.identity.me.WvmpAnonymousProfileViewCard")
                {
                    let num = anon_card
                        .get("numViewers")
                        .and_then(|n| n.as_u64())
                        .unwrap_or(1);
                    let label = if num == 1 {
                        "1 anonymous viewer".to_string()
                    } else {
                        format!("{} anonymous viewers", num)
                    };
                    println!("[{}] (anonymous) {}", viewer_index, label);
                    continue;
                }

                // Case 5: Premium upsell card -- skip, not a real viewer.
                if card_value
                    .get("com.linkedin.voyager.identity.me.WvmpPremiumUpsellCard")
                    .is_some()
                {
                    viewer_index -= 1; // don't count as a viewer entry
                    continue;
                }

                // Fallback: unknown card type -- print the union key.
                if let Some(obj) = card_value.as_object() {
                    let key = obj.keys().next().unwrap_or(&String::new()).clone();
                    println!("[{}] (unknown: {})", viewer_index, key);
                } else {
                    println!("[{}] (unknown card)", viewer_index);
                }
            }
        }
    }

    if viewer_index == 0 {
        println!("Profile viewers");
        println!("---");
        println!("(no viewers found)");
    }
}

/// Extract name and occupation from a WvmpProfileViewCard.
///
/// The viewer profile is nested under:
///   `viewer["com.linkedin.voyager.identity.me.FullProfileViewer"].profile.miniProfile`
/// or directly as `viewer.miniProfile`.
fn extract_viewer_profile(profile_card: &serde_json::Value) -> (String, String) {
    // Try the full union path first.
    let mini_profile = profile_card.get("viewer").and_then(|v| {
        v.get("com.linkedin.voyager.identity.me.FullProfileViewer")
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
fn print_profile_summary(profile: &serde_json::Value) {
    // Name.
    let first = profile
        .get("firstName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let last = profile
        .get("lastName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !first.is_empty() || !last.is_empty() {
        println!("Name: {} {}", first, last);
    }

    // Public identifier.
    if let Some(pub_id) = profile.get("publicIdentifier").and_then(|v| v.as_str()) {
        println!("Public ID: {}", pub_id);
    }

    // Headline.
    if let Some(headline) = profile.get("headline").and_then(|v| v.as_str()) {
        println!("Headline: {}", headline);
    }

    // Location -- Dash uses geoLocation.geo.defaultLocalizedName.
    let geo_name = profile
        .get("geoLocation")
        .and_then(|g| g.get("geo"))
        .and_then(|g| g.get("defaultLocalizedName"))
        .and_then(|v| v.as_str());
    if let Some(loc) = geo_name {
        println!("Location: {}", loc);
    }

    // Industry -- Dash uses industry.name.
    let industry_name = profile
        .get("industry")
        .and_then(|i| i.get("name"))
        .and_then(|v| v.as_str());
    if let Some(industry) = industry_name {
        println!("Industry: {}", industry);
    }

    // Summary / About.
    if let Some(summary) = profile.get("summary").and_then(|v| v.as_str()) {
        println!("About: {}", truncate_with_ellipsis(summary, 200));
    }

    // Connection/follower count -- may be in networkInfo, memberRelationship, or at top level.
    let connections = profile
        .get("networkInfo")
        .and_then(|n| n.get("connectionsCount").and_then(|v| v.as_u64()))
        .or_else(|| {
            profile
                .get("memberRelationship")
                .and_then(|m| m.get("connectionsCount").and_then(|v| v.as_u64()))
        })
        .or_else(|| profile.get("connectionsCount").and_then(|v| v.as_u64()));
    let followers = profile
        .get("networkInfo")
        .and_then(|n| n.get("followersCount").and_then(|v| v.as_u64()))
        .or_else(|| profile.get("followersCount").and_then(|v| v.as_u64()));

    if let Some(count) = connections {
        print!("Connections: {}", count);
        if let Some(f) = followers {
            print!("  |  Followers: {}", f);
        }
        println!();
    } else if let Some(f) = followers {
        println!("Followers: {}", f);
    }

    // Entity URN.
    if let Some(urn) = profile.get("entityUrn").and_then(|v| v.as_str()) {
        println!("URN: {}", urn);
    }

    // Positions -- Dash uses profilePositionGroups.elements[].profilePositionInPositionGroup.elements[].
    if let Some(groups) = profile
        .get("profilePositionGroups")
        .and_then(|p| p.get("elements"))
        .and_then(|e| e.as_array())
    {
        let mut printed_header = false;
        for group in groups {
            let positions = group
                .get("profilePositionInPositionGroup")
                .and_then(|p| p.get("elements"))
                .and_then(|e| e.as_array());
            if let Some(pos_list) = positions {
                for pos in pos_list {
                    if !printed_header {
                        println!("\nPositions:");
                        printed_header = true;
                    }
                    let title = pos.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let company = pos
                        .get("companyName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let period = format_date_range(pos.get("dateRange"));
                    if !title.is_empty() || !company.is_empty() {
                        println!("  - {} at {}{}", title, company, period);
                    }
                }
            }
        }
    }

    // Education -- Dash uses profileEducations.elements[].
    if let Some(edu_list) = profile
        .get("profileEducations")
        .and_then(|e| e.get("elements"))
        .and_then(|e| e.as_array())
    {
        if !edu_list.is_empty() {
            println!("\nEducation:");
            for edu in edu_list {
                let school = edu.get("schoolName").and_then(|v| v.as_str()).unwrap_or("");
                let degree = edu.get("degreeName").and_then(|v| v.as_str()).unwrap_or("");
                let field = edu
                    .get("fieldOfStudy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let period = format_date_range(edu.get("dateRange"));
                let degree_field = match (degree.is_empty(), field.is_empty()) {
                    (true, true) => String::new(),
                    (false, true) => format!(", {}", degree),
                    (true, false) => format!(", {}", field),
                    (false, false) => format!(", {} in {}", degree, field),
                };
                if !school.is_empty() {
                    println!("  - {}{}{}", school, degree_field, period);
                }
            }
        }
    }
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
fn print_me_summary(me: &serde_json::Value) {
    if let Some(mini) = me.get("miniProfile") {
        let first = mini.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
        let last = mini.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
        if !first.is_empty() || !last.is_empty() {
            println!("Name: {} {}", first, last);
        }

        if let Some(occ) = mini.get("occupation").and_then(|v| v.as_str()) {
            println!("Headline: {}", occ);
        }

        if let Some(urn) = mini.get("entityUrn").and_then(|v| v.as_str()) {
            println!("URN: {}", urn);
        }

        if let Some(vanity) = mini.get("publicIdentifier").and_then(|v| v.as_str()) {
            println!("Public ID: {}", vanity);
        }
    }

    if let Some(id) = me.get("plainId").and_then(|v| v.as_i64()) {
        println!("Member ID: {}", id);
    }

    if let Some(premium) = me.get("premiumSubscriber").and_then(|v| v.as_bool()) {
        println!("Premium: {}", if premium { "yes" } else { "no" });
    }

    // Print top-level keys for discoverability.
    if let Some(obj) = me.as_object() {
        let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        if !keys.is_empty() {
            println!("Response keys: {}", keys.join(", "));
        }
    }
}

/// Recursively search a JSON value for a string that starts with
/// `urn:li:fsd_profile:`. Returns the first match.
pub fn find_fsd_profile_urn(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) if s.starts_with("urn:li:fsd_profile:") => Some(s.clone()),
        serde_json::Value::Object(map) => map.values().find_map(find_fsd_profile_urn),
        serde_json::Value::Array(arr) => arr.iter().find_map(find_fsd_profile_urn),
        _ => None,
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
