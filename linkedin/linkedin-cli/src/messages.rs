use linkedin_api::client::LinkedInClient;
use linkedin_api::models::ConnectionsResponse;

use crate::graphql_print::{print_graphql_conversation, print_graphql_message};
use crate::session::load_session_client;
use crate::spam::{is_spam_conversation, is_spam_invitation};
use crate::util::truncate_with_ellipsis;

/// Handle `inbox [--json] [--all]`.
///
/// Shows a daily summary: unread messages, pending invitations, and
/// recent unread notifications in one view. Filters likely recruiter
/// spam by default; use `--all` to see everything.
pub async fn cmd_inbox(raw_json: bool, show_all: bool) -> Result<(), String> {
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
    let conversations = conversations_res.map_err(|e| format!("failed to fetch messages: {e}"))?;
    let invitations = invitations_res.map_err(|e| format!("failed to fetch invitations: {e}"))?;
    let notifications =
        notifications_res.map_err(|e| format!("failed to fetch notifications: {e}"))?;

    if raw_json {
        let combined = serde_json::json!({
            "unreadMessages": conversations,
            "pendingInvitations": invitations,
            "recentNotifications": notifications,
        });
        let pretty = serde_json::to_string_pretty(&combined)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // --- Unread messages ---
    let conv_elements = conversations
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let unread: Vec<&serde_json::Value> = conv_elements
        .iter()
        .filter(|c| {
            c.get("read").and_then(|r| r.as_bool()) == Some(false)
                || c.get("unreadCount").and_then(|n| n.as_u64()).unwrap_or(0) > 0
        })
        .collect();

    let spam_msg_count = if show_all {
        0
    } else {
        unread.iter().filter(|c| is_spam_conversation(c)).count()
    };
    let displayed_unread: Vec<&serde_json::Value> = unread
        .into_iter()
        .filter(|c| show_all || !is_spam_conversation(c))
        .collect();

    println!("Unread Messages ({})", displayed_unread.len());
    println!("---");
    if displayed_unread.is_empty() {
        println!("  (all caught up)");
    } else {
        for (i, conv) in displayed_unread.iter().enumerate() {
            let title = conv.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let names = extract_conversation_names(conv);
            let display = if !title.is_empty() {
                title.to_string()
            } else if !names.is_empty() {
                names.join(", ")
            } else {
                "(unknown)".to_string()
            };

            let backend_urn = conv
                .get("backendUrn")
                .and_then(|u| u.as_str())
                .unwrap_or("");
            let conv_id = backend_urn
                .strip_prefix("urn:li:messagingThread:")
                .unwrap_or(backend_urn);

            let last_msg = conv
                .get("messages")
                .and_then(|m| m.get("elements"))
                .and_then(|e| e.as_array())
                .and_then(|arr| arr.first())
                .and_then(|msg| {
                    msg.get("body").and_then(|b| {
                        if b.is_string() {
                            b.as_str().map(|s| s.to_string())
                        } else {
                            b.get("text")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string())
                        }
                    })
                })
                .unwrap_or_default();

            let unread_count = conv
                .get("unreadCount")
                .and_then(|n| n.as_u64())
                .unwrap_or(1);

            println!("  [{}] {} ({} unread)", i + 1, display, unread_count);
            if !last_msg.is_empty() {
                println!("      {}", truncate_with_ellipsis(&last_msg, 80));
            }
            println!("      read: messages read {}", conv_id);
        }
    }
    if spam_msg_count > 0 {
        println!(
            "  ({} recruiter message(s) hidden, use --all to show)",
            spam_msg_count
        );
    }
    println!();

    // --- Pending invitations ---
    let inv_elements = invitations
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let spam_inv_count = if show_all {
        0
    } else {
        inv_elements
            .iter()
            .filter(|i| is_spam_invitation(i))
            .count()
    };
    let displayed_invs: Vec<&serde_json::Value> = inv_elements
        .iter()
        .filter(|i| show_all || !is_spam_invitation(i))
        .collect();

    println!("Pending Invitations ({})", displayed_invs.len());
    println!("---");
    if displayed_invs.is_empty() {
        println!("  (none)");
    } else {
        for (i, inv) in displayed_invs.iter().enumerate() {
            let name = inv
                .get("title")
                .and_then(|t| t.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("(unknown)");
            let headline = inv
                .get("subtitle")
                .and_then(|t| t.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sent_time = inv
                .get("sentTimeLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let invitation_id = inv
                .get("invitation")
                .and_then(|i| i.get("entityUrn"))
                .and_then(|v| v.as_str())
                .and_then(|u| u.rsplit(':').next())
                .unwrap_or("");
            let secret = inv
                .get("invitation")
                .and_then(|i| i.get("sharedSecret"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            print!("  [{}] {}", i + 1, name);
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
    }
    if spam_inv_count > 0 {
        println!(
            "  ({} recruiter invitation(s) hidden, use --all to show)",
            spam_inv_count
        );
    }
    println!();

    // --- Recent notifications (unread only) ---
    let notif_elements = notifications
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let unread_notifs: Vec<&serde_json::Value> = notif_elements
        .iter()
        .filter(|n| n.get("read").and_then(|r| r.as_bool()) == Some(false))
        .collect();

    println!("Unread Notifications ({})", unread_notifs.len());
    println!("---");
    if unread_notifs.is_empty() {
        println!("  (all read)");
    } else {
        for (i, notif) in unread_notifs.iter().take(5).enumerate() {
            let headline = notif
                .get("headline")
                .and_then(|h| h.get("text").and_then(|t| t.as_str()))
                .unwrap_or("(no headline)");
            let kicker = notif
                .get("kicker")
                .and_then(|k| k.get("text").and_then(|t| t.as_str()))
                .unwrap_or("");

            print!("  [{}] {}", i + 1, truncate_with_ellipsis(headline, 80));
            if !kicker.is_empty() {
                print!("  ({})", kicker);
            }
            println!();
        }
        if unread_notifs.len() > 5 {
            println!("  ... and {} more", unread_notifs.len() - 5);
        }
    }

    Ok(())
}

/// Handle `who <company> [--json]`.
///
/// Shows your network overlap with a company: connections who work there,
/// profile viewers from there, recent messages with people there, and
/// key people at the company.
pub async fn cmd_who(company_slug: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    eprintln!("Looking up {}...", company_slug);
    let company_data = client
        .get_company(company_slug)
        .await
        .map_err(|e| format!("company lookup failed: {e}"))?;
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

    if raw_json {
        return print_who_json(&company, &connections, &viewers, &messages, &people);
    }
    print_who_report(&company, &connections, &viewers, &messages, &people);
    Ok(())
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
) -> Result<Vec<ConnectionMatch>, String> {
    let mut matches = Vec::new();
    let page_size = 40u32;
    let mut offset = 0u32;
    loop {
        let value = client
            .get_connections(offset, page_size)
            .await
            .map_err(|e| format!("connections fetch failed: {e}"))?;
        let resp: ConnectionsResponse =
            serde_json::from_value(value).map_err(|e| format!("parse error: {e}"))?;

        for element in &resp.elements {
            if let Some(m) = match_connection_at(element, name_lower, slug_lower) {
                matches.push(m);
            }
        }

        let page_count = resp.elements.len() as u32;
        if page_count < page_size {
            break;
        }
        if let Some(total) = resp.paging.as_ref().and_then(|p| p.total) {
            if offset + page_count >= total {
                break;
            }
        }
        offset += page_count;
    }
    Ok(matches)
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

/// Message bodies come back as either a bare string or a `{text: "..."}`
/// object depending on how the conversation was rendered.
fn extract_message_body(msg: &serde_json::Value) -> Option<String> {
    let body = msg.get("body")?;
    if body.is_string() {
        return body.as_str().map(str::to_string);
    }
    body.get("text").and_then(|t| t.as_str()).map(str::to_string)
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

fn print_who_json(
    company: &CompanySummary,
    connections: &[ConnectionMatch],
    viewers: &[ViewerMatch],
    messages: &[MessageMatch],
    people: &[PersonMatch],
) -> Result<(), String> {
    let output = serde_json::json!({
        "company": {
            "name": company.name,
            "slug": company.slug,
            "hq": format!("{}, {}", company.hq, company.hq_country),
            "staff": company.staff,
            "industry": company.industry,
        },
        "connections": connections
            .iter()
            .map(|c| serde_json::json!({"name": c.name, "headline": c.headline, "publicId": c.public_id}))
            .collect::<Vec<_>>(),
        "viewers": viewers
            .iter()
            .map(|v| serde_json::json!({"name": v.name, "headline": v.headline}))
            .collect::<Vec<_>>(),
        "messages": messages
            .iter()
            .map(|m| serde_json::json!({"name": m.name, "lastMessage": m.last_msg, "conversationId": m.conv_id}))
            .collect::<Vec<_>>(),
        "people": people
            .iter()
            .map(|p| serde_json::json!({"name": p.name, "headline": p.headline, "degree": p.degree, "publicId": p.public_id}))
            .collect::<Vec<_>>(),
    });
    let pretty =
        serde_json::to_string_pretty(&output).map_err(|e| format!("JSON format error: {e}"))?;
    println!("{}", pretty);
    Ok(())
}

fn print_who_report(
    company: &CompanySummary,
    connections: &[ConnectionMatch],
    viewers: &[ViewerMatch],
    messages: &[MessageMatch],
    people: &[PersonMatch],
) {
    print_company_header(company);
    print_connections_section(connections);
    print_viewers_section(&company.name, viewers);
    print_messages_section(messages);
    print_people_section(&company.name, people);
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
pub fn extract_conversation_names(conv: &serde_json::Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(participants) = conv
        .get("conversationParticipants")
        .and_then(|p| p.as_array())
    {
        for p in participants {
            if let Some(name) = p
                .get("participantType")
                .and_then(|pt| pt.get("member"))
                .and_then(|member| {
                    let first = member
                        .get("firstName")
                        .and_then(|f| {
                            f.get("text")
                                .and_then(|v| v.as_str())
                                .or_else(|| f.as_str())
                        })
                        .unwrap_or("");
                    let last = member
                        .get("lastName")
                        .and_then(|l| {
                            l.get("text")
                                .and_then(|v| v.as_str())
                                .or_else(|| l.as_str())
                        })
                        .unwrap_or("");
                    if first.is_empty() && last.is_empty() {
                        None
                    } else {
                        Some(format!("{} {}", first, last).trim().to_string())
                    }
                })
            {
                names.push(name);
            }
        }
    }
    names
}

/// Handle `messages list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/messaging/conversations with
/// pagination params, and prints the results.
pub async fn cmd_messages_list(
    count: u32,
    created_before: Option<u64>,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_conversations(count, created_before)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
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
    created_before: Option<u64>,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_conversation_events(conversation_id, created_before)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
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
) -> Result<(), String> {
    if !confirmed {
        let preview = if message.chars().count() > 80 {
            let truncated: String = message.chars().take(80).collect();
            format!("{}…", truncated)
        } else {
            message.to_string()
        };
        return Err(format!(
            "this will send a REAL DIRECT MESSAGE to {} that they will see in \
             their inbox: \"{}\". Pass --yes to confirm.",
            recipient, preview
        ));
    }

    let (client, _path) = load_session_client()?;

    // Resolve recipient to fsd_profile URN. Accepts:
    // - Direct URN: urn:li:fsd_profile:ACoAABivN...
    // - Vanity slug: john-doe-123
    // - Name with spaces: "Paul Bang" (fuzzy-matched against connections)
    let profile_urn = if recipient.starts_with("urn:li:fsd_profile:")
        || recipient.starts_with("urn:li:member:")
        || recipient.starts_with("urn:li:fs_miniProfile:")
    {
        eprintln!("Using provided URN directly.");
        recipient.to_string()
    } else if recipient.contains(' ') {
        // Name-based lookup: search connections for a match.
        eprintln!("Searching connections for '{}'...", recipient);
        resolve_recipient_by_name(&client, recipient).await?
    } else {
        eprintln!("Resolving profile URN for '{}'...", recipient);
        client
            .resolve_profile_urn(recipient)
            .await
            .map_err(|e| format!("failed to resolve profile URN: {e}"))?
    };
    eprintln!("Recipient URN: {}", profile_urn);

    // Send the message.
    eprintln!("Sending message...");
    let value = client
        .send_message(&profile_urn, message)
        .await
        .map_err(|e| format!("failed to send message: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Message sent to {} ({})", recipient, profile_urn);
    }

    Ok(())
}

/// Resolve a recipient name to a profile URN by searching connections.
///
/// Fetches connections and finds the best match for the given name
/// (case-insensitive substring match on first+last name). If multiple
/// matches are found, lists them and asks the user to be more specific.
async fn resolve_recipient_by_name(client: &LinkedInClient, name: &str) -> Result<String, String> {
    let name_lower = name.to_lowercase();

    // Search through connections (fetch up to 200 to get a good match).
    let mut offset = 0u32;
    let page_size = 40u32;
    let mut matches: Vec<(String, String, String)> = Vec::new(); // (name, slug, urn)

    loop {
        let value = client
            .get_connections(offset, page_size)
            .await
            .map_err(|e| format!("failed to fetch connections: {e}"))?;

        let elements = value
            .get("elements")
            .and_then(|e| e.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        for conn in elements {
            let mini = match conn.get("miniProfile") {
                Some(m) => m,
                None => continue,
            };

            let first = mini.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
            let last = mini.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
            let full_name = format!("{} {}", first, last);
            let slug = mini
                .get("publicIdentifier")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let urn = mini
                .get("dashEntityUrn")
                .or_else(|| mini.get("entityUrn"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if full_name.to_lowercase().contains(&name_lower) {
                matches.push((
                    full_name.trim().to_string(),
                    slug.to_string(),
                    urn.to_string(),
                ));
            }
        }

        let page_count = elements.len() as u32;
        if page_count < page_size || offset + page_count >= 200 {
            break;
        }
        offset += page_count;
    }

    match matches.len() {
        0 => Err(format!(
            "no connection found matching '{}'. Try a vanity slug instead.",
            name
        )),
        1 => {
            let (matched_name, slug, urn) = &matches[0];
            eprintln!("Matched: {} ({})", matched_name, slug);
            let profile_urn = if !urn.is_empty() {
                // Convert fs_miniProfile URN to fsd_profile if needed.
                urn.replace("fs_miniProfile", "fsd_profile")
            } else if !slug.is_empty() {
                client
                    .resolve_profile_urn(slug)
                    .await
                    .map_err(|e| format!("failed to resolve profile: {e}"))?
            } else {
                return Err("matched connection has no URN or slug".to_string());
            };
            Ok(profile_urn)
        }
        _ => {
            eprintln!("Multiple matches for '{}':", name);
            for (i, (n, s, _)) in matches.iter().enumerate() {
                eprintln!("  [{}] {} ({})", i + 1, n, s);
            }
            Err(format!(
                "ambiguous name '{}': {} matches found. Use a more specific name or the vanity slug.",
                name,
                matches.len()
            ))
        }
    }
}

/// Handle `messages reply <conversation_id> <message> [--yes] [--json]`.
///
/// Sends a reply to an existing conversation thread.
pub async fn cmd_messages_reply(
    conversation_id: &str,
    message: &str,
    confirmed: bool,
    raw_json: bool,
) -> Result<(), String> {
    if !confirmed {
        // Show the last few messages for context before confirming.
        let (client, _path) = load_session_client()?;
        let events = client
            .get_conversation_events(conversation_id, None)
            .await
            .map_err(|e| format!("failed to load conversation: {e}"))?;

        let elements = events
            .get("elements")
            .and_then(|e| e.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        eprintln!("--- Last messages in thread ---");
        // Show last 3 messages for context.
        let show = if elements.len() > 3 {
            &elements[elements.len() - 3..]
        } else {
            elements
        };
        for msg in show {
            let sender = msg
                .get("sender")
                .and_then(|s| s.get("participantType"))
                .and_then(|pt| pt.get("member"))
                .and_then(|m| {
                    let first = m
                        .get("firstName")
                        .and_then(|f| {
                            f.get("text")
                                .and_then(|v| v.as_str())
                                .or_else(|| f.as_str())
                        })
                        .unwrap_or("");
                    let last = m
                        .get("lastName")
                        .and_then(|l| {
                            l.get("text")
                                .and_then(|v| v.as_str())
                                .or_else(|| l.as_str())
                        })
                        .unwrap_or("");
                    if first.is_empty() && last.is_empty() {
                        None
                    } else {
                        Some(format!("{} {}", first, last).trim().to_string())
                    }
                })
                .unwrap_or_else(|| {
                    msg.get("sender")
                        .and_then(|s| s.get("hostIdentityUrn"))
                        .and_then(|u| u.as_str())
                        .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
                        .unwrap_or("unknown")
                        .to_string()
                });
            let body = msg
                .get("body")
                .and_then(|b| {
                    if b.is_string() {
                        b.as_str().map(|s| s.to_string())
                    } else {
                        b.get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                    }
                })
                .unwrap_or_default();
            eprintln!("  {}: {}", sender, truncate_with_ellipsis(&body, 100));
        }
        eprintln!("---");
        eprintln!("Your reply: {}", message);
        eprintln!();
        return Err(
            "this will send a REAL MESSAGE in this LinkedIn conversation. Pass --yes to confirm."
                .to_string(),
        );
    }

    let (client, _path) = load_session_client()?;

    eprintln!("Replying to conversation {}...", conversation_id);
    let value = client
        .reply_to_conversation(conversation_id, message)
        .await
        .map_err(|e| format!("failed to send reply: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Reply sent to conversation {}", conversation_id);
    }

    Ok(())
}
