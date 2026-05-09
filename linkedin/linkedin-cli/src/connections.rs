use linkedin_api::client::LinkedInClient;
use linkedin_api::models::{ConnectionsResponse, Paging};

use crate::session::load_session_client;
use crate::util::print_paging_header;

/// Handle `connections list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/relationships/connections with
/// pagination params sorted by RECENTLY_ADDED, and prints the results.
pub async fn cmd_connections_list(
    start: u32,
    count: u32,
    fetch_all: bool,
    keyword_filter: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    if fetch_all {
        return cmd_connections_list_all(&client, keyword_filter, raw_json).await;
    }

    let value = client
        .get_connections(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let resp: ConnectionsResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse connections response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header("Connections", paging);
    }
    if let Some(kw) = keyword_filter {
        println!("  filter: \"{}\"", kw);
    }
    println!("---");

    if resp.elements.is_empty() {
        println!("(no connections)");
        return Ok(());
    }

    let kw_lower = keyword_filter.map(|s| s.to_lowercase());
    let mut shown = 0;

    for (i, element) in resp.elements.iter().enumerate() {
        let idx = start as usize + i + 1;

        if let Some(ref kw) = kw_lower {
            let mini = element.get("miniProfile");
            let first = mini
                .and_then(|m| m.get("firstName").and_then(|v| v.as_str()))
                .unwrap_or("");
            let last = mini
                .and_then(|m| m.get("lastName").and_then(|v| v.as_str()))
                .unwrap_or("");
            let headline = mini
                .and_then(|m| m.get("occupation").and_then(|v| v.as_str()))
                .unwrap_or("");
            let searchable = format!("{} {} {}", first, last, headline).to_lowercase();
            if !searchable.contains(kw) {
                continue;
            }
        }

        shown += 1;
        print_connection(idx, element);
        println!();
    }

    if shown == 0 {
        println!("(no matching connections)");
    }

    Ok(())
}

/// Fetch all connections with auto-pagination.
///
/// Pages through the connections endpoint in batches of 40, printing
/// each connection as it arrives. The client's built-in rate limiter
/// handles throttling between requests.
pub async fn cmd_connections_list_all(
    client: &LinkedInClient,
    keyword_filter: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
    let page_size = 40u32;
    let mut offset = 0u32;
    let mut total_shown = 0usize;
    let mut all_elements: Vec<serde_json::Value> = Vec::new();
    let kw_lower = keyword_filter.map(|s| s.to_lowercase());

    loop {
        let value = client
            .get_connections(offset, page_size)
            .await
            .map_err(|e| format!("API call failed: {e}"))?;

        let resp: ConnectionsResponse = serde_json::from_value(value.clone())
            .map_err(|e| format!("failed to parse connections response: {e}"))?;

        if raw_json {
            all_elements.extend(resp.elements.clone());
        } else {
            if offset == 0 {
                let total = resp
                    .paging
                    .as_ref()
                    .and_then(|p| p.total)
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "?".to_string());
                eprintln!("Fetching all connections (total: {})...", total);
                if let Some(kw) = keyword_filter {
                    println!("  filter: \"{}\"", kw);
                }
                println!("---");
            }

            for element in &resp.elements {
                let idx = offset as usize + total_shown + 1;

                if let Some(ref kw) = kw_lower {
                    let mini = element.get("miniProfile");
                    let first = mini
                        .and_then(|m| m.get("firstName").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let last = mini
                        .and_then(|m| m.get("lastName").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let headline = mini
                        .and_then(|m| m.get("occupation").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let searchable = format!("{} {} {}", first, last, headline).to_lowercase();
                    if !searchable.contains(kw) {
                        continue;
                    }
                }

                total_shown += 1;
                print_connection(idx, element);
                println!();
            }
        }

        let page_count = resp.elements.len() as u32;
        if page_count < page_size {
            break; // Last page.
        }

        // Check if we've fetched everything.
        if let Some(total) = resp.paging.as_ref().and_then(|p| p.total) {
            if offset + page_count >= total {
                break;
            }
        }

        offset += page_count;
        eprintln!("  fetched {}...", offset);
    }

    if raw_json {
        let combined = serde_json::json!({
            "elements": all_elements,
            "total": all_elements.len(),
        });
        let pretty = serde_json::to_string_pretty(&combined)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else if total_shown == 0 {
        println!("(no matching connections)");
    } else {
        eprintln!("Total: {} connections", total_shown);
    }

    Ok(())
}

/// Handle `connections invite <public_id_or_urn> [--message "text"] [--json]`.
///
/// Resolves the target to a profile URN if a public identifier is given,
/// then sends a connection request via the normInvitations endpoint.
pub async fn cmd_connections_invite(
    public_id_or_urn: &str,
    message: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // Resolve to a profile URN if needed.
    let profile_urn = if public_id_or_urn.starts_with("urn:li:") {
        public_id_or_urn.to_string()
    } else {
        eprintln!("Resolving profile URN for '{}'...", public_id_or_urn);
        client
            .resolve_profile_urn(public_id_or_urn)
            .await
            .map_err(|e| format!("failed to resolve profile: {e}"))?
    };

    let value = client
        .send_connection_request(&profile_urn, message)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!(
            "Connection request sent to {} ({})",
            public_id_or_urn, profile_urn
        );
        if message.is_some() {
            println!("  (with custom message)");
        }
    }

    Ok(())
}

/// Parse a batch input list (one slug/URN per line).
///
/// Strips surrounding whitespace from each line and drops blank lines and
/// comments (lines starting with `#`). Inline trailing comments are kept
/// as-is — only full-line comments are filtered, since slugs never start
/// with `#` and a strict parser surprises less than a clever one.
fn parse_batch_targets(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Handle `connections invite-batch --from-file <path|->`.
///
/// Sends a connection invitation to each line of the input list with a
/// configurable pacing delay between calls. Tab-separated status lines on
/// stdout, errors on stderr. Returns Err only when stop_on_error is set
/// and a per-line failure occurs; otherwise summarises with a final
/// success/fail count.
pub async fn cmd_connections_invite_batch(
    from_file: &str,
    message: Option<&str>,
    pacing_ms: u64,
    stop_on_error: bool,
) -> Result<(), String> {
    let raw = if from_file == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        buf
    } else {
        std::fs::read_to_string(from_file)
            .map_err(|e| format!("failed to read {}: {}", from_file, e))?
    };

    let targets = parse_batch_targets(&raw);
    if targets.is_empty() {
        return Err("no targets in input (every line was blank or a comment)".to_string());
    }

    let (client, _path) = load_session_client()?;

    let total = targets.len();
    let mut ok_count = 0usize;
    let mut fail_count = 0usize;

    for (i, target) in targets.iter().enumerate() {
        if i > 0 && pacing_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(pacing_ms)).await;
        }

        let profile_urn = if target.starts_with("urn:li:") {
            target.clone()
        } else {
            match client.resolve_profile_urn(target).await {
                Ok(u) => u,
                Err(e) => {
                    fail_count += 1;
                    println!("{}\tFAIL\tresolve: {}", target, e);
                    if stop_on_error {
                        return Err(format!("stopped at #{} ({}): {}", i + 1, target, e));
                    }
                    continue;
                }
            }
        };

        match client.send_connection_request(&profile_urn, message).await {
            Ok(value) => {
                let invitation = value
                    .get("value")
                    .and_then(|v| v.get("invitationUrn"))
                    .or_else(|| value.get("invitationUrn"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no invitation urn)");
                ok_count += 1;
                println!("{}\tOK\t{}", target, invitation);
            }
            Err(e) => {
                fail_count += 1;
                println!("{}\tFAIL\tsend: {}", target, e);
                if stop_on_error {
                    return Err(format!("stopped at #{} ({}): {}", i + 1, target, e));
                }
            }
        }
    }

    eprintln!(
        "Batch complete: {} ok, {} failed (of {} total)",
        ok_count, fail_count, total
    );
    Ok(())
}

/// Handle `connections invitations [--count N] [--start N] [--json]`.
///
/// Lists pending (received) connection invitations using the Dash GraphQL
/// `voyagerRelationshipsDashInvitationViews` endpoint.
pub async fn cmd_connections_invitations(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_invitations(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // Parse paging for the header line.
    let paging: Option<Paging> = value
        .get("paging")
        .and_then(|p| serde_json::from_value(p.clone()).ok());

    if let Some(ref paging) = paging {
        print_paging_header("Pending invitations", paging);
    }
    println!("---");

    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    if elements.is_empty() {
        println!("(no pending invitations)");
        return Ok(());
    }

    for (i, element) in elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_invitation(idx, element);
        println!();
    }

    Ok(())
}

/// Handle `connections accept <invitation_id> --secret <shared_secret> [--json]`.
///
/// Accepts a pending connection invitation using the Dash REST endpoint.
pub async fn cmd_connections_accept(
    invitation_id: &str,
    shared_secret: &str,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // Build the full invitation URN if only the ID portion was given.
    let invitation_urn = if invitation_id.starts_with("urn:li:") {
        invitation_id.to_string()
    } else {
        format!("urn:li:fsd_invitation:{}", invitation_id)
    };

    let value = client
        .accept_invitation(&invitation_urn, shared_secret)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Invitation accepted: {}", invitation_urn);
    }

    Ok(())
}

pub async fn cmd_connections_withdraw(
    invitation_id: &str,
    shared_secret: &str,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let invitation_urn = if invitation_id.starts_with("urn:li:") {
        invitation_id.to_string()
    } else {
        format!("urn:li:fsd_invitation:{}", invitation_id)
    };

    let value = client
        .withdraw_invitation(&invitation_urn, shared_secret)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Invitation withdrawn: {}", invitation_urn);
    }

    Ok(())
}

/// Print a brief human-readable summary of a single invitation view.
///
/// The GraphQL `relationshipsDashInvitationViewsByReceived` response returns
/// `InvitationView` objects with:
/// - `title.text`: inviter's name
/// - `subtitle.text`: inviter's headline
/// - `sentTimeLabel`: human-readable time (e.g. "2 days ago")
/// - `invitation.entityUrn`: invitation URN (needed for accept/ignore)
/// - `invitation.sharedSecret`: required for accept action
/// - `invitation.message`: optional custom message from inviter
/// - `invitation.genericInvitationType`: type of invitation (CONNECTION, etc.)
fn print_invitation(index: usize, view: &serde_json::Value) {
    let name = view
        .get("title")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");

    let headline = view
        .get("subtitle")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let sent_time = view
        .get("sentTimeLabel")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let invitation = view.get("invitation");

    let invitation_urn = invitation
        .and_then(|inv| inv.get("entityUrn"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let shared_secret = invitation
        .and_then(|inv| inv.get("sharedSecret"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let message = invitation
        .and_then(|inv| inv.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let inv_type = invitation
        .and_then(|inv| inv.get("genericInvitationType"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Extract invitation ID from URN for easier accept command usage.
    let invitation_id = invitation_urn.rsplit(':').next().unwrap_or(invitation_urn);

    print!("[{}] {}", index, name);
    if !sent_time.is_empty() {
        print!("  ({})", sent_time);
    }
    println!();

    if !headline.is_empty() {
        println!("    {}", headline);
    }
    if !inv_type.is_empty() {
        println!("    type: {}", inv_type);
    }
    if !message.is_empty() {
        println!("    message: \"{}\"", message);
    }
    if !invitation_id.is_empty() {
        println!("    id: {}  secret: {}", invitation_id, shared_secret);
        println!(
            "    accept: connections accept {} --secret \"{}\"",
            invitation_id, shared_secret
        );
    }
}

/// Print a brief human-readable summary of a single connection.
fn print_connection(index: usize, conn: &serde_json::Value) {
    // Extract name and headline from the embedded miniProfile.
    let mini = conn.get("miniProfile");

    let name = mini
        .and_then(|m| {
            let first = m.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
            let last = m.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
            if first.is_empty() && last.is_empty() {
                None
            } else {
                Some(format!("{} {}", first, last).trim().to_string())
            }
        })
        .unwrap_or_else(|| "(unknown)".to_string());

    let headline = mini
        .and_then(|m| m.get("occupation").and_then(|v| v.as_str()))
        .unwrap_or("");

    // Connected-since date from createdAt (epoch millis).
    let connected_since = conn
        .get("createdAt")
        .and_then(|c| c.as_u64())
        .and_then(|millis| {
            let secs = (millis / 1000) as i64;
            chrono::DateTime::from_timestamp(secs, 0).map(|dt| dt.format("%Y-%m-%d").to_string())
        })
        .unwrap_or_default();

    // Public identifier for reference.
    let pub_id = mini
        .and_then(|m| m.get("publicIdentifier").and_then(|v| v.as_str()))
        .unwrap_or("");

    print!("[{}] {}", index, name);
    if !pub_id.is_empty() {
        print!(" ({})", pub_id);
    }
    println!();

    if !headline.is_empty() {
        println!("    {}", headline);
    }
    if !connected_since.is_empty() {
        println!("    connected since: {}", connected_since);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_parser_strips_blanks_and_comments() {
        let raw = "\
# This is a comment
slug-one
   slug-two

# another comment
urn:li:fsd_profile:ACoAAA111
\t

slug-three
";
        let parsed = parse_batch_targets(raw);
        assert_eq!(
            parsed,
            vec![
                "slug-one".to_string(),
                "slug-two".to_string(),
                "urn:li:fsd_profile:ACoAAA111".to_string(),
                "slug-three".to_string(),
            ]
        );
    }

    #[test]
    fn batch_parser_returns_empty_for_all_comments() {
        let raw = "# only comments\n# and blanks\n\n   \n";
        assert!(parse_batch_targets(raw).is_empty());
    }
}
