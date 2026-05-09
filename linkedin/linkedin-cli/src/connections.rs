use serde_json::Value;

use linkedin_api::client::LinkedInClient;
use linkedin_api::models::{ConnectionsResponse, Paging};

use crate::session::load_session_client;
use crate::util::print_paging_header;

/// Options for `connections list`. Bundles the five-argument call into a
/// single struct so callers don't have to keep parameters straight by
/// position.
pub struct ConnectionsListOptions<'a> {
    pub start: u32,
    pub count: u32,
    pub fetch_all: bool,
    pub keyword_filter: Option<&'a str>,
    pub raw_json: bool,
}

/// Handle `connections list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/relationships/connections with
/// pagination params sorted by RECENTLY_ADDED, and prints the results.
pub async fn cmd_connections_list(opts: ConnectionsListOptions<'_>) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    if opts.fetch_all {
        return cmd_connections_list_all(&client, opts.keyword_filter, opts.raw_json).await;
    }

    let value = client
        .get_connections(opts.start, opts.count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if opts.raw_json {
        return print_json(&value);
    }

    let resp: ConnectionsResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse connections response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header("Connections", paging);
    }
    if let Some(kw) = opts.keyword_filter {
        println!("  filter: \"{}\"", kw);
    }
    println!("---");

    if resp.elements.is_empty() {
        println!("(no connections)");
        return Ok(());
    }

    let kw_lower = opts.keyword_filter.map(str::to_lowercase);
    let shown = print_connection_page(&resp.elements, opts.start as usize, kw_lower.as_deref());

    if shown == 0 {
        println!("(no matching connections)");
    }

    Ok(())
}

/// Print a slice of connection elements with optional keyword filtering.
/// Returns the number of connections actually printed.
fn print_connection_page(
    elements: &[Value],
    start_index: usize,
    keyword_lower: Option<&str>,
) -> usize {
    let mut shown = 0;
    for (i, element) in elements.iter().enumerate() {
        if let Some(kw) = keyword_lower {
            if !connection_matches_keyword(element, kw) {
                continue;
            }
        }
        shown += 1;
        print_connection(start_index + i + 1, element);
        println!();
    }
    shown
}

/// Check whether a connection's name and headline contain the lowercase
/// `keyword`. Returns true if no keyword is required.
fn connection_matches_keyword(element: &Value, keyword_lower: &str) -> bool {
    let mini = element.get("miniProfile");
    let first = profile_str(mini, "firstName");
    let last = profile_str(mini, "lastName");
    let headline = profile_str(mini, "occupation");
    let searchable = format!("{} {} {}", first, last, headline).to_lowercase();
    searchable.contains(keyword_lower)
}

/// Read a string field from a `miniProfile`-shaped value.
fn profile_str<'a>(mini: Option<&'a Value>, key: &str) -> &'a str {
    mini.and_then(|m| m.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
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
    let mut all_elements: Vec<Value> = Vec::new();
    let kw_lower = keyword_filter.map(str::to_lowercase);

    loop {
        let resp = fetch_connections_page(client, offset, page_size).await?;

        if raw_json {
            all_elements.extend(resp.elements.clone());
        } else {
            if offset == 0 {
                print_fetch_all_header(&resp, keyword_filter);
            }
            total_shown += print_connection_page(
                &resp.elements,
                offset as usize + total_shown,
                kw_lower.as_deref(),
            );
        }

        let page_count = resp.elements.len() as u32;
        if is_last_page(&resp, offset, page_count, page_size) {
            break;
        }
        offset += page_count;
        eprintln!("  fetched {}...", offset);
    }

    finalize_fetch_all(raw_json, total_shown, all_elements)
}

async fn fetch_connections_page(
    client: &LinkedInClient,
    offset: u32,
    page_size: u32,
) -> Result<ConnectionsResponse, String> {
    let value = client
        .get_connections(offset, page_size)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;
    serde_json::from_value(value).map_err(|e| format!("failed to parse connections response: {e}"))
}

fn print_fetch_all_header(resp: &ConnectionsResponse, keyword_filter: Option<&str>) {
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

fn is_last_page(resp: &ConnectionsResponse, offset: u32, page_count: u32, page_size: u32) -> bool {
    if page_count < page_size {
        return true;
    }
    resp.paging
        .as_ref()
        .and_then(|p| p.total)
        .is_some_and(|total| offset + page_count >= total)
}

fn finalize_fetch_all(
    raw_json: bool,
    total_shown: usize,
    all_elements: Vec<Value>,
) -> Result<(), String> {
    if raw_json {
        let combined = serde_json::json!({
            "elements": all_elements,
            "total": all_elements.len(),
        });
        print_json(&combined)?;
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
    let profile_urn = resolve_invite_target(&client, public_id_or_urn).await?;

    let value = client
        .send_connection_request(&profile_urn, message)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        print_json(&value)?;
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

/// Resolve a public ID to a profile URN, passing URNs through unchanged.
async fn resolve_invite_target(
    client: &LinkedInClient,
    public_id_or_urn: &str,
) -> Result<String, String> {
    if public_id_or_urn.starts_with("urn:li:") {
        return Ok(public_id_or_urn.to_string());
    }
    eprintln!("Resolving profile URN for '{}'...", public_id_or_urn);
    client
        .resolve_profile_urn(public_id_or_urn)
        .await
        .map_err(|e| format!("failed to resolve profile: {e}"))
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

/// Read a batch source: either a path on disk, or `-` for stdin.
fn read_batch_input(from_file: &str) -> Result<String, String> {
    if from_file == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(from_file)
            .map_err(|e| format!("failed to read {}: {}", from_file, e))
    }
}

/// Outcome of one row in a batch invite run. The `Fail` arm carries the
/// reason so the caller can surface it when `--stop-on-error` is set.
enum BatchRowResult {
    Ok,
    Fail(String),
}

/// Send one invite row. Logs to stdout, returns Ok/Fail for tally.
async fn invite_one(
    client: &LinkedInClient,
    target: &str,
    message: Option<&str>,
) -> BatchRowResult {
    let profile_urn = if target.starts_with("urn:li:") {
        target.to_string()
    } else {
        match client.resolve_profile_urn(target).await {
            Ok(u) => u,
            Err(e) => {
                println!("{}\tFAIL\tresolve: {}", target, e);
                return BatchRowResult::Fail(format!("resolve: {e}"));
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
            println!("{}\tOK\t{}", target, invitation);
            BatchRowResult::Ok
        }
        Err(e) => {
            println!("{}\tFAIL\tsend: {}", target, e);
            BatchRowResult::Fail(format!("send: {e}"))
        }
    }
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
    let raw = read_batch_input(from_file)?;
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

        match invite_one(&client, target, message).await {
            BatchRowResult::Ok => ok_count += 1,
            BatchRowResult::Fail(reason) => {
                fail_count += 1;
                if stop_on_error {
                    return Err(format!("stopped at #{} ({}): {}", i + 1, target, reason));
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
pub async fn cmd_connections_invitations(
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_invitations(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        return print_json(&value);
    }

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
        print_invitation(start as usize + i + 1, element);
        println!();
    }

    Ok(())
}

/// Which invitation lifecycle action to perform.
enum InvitationOp {
    Accept,
    Withdraw,
}

impl InvitationOp {
    fn done_label(&self) -> &'static str {
        match self {
            InvitationOp::Accept => "Invitation accepted",
            InvitationOp::Withdraw => "Invitation withdrawn",
        }
    }
}

/// Build a full `urn:li:fsd_invitation:*` URN, or pass through if already qualified.
fn build_invitation_urn(invitation_id: &str) -> String {
    if invitation_id.starts_with("urn:li:") {
        invitation_id.to_string()
    } else {
        format!("urn:li:fsd_invitation:{}", invitation_id)
    }
}

async fn invitation_op(
    invitation_id: &str,
    shared_secret: &str,
    raw_json: bool,
    op: InvitationOp,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;
    let invitation_urn = build_invitation_urn(invitation_id);

    let value = match op {
        InvitationOp::Accept => {
            client
                .accept_invitation(&invitation_urn, shared_secret)
                .await
        }
        InvitationOp::Withdraw => {
            client
                .withdraw_invitation(&invitation_urn, shared_secret)
                .await
        }
    }
    .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        print_json(&value)?;
    } else {
        println!("{}: {}", op.done_label(), invitation_urn);
    }
    Ok(())
}

/// Handle `connections accept <invitation_id> --secret <shared_secret> [--json]`.
pub async fn cmd_connections_accept(
    invitation_id: &str,
    shared_secret: &str,
    raw_json: bool,
) -> Result<(), String> {
    invitation_op(invitation_id, shared_secret, raw_json, InvitationOp::Accept).await
}

pub async fn cmd_connections_withdraw(
    invitation_id: &str,
    shared_secret: &str,
    raw_json: bool,
) -> Result<(), String> {
    invitation_op(
        invitation_id,
        shared_secret,
        raw_json,
        InvitationOp::Withdraw,
    )
    .await
}

/// Pretty-print a JSON value to stdout.
fn print_json(value: &Value) -> Result<(), String> {
    let pretty =
        serde_json::to_string_pretty(value).map_err(|e| format!("JSON format error: {e}"))?;
    println!("{}", pretty);
    Ok(())
}

/// Print a brief human-readable summary of a single invitation view.
///
/// The GraphQL `relationshipsDashInvitationViewsByReceived` response returns
/// `InvitationView` objects with structured text and an embedded
/// `invitation` object carrying the URN and shared secret needed for accept.
fn print_invitation(index: usize, view: &Value) {
    let name = nested_text(view, &["title", "text"]).unwrap_or("(unknown)");
    let headline = nested_text(view, &["subtitle", "text"]).unwrap_or("");
    let sent_time = view
        .get("sentTimeLabel")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let invitation = view.get("invitation");
    let invitation_urn = invitation_str(invitation, "entityUrn");
    let shared_secret = invitation_str(invitation, "sharedSecret");
    let message = invitation_str(invitation, "message");
    let inv_type = invitation_str(invitation, "genericInvitationType");
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

/// Walk a nested object path, returning the final string if every step
/// resolves and the leaf is a string.
fn nested_text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    current.as_str()
}

/// Convenience for the `invitation.<key>` field-as-string lookup.
fn invitation_str<'a>(invitation: Option<&'a Value>, key: &str) -> &'a str {
    invitation
        .and_then(|inv| inv.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

/// Print a brief human-readable summary of a single connection.
fn print_connection(index: usize, conn: &Value) {
    let mini = conn.get("miniProfile");
    let name = connection_full_name(mini);
    let headline = profile_str(mini, "occupation");
    let connected_since = connected_since_label(conn);
    let pub_id = profile_str(mini, "publicIdentifier");

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

fn connection_full_name(mini: Option<&Value>) -> String {
    let first = profile_str(mini, "firstName");
    let last = profile_str(mini, "lastName");
    if first.is_empty() && last.is_empty() {
        "(unknown)".to_string()
    } else {
        format!("{} {}", first, last).trim().to_string()
    }
}

fn connected_since_label(conn: &Value) -> String {
    conn.get("createdAt")
        .and_then(|c| c.as_u64())
        .and_then(|millis| {
            let secs = (millis / 1000) as i64;
            chrono::DateTime::from_timestamp(secs, 0).map(|dt| dt.format("%Y-%m-%d").to_string())
        })
        .unwrap_or_default()
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
