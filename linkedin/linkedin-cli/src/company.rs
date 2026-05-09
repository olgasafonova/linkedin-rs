use crate::session::load_session_client;
use crate::util::truncate_with_ellipsis;

/// Handle `company view <slug> [--json]`.
///
/// Fetches company info by universal name (URL slug) and prints a summary.
pub async fn cmd_company_view(slug: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let company = client
        .get_company(slug)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty = serde_json::to_string_pretty(&company)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    print_company_summary(&company);
    Ok(())
}

/// Handle `company followers <slug> [--count N] [--start N] [--json]`.
///
/// First resolves the slug to a company ID, then fetches followers.
/// Falls back to showing first-degree connections that follow if the
/// admin follower endpoint is unavailable.
pub async fn cmd_company_followers(
    slug: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // First get company info to extract the numeric ID.
    let company = client
        .get_company(slug)
        .await
        .map_err(|e| format!("failed to fetch company: {e}"))?;

    let company_name = company.get("name").and_then(|v| v.as_str()).unwrap_or(slug);

    // Extract numeric company ID from entityUrn.
    let company_id = company
        .get("entityUrn")
        .and_then(|v| v.as_str())
        .and_then(|urn| urn.rsplit(':').next())
        .or_else(|| {
            company
                .get("companyId")
                .and_then(|v| v.as_u64())
                .map(|_| "")
        })
        .ok_or_else(|| "could not extract company ID from response".to_string())?;

    // Try the admin follower endpoints.
    match client.get_company_followers(company_id, start, count).await {
        Ok(value) => {
            if raw_json {
                let pretty = serde_json::to_string_pretty(&value)
                    .map_err(|e| format!("JSON format error: {e}"))?;
                println!("{}", pretty);
                return Ok(());
            }

            // Try to print follower elements.
            if let Some(elements) = value.get("elements").and_then(|e| e.as_array()) {
                let follower_count = company
                    .get("followingInfo")
                    .and_then(|f| f.get("followerCount"))
                    .and_then(|v| v.as_u64());

                println!(
                    "Followers of {} (showing {}, total {})",
                    company_name,
                    elements.len(),
                    follower_count
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".to_string())
                );
                println!("---");

                for (i, el) in elements.iter().enumerate() {
                    let idx = start as usize + i + 1;
                    // Follower analytics elements may have different shapes.
                    let name = el
                        .get("follower")
                        .and_then(|f| {
                            let first = f.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
                            let last = f.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
                            if first.is_empty() && last.is_empty() {
                                None
                            } else {
                                Some(format!("{} {}", first, last))
                            }
                        })
                        .or_else(|| {
                            el.get("miniProfile").and_then(|mp| {
                                let first =
                                    mp.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
                                let last =
                                    mp.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
                                if first.is_empty() && last.is_empty() {
                                    None
                                } else {
                                    Some(format!("{} {}", first, last))
                                }
                            })
                        })
                        .unwrap_or_else(|| "(unknown)".to_string());

                    println!("[{}] {}", idx, name.trim());
                }

                if elements.is_empty() {
                    println!("(no follower details available)");
                }
            } else {
                // Response might be analytics/stats rather than a list.
                println!("Follower analytics for {}:", company_name);
                let pretty = serde_json::to_string_pretty(&value)
                    .map_err(|e| format!("JSON format error: {e}"))?;
                println!("{}", pretty);
            }
            Ok(())
        }
        Err(_) => {
            // Fall back to first-degree connections that follow.
            let first_degree = company
                .get("firstDegreeConnectionsThatFollow")
                .and_then(|v| v.as_array());

            let follower_count = company
                .get("followingInfo")
                .and_then(|f| f.get("followerCount"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            if raw_json {
                let data = serde_json::json!({
                    "followerCount": follower_count,
                    "firstDegreeConnectionsThatFollow": first_degree,
                });
                let pretty = serde_json::to_string_pretty(&data)
                    .map_err(|e| format!("JSON format error: {e}"))?;
                println!("{}", pretty);
                return Ok(());
            }

            println!("Followers of {} (total: {})", company_name, follower_count);
            println!("---");
            eprintln!(
                "Note: Full follower list requires admin API access. Showing connections that follow."
            );

            if let Some(urns) = first_degree {
                if urns.is_empty() {
                    println!("(no first-degree connections follow this page)");
                } else {
                    println!("{} of your connections follow this page:", urns.len());
                    for (i, urn) in urns.iter().enumerate() {
                        let urn_str = urn.as_str().unwrap_or("");
                        let id = urn_str
                            .strip_prefix("urn:li:fs_normalized_profile:")
                            .unwrap_or(urn_str);
                        println!("[{}] {}", i + 1, id);
                    }
                }
            } else {
                println!("(no follower data available)");
            }

            Ok(())
        }
    }
}

/// Print a human-readable summary of a company/organization response.
fn print_company_summary(company: &serde_json::Value) {
    // Name -- several possible field names.
    let name = company
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| company.get("localizedName").and_then(|v| v.as_str()))
        .unwrap_or("(unknown)");
    println!("Company: {}", name);

    if let Some(universal_name) = company.get("universalName").and_then(|v| v.as_str()) {
        println!("Slug: {}", universal_name);
    }

    if let Some(tagline) = company
        .get("tagline")
        .and_then(|v| v.as_str())
        .or_else(|| company.get("localizedTagline").and_then(|v| v.as_str()))
    {
        println!("Tagline: {}", tagline);
    }

    if let Some(description) = company
        .get("description")
        .and_then(|v| v.as_str())
        .or_else(|| company.get("localizedDescription").and_then(|v| v.as_str()))
    {
        println!("About: {}", truncate_with_ellipsis(description, 300));
    }

    if let Some(website) = company
        .get("companyPageUrl")
        .and_then(|v| v.as_str())
        .or_else(|| company.get("websiteUrl").and_then(|v| v.as_str()))
    {
        println!("Website: {}", website);
    }

    // Industry
    if let Some(industry) = company
        .get("companyIndustries")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|i| {
            i.get("localizedName")
                .and_then(|v| v.as_str())
                .or_else(|| i.get("name").and_then(|v| v.as_str()))
        })
    {
        println!("Industry: {}", industry);
    }

    // Staff count range
    if let Some(range) = company.get("staffCountRange").and_then(|r| {
        let start = r.get("start").and_then(|v| v.as_u64());
        let end = r.get("end").and_then(|v| v.as_u64());
        match (start, end) {
            (Some(s), Some(e)) => Some(format!("{}-{} employees", s, e)),
            (Some(s), None) => Some(format!("{}+ employees", s)),
            _ => None,
        }
    }) {
        println!("Size: {}", range);
    } else if let Some(count) = company.get("staffCount").and_then(|v| v.as_u64()) {
        println!("Staff count: {}", count);
    }

    // Follower count
    if let Some(followers) = company
        .get("followingInfo")
        .and_then(|f| f.get("followerCount").and_then(|v| v.as_u64()))
    {
        println!("Followers: {}", followers);
    }

    // Headquarters
    if let Some(hq) = company.get("headquarter").or_else(|| {
        company
            .get("confirmedLocations")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
    }) {
        let city = hq.get("city").and_then(|v| v.as_str()).unwrap_or("");
        let country = hq
            .get("country")
            .and_then(|v| v.as_str())
            .or_else(|| hq.get("countryCode").and_then(|v| v.as_str()))
            .unwrap_or("");
        if !city.is_empty() || !country.is_empty() {
            println!(
                "HQ: {}{}{}",
                city,
                if !city.is_empty() && !country.is_empty() {
                    ", "
                } else {
                    ""
                },
                country
            );
        }
    }

    // Entity URN
    if let Some(urn) = company.get("entityUrn").and_then(|v| v.as_str()) {
        println!("URN: {}", urn);
    }
}
