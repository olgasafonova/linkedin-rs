use serde_json::Value;

use linkedin_api::client::{ConversationCategory, LinkedInClient};
use linkedin_api::models::{ConnectionsResponse, Paging};
use linkedin_api::urn::{ConversationUrn, ProfileUrn};

use crate::error::{CliError, CliResult};
use crate::graphql_print::{print_graphql_conversation, print_graphql_message};
use crate::session::load_session_client;
use crate::spam::{is_spam_conversation, is_spam_invitation};
use crate::util::truncate_with_ellipsis;

/// URN prefixes recognised as already-resolved member identifiers.
const PROFILE_URN_PREFIXES: &[&str] = &[
    "urn:li:fsd_profile:",
    "urn:li:member:",
    "urn:li:fs_miniProfile:",
];

/// Pretty-print a JSON value to stdout.
fn print_json(value: &Value) -> CliResult<()> {
    let pretty = serde_json::to_string_pretty(value)?;
    println!("{}", pretty);
    Ok(())
}

/// Read a string field that may be either a bare string or a `{text: "..."}`
/// object. LinkedIn's GraphQL responses use both shapes interchangeably for
/// participant names.
fn flexible_text(value: Option<&Value>) -> Option<&str> {
    let v = value?;
    v.get("text")
        .and_then(|t| t.as_str())
        .or_else(|| v.as_str())
}

/// Build "First Last" from a member-shaped value, returning None if both
/// names are missing or empty.
fn extract_member_name(member: &Value) -> Option<String> {
    let first = flexible_text(member.get("firstName")).unwrap_or("");
    let last = flexible_text(member.get("lastName")).unwrap_or("");
    if first.is_empty() && last.is_empty() {
        None
    } else {
        Some(format!("{} {}", first, last).trim().to_string())
    }
}

/// Read a message's body text. Handles both the bare-string and the
/// `{text: "..."}` shapes.
fn extract_message_body(msg: &Value) -> Option<String> {
    let body = msg.get("body")?;
    if body.is_string() {
        return body.as_str().map(str::to_string);
    }
    body.get("text")
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

/// `elements` array from a wrapper response, defaulting to empty.
fn elements_slice(value: &Value) -> &[Value] {
    value
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// Handle `inbox [--json] [--all]`.
///
/// Shows a daily summary: unread messages, pending invitations, and
/// recent unread notifications in one view. Filters likely recruiter
/// spam by default; use `--all` to see everything.
pub async fn cmd_inbox(raw_json: bool, show_all: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    // Fetch all three concurrently. The client's request throttle still
    // serialises the actual sends, but tokio::join! lets each response
    // parse while the next request is queued, saving the gap between
    // sequential awaits.
    let (conversations_res, invitations_res, notifications_res) = tokio::join!(
        client.get_conversations(10, None),
        client.get_invitations(0, 10),
        client.get_notifications(0, 10),
    );
    let conversations =
        conversations_res.map_err(|e| CliError::Other(format!("failed to fetch messages: {e}")))?;
    let invitations = invitations_res
        .map_err(|e| CliError::Other(format!("failed to fetch invitations: {e}")))?;
    let notifications = notifications_res
        .map_err(|e| CliError::Other(format!("failed to fetch notifications: {e}")))?;

    if raw_json {
        let combined = serde_json::json!({
            "unreadMessages": conversations,
            "pendingInvitations": invitations,
            "recentNotifications": notifications,
        });
        return print_json(&combined);
    }

    print_unread_messages_section(&conversations, show_all);
    println!();
    print_pending_invitations_section(&invitations, show_all);
    println!();
    print_unread_notifications_section(&notifications);
    Ok(())
}

fn print_unread_messages_section(conversations: &Value, show_all: bool) {
    let unread: Vec<&Value> = elements_slice(conversations)
        .iter()
        .filter(|c| is_unread_conversation(c))
        .collect();

    let (displayed, spam_count) = filter_spam(unread.as_slice(), show_all, is_spam_conversation);

    println!("Unread Messages ({})", displayed.len());
    println!("---");
    if displayed.is_empty() {
        println!("  (all caught up)");
    } else {
        for (i, conv) in displayed.iter().enumerate() {
            print_inbox_conversation(i + 1, conv);
        }
    }
    if spam_count > 0 {
        println!(
            "  ({} recruiter message(s) hidden, use --all to show)",
            spam_count
        );
    }
}

fn is_unread_conversation(conv: &Value) -> bool {
    let read_false = conv.get("read").and_then(|r| r.as_bool()) == Some(false);
    let has_unread = conv
        .get("unreadCount")
        .and_then(|n| n.as_u64())
        .unwrap_or(0)
        > 0;
    read_false || has_unread
}

/// Split a slice into (displayed, hidden_count) using `is_spam`. When
/// `show_all` is true everything is displayed and the hidden count is 0.
fn filter_spam<'a, F>(items: &[&'a Value], show_all: bool, is_spam: F) -> (Vec<&'a Value>, usize)
where
    F: Fn(&Value) -> bool,
{
    if show_all {
        return (items.to_vec(), 0);
    }
    let mut displayed = Vec::with_capacity(items.len());
    let mut hidden = 0usize;
    for item in items {
        if is_spam(item) {
            hidden += 1;
        } else {
            displayed.push(*item);
        }
    }
    (displayed, hidden)
}

fn print_inbox_conversation(index: usize, conv: &Value) {
    let title = conv.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let names = extract_conversation_names(conv);
    let display = if !title.is_empty() {
        title.to_string()
    } else if !names.is_empty() {
        names.join(", ")
    } else {
        "(unknown)".to_string()
    };
    let conv_id = conversation_id(conv);
    let last_msg = first_message_body(conv).unwrap_or_default();
    let unread_count = conv
        .get("unreadCount")
        .and_then(|n| n.as_u64())
        .unwrap_or(1);

    println!("  [{}] {} ({} unread)", index, display, unread_count);
    if !last_msg.is_empty() {
        println!("      {}", truncate_with_ellipsis(&last_msg, 80));
    }
    println!("      read: messages read {}", conv_id);
}

fn conversation_id(conv: &Value) -> String {
    let backend_urn = conv
        .get("backendUrn")
        .and_then(|u| u.as_str())
        .unwrap_or("");
    backend_urn
        .strip_prefix("urn:li:messagingThread:")
        .unwrap_or(backend_urn)
        .to_string()
}

fn first_message_body(conv: &Value) -> Option<String> {
    conv.get("messages")
        .and_then(|m| m.get("elements"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(extract_message_body)
}

fn print_pending_invitations_section(invitations: &Value, show_all: bool) {
    let inv_refs: Vec<&Value> = elements_slice(invitations).iter().collect();
    let (displayed, spam_count) = filter_spam(inv_refs.as_slice(), show_all, is_spam_invitation);

    println!("Pending Invitations ({})", displayed.len());
    println!("---");
    if displayed.is_empty() {
        println!("  (none)");
    } else {
        for (i, inv) in displayed.iter().enumerate() {
            print_inbox_invitation(i + 1, inv);
        }
    }
    if spam_count > 0 {
        println!(
            "  ({} recruiter invitation(s) hidden, use --all to show)",
            spam_count
        );
    }
}

fn print_inbox_invitation(index: usize, inv: &Value) {
    let name = nested_text(inv, &["title", "text"]).unwrap_or("(unknown)");
    let headline = nested_text(inv, &["subtitle", "text"]).unwrap_or("");
    let sent_time = inv
        .get("sentTimeLabel")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let invitation = inv.get("invitation");
    let invitation_id = invitation
        .and_then(|i| i.get("entityUrn"))
        .and_then(|v| v.as_str())
        .and_then(|u| u.rsplit(':').next())
        .unwrap_or("");
    let secret = invitation
        .and_then(|i| i.get("sharedSecret"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    print!("  [{}] {}", index, name);
    if !sent_time.is_empty() {
        print!("  ({})", sent_time);
    }
    println!();
    if !headline.is_empty() {
        println!("      {}", truncate_with_ellipsis(headline, 60));
    }
    if !invitation_id.is_empty() {
        println!(
            "      accept: connections accept {} --secret \"{}\"",
            invitation_id, secret
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

fn print_unread_notifications_section(notifications: &Value) {
    let unread: Vec<&Value> = elements_slice(notifications)
        .iter()
        .filter(|n| n.get("read").and_then(|r| r.as_bool()) == Some(false))
        .collect();

    println!("Unread Notifications ({})", unread.len());
    println!("---");
    if unread.is_empty() {
        println!("  (all read)");
        return;
    }
    for (i, notif) in unread.iter().take(5).enumerate() {
        let headline = nested_text(notif, &["headline", "text"]).unwrap_or("(no headline)");
        let kicker = nested_text(notif, &["kicker", "text"]).unwrap_or("");
        print!("  [{}] {}", i + 1, truncate_with_ellipsis(headline, 80));
        if !kicker.is_empty() {
            print!("  ({})", kicker);
        }
        println!();
    }
    if unread.len() > 5 {
        println!("  ... and {} more", unread.len() - 5);
    }
}

/// Handle `who <company> [--json]`.
///
/// Shows your network overlap with a company: connections who work there,
/// profile viewers from there, recent messages with people there, and
/// key people at the company.
pub async fn cmd_who(company_slug: &str, raw_json: bool) -> CliResult<()> {
    let (client, _path) = load_session_client()?;

    eprintln!("Looking up {}...", company_slug);
    let company_data = client
        .get_company(company_slug)
        .await
        .map_err(|e| CliError::Other(format!("company lookup failed: {e}")))?;
    let company = summarize_company(&company_data, company_slug);
    let name_lower = company.name.to_lowercase();
    let slug_lower = company_slug.to_lowercase();

    eprintln!("Scanning connections...");
    let connections = scan_connections_at(&client, &name_lower, &slug_lower).await?;

    eprintln!("Checking profile viewers...");
    let viewers = scan_viewers_at(&client, &company.name, &name_lower).await;

    eprintln!("Checking messages...");
    let messages = scan_messages_with(&client, &name_lower).await;

    eprintln!("Searching people at {}...", company.name);
    let people = search_people_at(&client, &company.name, &name_lower, &slug_lower).await;

    let findings = WhoFindings {
        company,
        connections,
        viewers,
        messages,
        people,
    };
    if raw_json {
        return print_who_json(&findings);
    }
    print_who_report(&findings);
    Ok(())
}

/// Aggregated network-overlap findings for `cmd_who`. Lets the print
/// helpers take one argument instead of five.
struct WhoFindings {
    company: CompanySummary,
    connections: Vec<ConnectionMatch>,
    viewers: Vec<ViewerMatch>,
    messages: Vec<MessageMatch>,
    people: Vec<PersonMatch>,
}

struct CompanySummary {
    name: String,
    slug: String,
    hq: String,
    hq_country: String,
    staff: u64,
    industry: String,
    tagline: String,
}

struct ConnectionMatch {
    name: String,
    headline: String,
    public_id: String,
}

struct ViewerMatch {
    name: String,
    headline: String,
}

struct MessageMatch {
    name: String,
    last_msg: String,
    conv_id: String,
}

struct PersonMatch {
    name: String,
    headline: String,
    degree: String,
    public_id: String,
}

fn summarize_company(company: &serde_json::Value, slug: &str) -> CompanySummary {
    let name = company
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(slug)
        .to_string();
    let hq = company
        .get("headquarter")
        .and_then(|h| h.get("city"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let hq_country = company
        .get("headquarter")
        .and_then(|h| h.get("country"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let staff = company
        .get("staffCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let industry = company
        .get("companyIndustries")
        .and_then(|arr| arr.as_array())
        .and_then(|a| a.first())
        .and_then(|i| i.get("localizedName"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let tagline = company
        .get("tagline")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    CompanySummary {
        name,
        slug: slug.to_string(),
        hq,
        hq_country,
        staff,
        industry,
        tagline,
    }
}

async fn scan_connections_at(
    client: &LinkedInClient,
    name_lower: &str,
    slug_lower: &str,
) -> CliResult<Vec<ConnectionMatch>> {
    let mut matches = Vec::new();
    let page_size = 40u32;
    let mut offset = 0u32;
    loop {
        let value = client
            .get_connections(offset, page_size)
            .await
            .map_err(|e| CliError::Other(format!("connections fetch failed: {e}")))?;
        let resp: ConnectionsResponse = serde_json::from_value(value)
            .map_err(|e| CliError::Other(format!("parse error: {e}")))?;

        matches.extend(
            resp.elements
                .iter()
                .filter_map(|el| match_connection_at(el, name_lower, slug_lower)),
        );

        let page_count = resp.elements.len() as u32;
        if scan_page_exhausted(page_count, page_size, offset, resp.paging.as_ref()) {
            break;
        }
        offset += page_count;
    }
    Ok(matches)
}

/// True when the current page is the last one: either short (fewer rows than
/// `page_size`) or we've reached `paging.total`.
fn scan_page_exhausted(
    page_count: u32,
    page_size: u32,
    offset: u32,
    paging: Option<&Paging>,
) -> bool {
    if page_count < page_size {
        return true;
    }
    paging
        .and_then(|p| p.total)
        .is_some_and(|total| offset + page_count >= total)
}

fn match_connection_at(
    element: &serde_json::Value,
    name_lower: &str,
    slug_lower: &str,
) -> Option<ConnectionMatch> {
    let mini = element.get("miniProfile")?;
    let headline = mini
        .get("occupation")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let h_lower = headline.to_lowercase();
    if !h_lower.contains(name_lower) && !h_lower.contains(slug_lower) {
        return None;
    }
    let first = mini.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
    let last = mini.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
    let public_id = mini
        .get("publicIdentifier")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    Some(ConnectionMatch {
        name: format!("{} {}", first, last).trim().to_string(),
        headline: headline.to_string(),
        public_id: public_id.to_string(),
    })
}

async fn scan_viewers_at(
    client: &LinkedInClient,
    company_name: &str,
    name_lower: &str,
) -> Vec<ViewerMatch> {
    let Ok(viewers_data) = client.get_profile_viewers().await else {
        return Vec::new();
    };
    let viewers_str = serde_json::to_string(&viewers_data).unwrap_or_default();
    if !viewers_str.to_lowercase().contains(name_lower) {
        return Vec::new();
    }
    let Some(elements) = viewers_data.get("elements").and_then(|e| e.as_array()) else {
        return Vec::new();
    };
    elements
        .iter()
        .filter(|el| viewer_card_mentions(el, name_lower))
        .map(|_| ViewerMatch {
            name: "(viewer from company)".to_string(),
            headline: company_name.to_string(),
        })
        .collect()
}

/// True when the WvmpViewersCard nested under `el` contains an insight card
/// whose serialized form mentions `name_lower`. The cards are deeply nested
/// under `value.com.linkedin.voyager.identity.me.WvmpViewersCard.insightCards`,
/// then again under each card's WvmpSummaryInsightCard.
fn viewer_card_mentions(el: &serde_json::Value, name_lower: &str) -> bool {
    let el_str = serde_json::to_string(el).unwrap_or_default();
    if !el_str.to_lowercase().contains(name_lower) {
        return false;
    }
    el.pointer("/value/com.linkedin.voyager.identity.me.WvmpViewersCard/insightCards")
        .and_then(|cards| cards.as_array())
        .and_then(|arr| arr.iter().find(|c| serialized_contains(c, name_lower)))
        .and_then(|card| {
            card.pointer("/value/com.linkedin.voyager.identity.me.WvmpSummaryInsightCard/cards")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.iter().find(|c| serialized_contains(c, name_lower)))
        })
        .is_some()
}

fn serialized_contains(value: &serde_json::Value, needle: &str) -> bool {
    serde_json::to_string(value)
        .unwrap_or_default()
        .to_lowercase()
        .contains(needle)
}

async fn scan_messages_with(client: &LinkedInClient, name_lower: &str) -> Vec<MessageMatch> {
    let Ok(conversations) = client.get_conversations(20, None).await else {
        return Vec::new();
    };
    let Some(elements) = conversations.get("elements").and_then(|e| e.as_array()) else {
        return Vec::new();
    };
    elements
        .iter()
        .filter(|conv| serialized_contains(conv, name_lower))
        .map(extract_message_match)
        .collect()
}

fn extract_message_match(conv: &serde_json::Value) -> MessageMatch {
    let names = extract_conversation_names(conv);
    let last_msg = conv
        .get("messages")
        .and_then(|m| m.get("elements"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(extract_message_body)
        .unwrap_or_default();
    let conv_id = conv
        .get("backendUrn")
        .and_then(|u| u.as_str())
        .and_then(|u| u.strip_prefix("urn:li:messagingThread:"))
        .unwrap_or("")
        .to_string();
    MessageMatch {
        name: names.join(", "),
        last_msg,
        conv_id,
    }
}

async fn search_people_at(
    client: &LinkedInClient,
    company_name: &str,
    name_lower: &str,
    slug_lower: &str,
) -> Vec<PersonMatch> {
    let search_results = client
        .search_people(company_name, 0, 5)
        .await
        .unwrap_or_default();
    let Some(elements) = search_results.get("elements").and_then(|e| e.as_array()) else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|el| el.get("items").and_then(|i| i.as_array()))
        .flatten()
        .filter_map(|item| match_search_person(item, name_lower, slug_lower))
        .collect()
}

fn match_search_person(
    item: &serde_json::Value,
    name_lower: &str,
    slug_lower: &str,
) -> Option<PersonMatch> {
    let er = item.pointer("/item/entityResult")?;
    let headline = er
        .get("primarySubtitle")
        .and_then(|t| t.get("text").and_then(|v| v.as_str()))
        .unwrap_or("");
    let h_lower = headline.to_lowercase();
    if !h_lower.contains(name_lower) && !h_lower.contains(slug_lower) {
        return None;
    }
    let title = er
        .get("title")
        .and_then(|t| t.get("text").and_then(|v| v.as_str()))
        .unwrap_or("");
    let badge = er
        .get("badgeText")
        .and_then(|t| t.get("text").and_then(|v| v.as_str()))
        .unwrap_or("");
    let nav_url = er
        .get("navigationUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let public_id = nav_url
        .strip_prefix("https://www.linkedin.com/in/")
        .and_then(|s| s.split('?').next())
        .unwrap_or("");
    Some(PersonMatch {
        name: title.to_string(),
        headline: headline.to_string(),
        degree: badge.to_string(),
        public_id: public_id.to_string(),
    })
}

fn print_who_json(f: &WhoFindings) -> CliResult<()> {
    let output = serde_json::json!({
        "company": {
            "name": f.company.name,
            "slug": f.company.slug,
            "hq": format!("{}, {}", f.company.hq, f.company.hq_country),
            "staff": f.company.staff,
            "industry": f.company.industry,
        },
        "connections": f.connections
            .iter()
            .map(|c| serde_json::json!({"name": c.name, "headline": c.headline, "publicId": c.public_id}))
            .collect::<Vec<_>>(),
        "viewers": f.viewers
            .iter()
            .map(|v| serde_json::json!({"name": v.name, "headline": v.headline}))
            .collect::<Vec<_>>(),
        "messages": f.messages
            .iter()
            .map(|m| serde_json::json!({"name": m.name, "lastMessage": m.last_msg, "conversationId": m.conv_id}))
            .collect::<Vec<_>>(),
        "people": f.people
            .iter()
            .map(|p| serde_json::json!({"name": p.name, "headline": p.headline, "degree": p.degree, "publicId": p.public_id}))
            .collect::<Vec<_>>(),
    });
    print_json(&output)
}

fn print_who_report(f: &WhoFindings) {
    print_company_header(&f.company);
    print_connections_section(&f.connections);
    print_viewers_section(&f.company.name, &f.viewers);
    print_messages_section(&f.messages);
    print_people_section(&f.company.name, &f.people);
}

fn print_company_header(company: &CompanySummary) {
    println!("{}", company.name);
    let mut meta = Vec::new();
    if !company.hq.is_empty() {
        if company.hq_country.is_empty() {
            meta.push(company.hq.clone());
        } else {
            meta.push(format!("{}, {}", company.hq, company.hq_country));
        }
    }
    if company.staff > 0 {
        meta.push(format!("{} employees", company.staff));
    }
    if !company.industry.is_empty() {
        meta.push(company.industry.clone());
    }
    if !meta.is_empty() {
        println!("  {}", meta.join("  |  "));
    }
    if !company.tagline.is_empty() {
        println!("  \"{}\"", truncate_with_ellipsis(&company.tagline, 80));
    }
    println!();
}

fn print_connections_section(connections: &[ConnectionMatch]) {
    println!("Your connections there: {}", connections.len());
    for c in connections {
        println!("  {} ({})", c.name, c.public_id);
        println!("    {}", truncate_with_ellipsis(&c.headline, 80));
    }
    println!();
}

fn print_viewers_section(company_name: &str, viewers: &[ViewerMatch]) {
    if viewers.is_empty() {
        return;
    }
    println!(
        "Recent profile viewers from {}: {}",
        company_name,
        viewers.len()
    );
    for v in viewers {
        println!("  {} — {}", v.name, v.headline);
    }
    println!();
}

fn print_messages_section(messages: &[MessageMatch]) {
    if messages.is_empty() {
        return;
    }
    println!("Recent messages:");
    for m in messages {
        println!("  {}", m.name);
        println!("    \"{}\"", truncate_with_ellipsis(&m.last_msg, 80));
        println!("    read: messages read {}", m.conv_id);
    }
    println!();
}

fn print_people_section(company_name: &str, people: &[PersonMatch]) {
    if people.is_empty() {
        println!("No people found at {} in search.", company_name);
        return;
    }
    println!("People at {} (from search):", company_name);
    for p in people {
        println!("  {} {} ({})", p.name, p.degree, p.public_id);
        println!("    {}", truncate_with_ellipsis(&p.headline, 80));
    }
}

/// Extract participant names from a conversation element.
pub fn extract_conversation_names(conv: &Value) -> Vec<String> {
    conv.get("conversationParticipants")
        .and_then(|p| p.as_array())
        .map(|participants| {
            participants
                .iter()
                .filter_map(|p| {
                    p.get("participantType")
                        .and_then(|pt| pt.get("member"))
                        .and_then(extract_member_name)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Handle `messages list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/messaging/conversations with
/// pagination params, and prints the results.
pub async fn cmd_messages_list(
    count: u32,
    cursor: Option<&str>,
    _created_before: Option<u64>,
    category: &str,
    raw_json: bool,
) -> CliResult<()> {
    let (client, _path) = load_session_client()?;
    let category: ConversationCategory =
        category.parse().map_err(|e: String| CliError::Other(e))?;

    if _created_before.is_some() {
        eprintln!("warning: --before is deprecated; use --cursor from metadata.nextCursor");
    }

    let value = client
        .get_conversations_by_category_with_cursor(count, cursor, category)
        .await?;

    if raw_json {
        return print_json(&value);
    }

    // The API client already unwraps the GraphQL envelope to
    // data.messengerConversationsByCategory, which contains { elements, paging }.
    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    println!("Conversations ({})", elements.len());
    println!("---");

    if elements.is_empty() {
        println!("(no conversations)");
        return Ok(());
    }

    for (i, element) in elements.iter().enumerate() {
        let idx = i + 1;
        print_graphql_conversation(idx, element);
        println!();
    }

    Ok(())
}

/// Handle `messages read <conversation_id> [--before TS] [--json]`.
///
/// Loads the session, calls GET /voyager/api/messaging/conversations/{id}/events
/// with cursor-based pagination, and prints the messages.
pub async fn cmd_messages_read(
    conversation_id: &str,
    cursor: Option<&str>,
    _created_before: Option<u64>,
    raw_json: bool,
) -> CliResult<()> {
    let (client, _path) = load_session_client()?;
    let conv_urn = ConversationUrn::from(conversation_id);

    if _created_before.is_some() {
        eprintln!("warning: --before is deprecated; use --cursor from metadata.nextCursor");
    }

    let value = client
        .get_conversation_events_with_cursor(&conv_urn, cursor)
        .await?;

    if raw_json {
        return print_json(&value);
    }

    // The API client already unwraps the GraphQL envelope to
    // data.messengerMessagesByConversation, which contains { elements, paging }.
    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    println!("Messages in {} ({})", conversation_id, elements.len());
    println!("---");

    if elements.is_empty() {
        println!("(no messages)");
        return Ok(());
    }

    for event in &elements {
        print_graphql_message(event);
        println!();
    }

    Ok(())
}

/// Handle `messages send <recipient> <message> [--yes] [--json]`.
///
/// Resolves the recipient's public identifier to an fsd_profile URN, then
/// sends a message via the REST messaging/conversations?action=create endpoint.
pub async fn cmd_messages_send(
    recipient: &str,
    message: &str,
    confirmed: bool,
    raw_json: bool,
) -> CliResult<()> {
    if !confirmed {
        return Err(CliError::Input(send_confirmation_error(recipient, message)));
    }

    let (client, _path) = load_session_client()?;
    let profile_urn = resolve_send_recipient(&client, recipient).await?;
    eprintln!("Recipient URN: {}", profile_urn);

    eprintln!("Sending message...");
    let value = client
        .send_message(&profile_urn, message)
        .await
        .map_err(|e| CliError::Other(format!("failed to send message: {e}")))?;

    if raw_json {
        print_json(&value)?;
    } else {
        println!("Message sent to {} ({})", recipient, profile_urn);
    }
    Ok(())
}

fn send_confirmation_error(recipient: &str, message: &str) -> String {
    let preview = if message.chars().count() > 80 {
        let truncated: String = message.chars().take(80).collect();
        format!("{}…", truncated)
    } else {
        message.to_string()
    };
    format!(
        "this will send a REAL DIRECT MESSAGE to {} that they will see in \
         their inbox: \"{}\". Pass --yes to confirm.",
        recipient, preview
    )
}

/// Resolve a `messages send` recipient. Accepts a URN, a vanity slug, or
/// a "First Last" name (fuzzy-matched against connections).
async fn resolve_send_recipient(client: &LinkedInClient, recipient: &str) -> CliResult<ProfileUrn> {
    if PROFILE_URN_PREFIXES
        .iter()
        .any(|p| recipient.starts_with(p))
    {
        eprintln!("Using provided URN directly.");
        return Ok(ProfileUrn::from(recipient));
    }
    if recipient.contains(' ') {
        eprintln!("Searching connections for '{}'...", recipient);
        return resolve_recipient_by_name(client, recipient).await;
    }
    eprintln!("Resolving profile URN for '{}'...", recipient);
    client
        .resolve_profile_urn(recipient)
        .await
        .map_err(|e| CliError::Other(format!("failed to resolve profile URN: {e}")))
}

/// One row of the name-fuzzy-match resolver. At least one of `slug` /
/// `urn` is always non-empty when this is constructed.
struct NameMatch {
    name: String,
    slug: String,
    urn: String,
}

/// Resolve a recipient name to a profile URN by searching connections.
///
/// Fetches connections (up to 200) and finds the best match for the
/// given name (case-insensitive substring match on first+last). If
/// multiple matches are found, lists them and asks the user to be more
/// specific.
async fn resolve_recipient_by_name(client: &LinkedInClient, name: &str) -> CliResult<ProfileUrn> {
    let matches = scan_connections_for_name(client, &name.to_lowercase()).await?;
    pick_unique_match(client, name, matches).await
}

/// Page through connections collecting every entry whose full name
/// contains `name_lower`. Caps at 200 connections scanned.
async fn scan_connections_for_name(
    client: &LinkedInClient,
    name_lower: &str,
) -> CliResult<Vec<NameMatch>> {
    let page_size = 40u32;
    let max_scan = 200u32;
    let mut offset = 0u32;
    let mut matches: Vec<NameMatch> = Vec::new();

    loop {
        let value = client
            .get_connections(offset, page_size)
            .await
            .map_err(|e| CliError::Other(format!("failed to fetch connections: {e}")))?;
        let elements = elements_slice(&value);

        for conn in elements {
            if let Some(m) = match_connection_by_name(conn, name_lower) {
                matches.push(m);
            }
        }

        let page_count = elements.len() as u32;
        if page_count < page_size || offset + page_count >= max_scan {
            break;
        }
        offset += page_count;
    }

    Ok(matches)
}

fn match_connection_by_name(conn: &Value, name_lower: &str) -> Option<NameMatch> {
    let mini = conn.get("miniProfile")?;
    let first = mini.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
    let last = mini.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
    let full_name = format!("{} {}", first, last);
    if !full_name.to_lowercase().contains(name_lower) {
        return None;
    }
    let slug = mini
        .get("publicIdentifier")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let urn = mini
        .get("dashEntityUrn")
        .or_else(|| mini.get("entityUrn"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    Some(NameMatch {
        name: full_name.trim().to_string(),
        slug: slug.to_string(),
        urn: urn.to_string(),
    })
}

/// Resolve a single match to an fsd_profile URN. Returns an ambiguity
/// error if multiple matches were collected, or a not-found error if
/// none were.
async fn pick_unique_match(
    client: &LinkedInClient,
    name: &str,
    matches: Vec<NameMatch>,
) -> CliResult<ProfileUrn> {
    match matches.len() {
        0 => Err(CliError::Other(format!(
            "no connection found matching '{}'. Try a vanity slug instead.",
            name
        ))),
        1 => urn_for_match(client, &matches[0]).await,
        _ => Err(CliError::Input(format_ambiguous_error(name, &matches))),
    }
}

async fn urn_for_match(client: &LinkedInClient, m: &NameMatch) -> CliResult<ProfileUrn> {
    eprintln!("Matched: {} ({})", m.name, m.slug);
    if !m.urn.is_empty() {
        return Ok(ProfileUrn::from(
            m.urn.replace("fs_miniProfile", "fsd_profile"),
        ));
    }
    if !m.slug.is_empty() {
        return client
            .resolve_profile_urn(&m.slug)
            .await
            .map_err(|e| CliError::Other(format!("failed to resolve profile: {e}")));
    }
    Err(CliError::Other(
        "matched connection has no URN or slug".to_string(),
    ))
}

fn format_ambiguous_error(name: &str, matches: &[NameMatch]) -> String {
    eprintln!("Multiple matches for '{}':", name);
    for (i, m) in matches.iter().enumerate() {
        eprintln!("  [{}] {} ({})", i + 1, m.name, m.slug);
    }
    format!(
        "ambiguous name '{}': {} matches found. Use a more specific name or the vanity slug.",
        name,
        matches.len()
    )
}

/// Handle `messages reply <conversation_id> <message> [--yes] [--json]`.
///
/// Sends a reply to an existing conversation thread.
pub async fn cmd_messages_reply(
    conversation_id: &str,
    message: &str,
    confirmed: bool,
    raw_json: bool,
) -> CliResult<()> {
    if !confirmed {
        return preview_thread_then_abort(conversation_id, message).await;
    }

    let (client, _path) = load_session_client()?;
    let conv_urn = ConversationUrn::from(conversation_id);
    eprintln!("Replying to conversation {}...", conversation_id);
    let value = client
        .reply_to_conversation(&conv_urn, message)
        .await
        .map_err(|e| CliError::Other(format!("failed to send reply: {e}")))?;

    if raw_json {
        print_json(&value)?;
    } else {
        println!("Reply sent to conversation {}", conversation_id);
    }
    Ok(())
}

async fn preview_thread_then_abort(conversation_id: &str, message: &str) -> CliResult<()> {
    let (client, _path) = load_session_client()?;
    let conv_urn = ConversationUrn::from(conversation_id);
    let events = client
        .get_conversation_events(&conv_urn, None)
        .await
        .map_err(|e| CliError::Other(format!("failed to load conversation: {e}")))?;
    let elements = elements_slice(&events);
    let show = elements
        .get(elements.len().saturating_sub(3)..)
        .unwrap_or(elements);

    eprintln!("--- Last messages in thread ---");
    for msg in show {
        let sender = sender_display_name(msg);
        let body = extract_message_body(msg).unwrap_or_default();
        eprintln!("  {}: {}", sender, truncate_with_ellipsis(&body, 100));
    }
    eprintln!("---");
    eprintln!("Your reply: {}", message);
    eprintln!();
    Err(CliError::Input(
        "this will send a REAL MESSAGE in this LinkedIn conversation. Pass --yes to confirm."
            .to_string(),
    ))
}

/// Best-effort sender label. Prefers the structured member name; falls
/// back to the trailing segment of `hostIdentityUrn`, then "unknown".
fn sender_display_name(msg: &Value) -> String {
    let sender = msg.get("sender");
    let from_member = sender
        .and_then(|s| s.get("participantType"))
        .and_then(|pt| pt.get("member"))
        .and_then(extract_member_name);
    if let Some(name) = from_member {
        return name;
    }
    sender
        .and_then(|s| s.get("hostIdentityUrn"))
        .and_then(|u| u.as_str())
        .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
        .unwrap_or("unknown")
        .to_string()
}
