use serde_json::Value;

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
        print_json(&company)?;
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

    let company = client
        .get_company(slug)
        .await
        .map_err(|e| format!("failed to fetch company: {e}"))?;

    let company_name = company.get("name").and_then(|v| v.as_str()).unwrap_or(slug);
    let company_id = extract_company_id(&company)?;

    match client.get_company_followers(company_id, start, count).await {
        Ok(value) => print_followers_primary(FollowersPrintArgs {
            company: &company,
            company_name,
            value: &value,
            start,
            raw_json,
        }),
        Err(_) => print_followers_fallback(&company, company_name, raw_json),
    }
}

/// Bundled inputs for [`print_followers_primary`].
struct FollowersPrintArgs<'a> {
    company: &'a Value,
    company_name: &'a str,
    value: &'a Value,
    start: u32,
    raw_json: bool,
}

/// Pull the numeric company ID from the response. Prefers the trailing
/// segment of `entityUrn`; falls back to a non-empty `companyId` field.
fn extract_company_id(company: &Value) -> Result<&str, String> {
    company
        .get("entityUrn")
        .and_then(|v| v.as_str())
        .and_then(|urn| urn.rsplit(':').next())
        .or_else(|| {
            company
                .get("companyId")
                .and_then(|v| v.as_u64())
                .map(|_| "")
        })
        .ok_or_else(|| "could not extract company ID from response".to_string())
}

/// Print the result of the admin follower endpoint. Branches on
/// list-vs-analytics shape; either path may emit JSON.
fn print_followers_primary(args: FollowersPrintArgs<'_>) -> Result<(), String> {
    if args.raw_json {
        return print_json(args.value);
    }

    if let Some(elements) = args.value.get("elements").and_then(|e| e.as_array()) {
        let total = follower_total(args.company);
        print_followers_list(args.company_name, total, elements, args.start);
        return Ok(());
    }

    println!("Follower analytics for {}:", args.company_name);
    print_json(args.value)
}

/// Print a numbered follower list with a header and optional total.
fn print_followers_list(company_name: &str, total: Option<u64>, elements: &[Value], start: u32) {
    let total_str = total
        .map(|c| c.to_string())
        .unwrap_or_else(|| "?".to_string());
    println!(
        "Followers of {} (showing {}, total {})",
        company_name,
        elements.len(),
        total_str
    );
    println!("---");

    for (i, el) in elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        let name = follower_display_name(el);
        println!("[{}] {}", idx, name.trim());
    }

    if elements.is_empty() {
        println!("(no follower details available)");
    }
}

/// Resolve a follower element to a display name. Tries the `follower`
/// branch first, then `miniProfile`, falling back to `(unknown)`.
fn follower_display_name(el: &Value) -> String {
    person_full_name(el.get("follower"))
        .or_else(|| person_full_name(el.get("miniProfile")))
        .unwrap_or_else(|| "(unknown)".to_string())
}

/// Build "First Last" from a person-shaped value. Returns None if both
/// names are missing or empty.
fn person_full_name(value: Option<&Value>) -> Option<String> {
    let v = value?;
    let first = v.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
    let last = v.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
    if first.is_empty() && last.is_empty() {
        None
    } else {
        Some(format!("{} {}", first, last))
    }
}

/// Print the fallback view when the admin follower endpoint is
/// unavailable: total follower count plus any first-degree connections
/// that follow the page.
fn print_followers_fallback(
    company: &Value,
    company_name: &str,
    raw_json: bool,
) -> Result<(), String> {
    let first_degree = company
        .get("firstDegreeConnectionsThatFollow")
        .and_then(|v| v.as_array());
    let follower_count = follower_total(company).unwrap_or(0);

    if raw_json {
        let data = serde_json::json!({
            "followerCount": follower_count,
            "firstDegreeConnectionsThatFollow": first_degree,
        });
        return print_json(&data);
    }

    println!("Followers of {} (total: {})", company_name, follower_count);
    println!("---");
    eprintln!(
        "Note: Full follower list requires admin API access. Showing connections that follow."
    );

    match first_degree {
        Some(urns) if urns.is_empty() => {
            println!("(no first-degree connections follow this page)");
        }
        Some(urns) => {
            println!("{} of your connections follow this page:", urns.len());
            for (i, urn) in urns.iter().enumerate() {
                let urn_str = urn.as_str().unwrap_or("");
                let id = urn_str
                    .strip_prefix("urn:li:fs_normalized_profile:")
                    .unwrap_or(urn_str);
                println!("[{}] {}", i + 1, id);
            }
        }
        None => println!("(no follower data available)"),
    }

    Ok(())
}

/// Pretty-print a JSON value to stdout.
fn print_json(value: &Value) -> Result<(), String> {
    let pretty =
        serde_json::to_string_pretty(value).map_err(|e| format!("JSON format error: {e}"))?;
    println!("{}", pretty);
    Ok(())
}

/// First non-empty string under `value` from the candidate keys.
fn first_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| value.get(k).and_then(|v| v.as_str()))
}

/// Total follower count, if exposed under `followingInfo.followerCount`.
fn follower_total(company: &Value) -> Option<u64> {
    company
        .get("followingInfo")
        .and_then(|f| f.get("followerCount"))
        .and_then(|v| v.as_u64())
}

/// Print a human-readable summary of a company/organization response.
fn print_company_summary(company: &Value) {
    print_summary_header(company);
    print_summary_about(company);
    print_summary_industry(company);
    print_summary_size(company);
    print_summary_followers(company);
    print_summary_hq(company);
    print_summary_urn(company);
}

fn print_summary_header(company: &Value) {
    let name = first_str(company, &["name", "localizedName"]).unwrap_or("(unknown)");
    println!("Company: {}", name);

    if let Some(slug) = company.get("universalName").and_then(|v| v.as_str()) {
        println!("Slug: {}", slug);
    }
    if let Some(tagline) = first_str(company, &["tagline", "localizedTagline"]) {
        println!("Tagline: {}", tagline);
    }
}

fn print_summary_about(company: &Value) {
    if let Some(description) = first_str(company, &["description", "localizedDescription"]) {
        println!("About: {}", truncate_with_ellipsis(description, 300));
    }
    if let Some(website) = first_str(company, &["companyPageUrl", "websiteUrl"]) {
        println!("Website: {}", website);
    }
}

fn print_summary_industry(company: &Value) {
    let industry = company
        .get("companyIndustries")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|i| first_str(i, &["localizedName", "name"]));
    if let Some(industry) = industry {
        println!("Industry: {}", industry);
    }
}

fn print_summary_size(company: &Value) {
    if let Some(range) = company.get("staffCountRange").and_then(format_staff_range) {
        println!("Size: {}", range);
    } else if let Some(count) = company.get("staffCount").and_then(|v| v.as_u64()) {
        println!("Staff count: {}", count);
    }
}

fn format_staff_range(range: &Value) -> Option<String> {
    let start = range.get("start").and_then(|v| v.as_u64());
    let end = range.get("end").and_then(|v| v.as_u64());
    match (start, end) {
        (Some(s), Some(e)) => Some(format!("{}-{} employees", s, e)),
        (Some(s), None) => Some(format!("{}+ employees", s)),
        _ => None,
    }
}

fn print_summary_followers(company: &Value) {
    if let Some(followers) = follower_total(company) {
        println!("Followers: {}", followers);
    }
}

fn print_summary_hq(company: &Value) {
    let hq = company.get("headquarter").or_else(|| {
        company
            .get("confirmedLocations")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
    });
    let Some(hq) = hq else { return };

    let city = hq.get("city").and_then(|v| v.as_str()).unwrap_or("");
    let country = first_str(hq, &["country", "countryCode"]).unwrap_or("");
    if city.is_empty() && country.is_empty() {
        return;
    }
    let separator = if !city.is_empty() && !country.is_empty() {
        ", "
    } else {
        ""
    };
    println!("HQ: {}{}{}", city, separator, country);
}

fn print_summary_urn(company: &Value) {
    if let Some(urn) = company.get("entityUrn").and_then(|v| v.as_str()) {
        println!("URN: {}", urn);
    }
}
