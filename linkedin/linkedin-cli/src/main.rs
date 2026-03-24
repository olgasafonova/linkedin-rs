use std::fs;
use std::process;

use clap::{Parser, Subcommand};
use linkedin_api::auth::Session;
use linkedin_api::client::LinkedInClient;
use linkedin_api::models::{
    ConnectionsResponse, FeedResponse, NotificationCardsResponse, Paging, SearchResponse,
};

#[derive(Parser)]
#[command(name = "linkedin-cli")]
#[command(about = "CLI for LinkedIn API", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with LinkedIn
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// View profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Messaging operations
    Messages {
        #[command(subcommand)]
        action: MessagesAction,
    },
    /// Feed and posts
    Feed {
        #[command(subcommand)]
        action: FeedAction,
    },
    /// Connection management
    Connections {
        #[command(subcommand)]
        action: ConnectionsAction,
    },
    /// Search LinkedIn
    Search {
        #[command(subcommand)]
        action: SearchAction,
    },
    /// Notifications
    Notifications {
        #[command(subcommand)]
        action: NotificationsAction,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Log in by providing a li_at cookie value
    Login {
        /// li_at cookie value from browser dev tools.
        /// Also accepts LINKEDIN_LI_AT environment variable.
        #[arg(long = "li-at")]
        li_at: Option<String>,
    },
    /// Check session status by calling the LinkedIn API
    Status {
        /// Only check locally (do not make an API call)
        #[arg(long)]
        local: bool,
    },
    /// Log out and clear stored session
    Logout,
}

#[derive(Subcommand)]
enum FeedAction {
    /// List feed updates
    List {
        /// Number of feed items to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Fetch the authenticated user's own profile
    Me {
        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// View a profile by public identifier (vanity URL slug)
    View {
        /// LinkedIn public identifier (vanity URL slug, e.g. john-doe-123)
        public_id: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ConnectionsAction {
    /// List connections
    List {
        /// Number of connections to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SearchAction {
    /// Search for people by keywords
    People {
        /// Search keywords
        keywords: String,

        /// Number of results to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum NotificationsAction {
    /// List notification cards
    List {
        /// Number of notifications to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum MessagesAction {
    /// List conversations
    List {
        /// Number of conversations to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Cursor for pagination: epoch-millis timestamp to fetch conversations created before
        #[arg(long)]
        before: Option<u64>,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Read messages in a conversation
    Read {
        /// Conversation ID (thread ID portion of URN, e.g. 2-abc123)
        conversation_id: String,

        /// Cursor for pagination: epoch-millis timestamp to fetch messages created before
        #[arg(long)]
        before: Option<u64>,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Send a message to a connection
    Send {
        /// LinkedIn public identifier (vanity URL slug, e.g. john-doe-123)
        recipient: String,

        /// Message text to send
        message: String,

        /// Output raw JSON response instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { action } => match action {
            AuthAction::Login { li_at } => {
                if let Err(e) = cmd_auth_login(li_at).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            AuthAction::Status { local } => {
                if let Err(e) = cmd_auth_status(local).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            AuthAction::Logout => {
                if let Err(e) = cmd_auth_logout() {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Profile { action } => match action {
            ProfileAction::Me { json } => {
                if let Err(e) = cmd_profile_me(json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            ProfileAction::View { public_id, json } => {
                if let Err(e) = cmd_profile_view(&public_id, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Messages { action } => match action {
            MessagesAction::List {
                count,
                before,
                json,
            } => {
                if let Err(e) = cmd_messages_list(count, before, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            MessagesAction::Read {
                conversation_id,
                before,
                json,
            } => {
                if let Err(e) = cmd_messages_read(&conversation_id, before, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            MessagesAction::Send {
                recipient,
                message,
                json,
            } => {
                if let Err(e) = cmd_messages_send(&recipient, &message, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Feed { action } => match action {
            FeedAction::List { count, start, json } => {
                if let Err(e) = cmd_feed_list(start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Connections { action } => match action {
            ConnectionsAction::List { count, start, json } => {
                if let Err(e) = cmd_connections_list(start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Search { action } => match action {
            SearchAction::People {
                keywords,
                count,
                start,
                json,
            } => {
                if let Err(e) = cmd_search_people(&keywords, start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Notifications { action } => match action {
            NotificationsAction::List { count, start, json } => {
                if let Err(e) = cmd_notifications_list(start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
    }
}

/// Handle `auth login --li-at <value>`.
///
/// Resolves the li_at value from the CLI flag or the `LINKEDIN_LI_AT`
/// environment variable. Generates a fresh JSESSIONID, creates a Session,
/// and saves it to the default path.
async fn cmd_auth_login(li_at_flag: Option<String>) -> Result<(), String> {
    let li_at = li_at_flag
        .or_else(|| std::env::var("LINKEDIN_LI_AT").ok())
        .ok_or_else(|| {
            "li_at cookie value required: use --li-at <value> or set LINKEDIN_LI_AT env var"
                .to_string()
        })?;

    if li_at.trim().is_empty() {
        return Err("li_at cookie value must not be empty".to_string());
    }

    // Generate a fresh JSESSIONID for this session.
    let client = LinkedInClient::new().map_err(|e| format!("failed to create client: {e}"))?;
    let jsessionid = client.jsessionid().to_string();

    let session = Session::new(li_at, jsessionid);
    let path = Session::default_path().map_err(|e| format!("{e}"))?;
    session.save(&path).map_err(|e| format!("{e}"))?;

    println!("Session saved to {}", path.display());
    println!("JSESSIONID: {}...", truncate(&session.jsessionid, 10));
    println!("li_at: {}...", truncate(&session.li_at, 10));
    Ok(())
}

/// Handle `auth status [--local]`.
///
/// Without `--local`, loads the session and calls GET /voyager/api/me to verify
/// the session is still valid server-side. With `--local`, only checks the
/// session file on disk (no network request).
async fn cmd_auth_status(local_only: bool) -> Result<(), String> {
    let path = Session::default_path().map_err(|e| format!("{e}"))?;

    if !path.exists() {
        println!("No session found at {}", path.display());
        println!("Status: not logged in");
        return Ok(());
    }

    let session = Session::load(&path).map_err(|e| format!("{e}"))?;

    println!("Session file: {}", path.display());
    println!("Created at: {}", session.created_at);
    println!("JSESSIONID: {}...", truncate(&session.jsessionid, 10));
    println!("li_at: {}...", truncate(&session.li_at, 10));

    if !session.is_valid() {
        println!("Status: invalid (empty li_at cookie)");
        return Ok(());
    }

    if local_only {
        println!("Status: valid (local check only -- session may be expired server-side)");
        return Ok(());
    }

    // Hit the live API to verify the session is actually valid.
    println!("Checking session against LinkedIn API...");
    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

    match client.get_me().await {
        Ok(me) => {
            println!("Status: authenticated");
            // Try to extract a display name from the response.
            if let Some(mini) = me.get("miniProfile") {
                let first = mini.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
                let last = mini.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
                if !first.is_empty() || !last.is_empty() {
                    println!("Logged in as: {} {}", first, last);
                }
            }
            if let Some(id) = me.get("plainId").and_then(|v| v.as_i64()) {
                println!("Member ID: {}", id);
            }
            Ok(())
        }
        Err(e) => {
            println!("Status: session invalid or expired");
            println!("API error: {e}");
            Ok(())
        }
    }
}

/// Handle `auth logout`.
///
/// Deletes the session file from disk.
fn cmd_auth_logout() -> Result<(), String> {
    let path = Session::default_path().map_err(|e| format!("{e}"))?;

    if !path.exists() {
        println!("No session file found at {}", path.display());
        return Ok(());
    }

    fs::remove_file(&path).map_err(|e| format!("failed to remove session file: {e}"))?;
    println!("Session removed: {}", path.display());
    Ok(())
}

/// Handle `profile me [--json]`.
///
/// Loads the session, creates a client, calls GET /voyager/api/me, and
/// prints the result. With `--json`, outputs raw pretty-printed JSON.
/// Without `--json`, outputs a human-readable summary.
async fn cmd_profile_me(raw_json: bool) -> Result<(), String> {
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

/// Handle `profile view <public_id> [--json]`.
///
/// Loads the session, creates a client, calls the identity/profiles endpoint
/// with decoration for full field projection, and prints the result.
async fn cmd_profile_view(public_id: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let profile = client
        .get_profile(public_id)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty = serde_json::to_string_pretty(&profile)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        print_profile_summary(&profile);
    }

    Ok(())
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

/// Handle `feed list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/feed/updates?q=findFeed with
/// pagination params, and prints the results.
async fn cmd_feed_list(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_feed(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // Try to parse into our typed FeedResponse for structured output.
    let feed: FeedResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse feed response: {e}"))?;

    // Print paging info.
    if let Some(ref paging) = feed.paging {
        print_paging_header("Feed updates", paging);
    }
    println!("---");

    if feed.elements.is_empty() {
        println!("(no feed items)");
        return Ok(());
    }

    for (i, element) in feed.elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_feed_item(idx, element);
        println!();
    }

    Ok(())
}

/// Print a brief human-readable summary of a single feed item.
///
/// Feed items are `UpdateV2` records. We extract what we can and skip
/// unknown fields gracefully. The real structure is deeply nested, so
/// this is best-effort until we've validated against live data.
fn print_feed_item(index: usize, item: &serde_json::Value) {
    // The real feed response wraps the UpdateV2 payload inside:
    //   element.value["com.linkedin.voyager.feed.render.UpdateV2"]
    // This is LinkedIn's Rest.li union encoding. Unwrap it first,
    // falling back to the element itself for forward-compatibility.
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    // Try to extract actor name.
    let actor_name = update
        .get("actor")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(unknown author)");

    // Try to extract commentary text.
    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Truncate long commentary for the summary view.
    let commentary_display = truncate_with_ellipsis(commentary, 120);

    // Entity URN -- lives at the top-level element, not inside the UpdateV2.
    let urn = item.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");

    // Social counts are inside the UpdateV2 payload.
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

    println!(
        "[{}] {} {}",
        index,
        actor_name,
        if !urn.is_empty() { urn } else { "" }
    );
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    println!("    likes: {}  comments: {}", likes, comments);
}

/// Handle `messages list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/messaging/conversations with
/// pagination params, and prints the results.
async fn cmd_messages_list(
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
async fn cmd_messages_read(
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

/// Handle `messages send <recipient> <message> [--json]`.
///
/// Resolves the recipient's public identifier to an fsd_profile URN, then
/// sends a message via the REST messaging/conversations?action=create endpoint.
async fn cmd_messages_send(recipient: &str, message: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // Resolve public identifier to fsd_profile URN.
    // The recipient can be either a public_id (vanity URL slug) or a direct
    // fsd_profile URN like "urn:li:fsd_profile:ACoAABivN...".
    let profile_urn = if recipient.starts_with("urn:li:fsd_profile:")
        || recipient.starts_with("urn:li:member:")
        || recipient.starts_with("urn:li:fs_miniProfile:")
    {
        eprintln!("Using provided URN directly.");
        recipient.to_string()
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

/// Handle `connections list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/relationships/connections with
/// pagination params sorted by RECENTLY_ADDED, and prints the results.
async fn cmd_connections_list(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

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
    println!("---");

    if resp.elements.is_empty() {
        println!("(no connections)");
        return Ok(());
    }

    for (i, element) in resp.elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_connection(idx, element);
        println!();
    }

    Ok(())
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

/// Handle `search people <keywords> [--count N] [--start N] [--json]`.
///
/// Loads the session, calls the Voyager GraphQL `searchDashClustersByAll`
/// endpoint and prints the results.
async fn cmd_search_people(
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

/// Handle `notifications list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls the Voyager GraphQL notifications endpoint
/// (`identityDashNotificationCardsByFilterVanityName`) with pagination
/// params, and prints the results.
async fn cmd_notifications_list(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_notifications(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let resp: NotificationCardsResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse notifications response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header("Notifications", paging);
    }
    println!("---");

    if resp.elements.is_empty() {
        println!("(no notifications)");
        return Ok(());
    }

    for (i, element) in resp.elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_notification_card(idx, element);
        println!();
    }

    Ok(())
}

/// Print a brief human-readable summary of a single notification card.
///
/// Notification cards use TextViewModel wrappers for text fields; we extract
/// the inner `text` string from `headline`, `subHeadline`, and `kicker`.
fn print_notification_card(index: usize, card: &serde_json::Value) {
    let read = card.get("read").and_then(|r| r.as_bool()).unwrap_or(true);
    let unread_marker = if read { " " } else { "*" };

    // headline.text -- primary notification text.
    let headline = card
        .get("headline")
        .and_then(|h| h.get("text").and_then(|t| t.as_str()))
        .unwrap_or("(no headline)");

    // subHeadline.text -- secondary detail.
    let sub_headline = card
        .get("subHeadline")
        .and_then(|s| s.get("text").and_then(|t| t.as_str()))
        .unwrap_or("");

    // kicker.text -- time indicator (e.g. "2h ago").
    let kicker = card
        .get("kicker")
        .and_then(|k| k.get("text").and_then(|t| t.as_str()))
        .unwrap_or("");

    // publishedAt -- epoch millis timestamp.
    let published_at = card
        .get("publishedAt")
        .and_then(|p| p.as_i64())
        .and_then(|millis| {
            let secs = millis / 1000;
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        })
        .unwrap_or_default();

    // contentType -- discriminator like PROFILE_VIEW, REACTION, etc.
    let content_type = card
        .get("contentType")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    // Truncate long headlines for summary view.
    let headline_display = truncate_with_ellipsis(headline, 120);

    print!("[{}]{} {}", index, unread_marker, headline_display);
    if !kicker.is_empty() {
        print!("  ({})", kicker);
    }
    println!();

    if !sub_headline.is_empty() {
        println!("    {}", sub_headline);
    }
    if !content_type.is_empty() {
        print!("    type: {}", content_type);
    }
    if !published_at.is_empty() {
        if !content_type.is_empty() {
            print!("  |  {}", published_at);
        } else {
            print!("    {}", published_at);
        }
    }
    if !content_type.is_empty() || !published_at.is_empty() {
        println!();
    }
}

/// Load the stored session or return a descriptive error.
fn load_session() -> Result<(Session, std::path::PathBuf), String> {
    let path = Session::default_path().map_err(|e| format!("{e}"))?;

    if !path.exists() {
        return Err(format!(
            "no session found at {} -- run `auth login` first",
            path.display()
        ));
    }

    let session = Session::load(&path).map_err(|e| format!("{e}"))?;

    if !session.is_valid() {
        return Err("session is invalid (empty li_at cookie)".to_string());
    }

    Ok((session, path))
}

/// Load the stored session and create an authenticated client.
fn load_session_client() -> Result<(LinkedInClient, std::path::PathBuf), String> {
    // Check for browser cookies file first (enables write operations).
    let cookies_path = std::path::Path::new("secrets/browser_cookies.json");
    if cookies_path.exists() {
        let data = std::fs::read_to_string(cookies_path)
            .map_err(|e| format!("failed to read browser cookies: {e}"))?;
        let cookies: std::collections::HashMap<String, String> = serde_json::from_str(&data)
            .map_err(|e| format!("failed to parse browser cookies: {e}"))?;
        let client = LinkedInClient::with_browser_cookies(&cookies)
            .map_err(|e| format!("client error: {e}"))?;
        let (_, path) = load_session()?;
        eprintln!("Using browser cookies from secrets/browser_cookies.json");
        return Ok((client, path));
    }

    let (session, path) = load_session()?;
    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;
    Ok((client, path))
}

/// Print a conversation from the GraphQL `messengerConversationsByCategory` response.
fn print_graphql_conversation(index: usize, conv: &serde_json::Value) {
    let backend_urn = conv
        .get("backendUrn")
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let conv_id = backend_urn
        .strip_prefix("urn:li:messagingThread:")
        .unwrap_or(backend_urn);

    let read = conv.get("read").and_then(|r| r.as_bool()).unwrap_or(true);
    let unread_marker = if read { " " } else { "*" };

    let unread_count = conv
        .get("unreadCount")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    // Extract participant names from conversationParticipants.
    let mut names = Vec::new();
    if let Some(participants) = conv
        .get("conversationParticipants")
        .and_then(|p| p.as_array())
    {
        for p in participants {
            let name = p
                .get("participantType")
                .and_then(|pt| pt.get("member"))
                .and_then(|member| {
                    let first = member
                        .get("firstName")
                        .and_then(|f| f.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let last = member
                        .get("lastName")
                        .and_then(|l| l.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if first.is_empty() && last.is_empty() {
                        None
                    } else {
                        Some(format!("{} {}", first, last).trim().to_string())
                    }
                });
            if let Some(n) = name {
                names.push(n);
            }
        }
    }

    // Last message from inline messages.elements.
    let last_message = conv
        .get("messages")
        .and_then(|m| m.get("elements"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|msg| msg.get("body"))
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

    let last_msg_display = truncate_with_ellipsis(&last_message, 80);

    let title = conv.get("title").and_then(|n| n.as_str()).unwrap_or("");
    let display_name = if !title.is_empty() {
        title.to_string()
    } else if !names.is_empty() {
        names.join(", ")
    } else {
        "(unknown)".to_string()
    };

    println!(
        "[{}]{} {} (id: {})",
        index, unread_marker, display_name, conv_id
    );
    if unread_count > 0 {
        println!("    unread: {}", unread_count);
    }
    if !last_msg_display.is_empty() {
        println!("    last: {}", last_msg_display);
    }
}

/// Print a message from the GraphQL `messengerMessagesByConversation` response.
fn print_graphql_message(msg: &serde_json::Value) {
    // Timestamp from deliveredAt.
    let delivered_at = msg.get("deliveredAt").and_then(|c| c.as_u64()).unwrap_or(0);
    let time_str = if delivered_at > 0 {
        let secs = (delivered_at / 1000) as i64;
        let nanos = ((delivered_at % 1000) * 1_000_000) as u32;
        chrono::DateTime::from_timestamp(secs, nanos)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| delivered_at.to_string())
    } else {
        String::new()
    };

    // Sender URN (we can't resolve name without a separate lookup).
    let sender_urn = msg
        .get("sender")
        .and_then(|s| s.get("hostIdentityUrn"))
        .and_then(|u| u.as_str())
        .unwrap_or("unknown");
    // Try to shorten the URN for display.
    let sender_display = sender_urn
        .strip_prefix("urn:li:fsd_profile:")
        .unwrap_or(sender_urn);

    // Message body.
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

    let subject = msg.get("subject").and_then(|s| s.as_str()).unwrap_or("");

    if !time_str.is_empty() {
        print!("[{}] ", time_str);
    }
    println!("{}", sender_display);
    if !subject.is_empty() {
        println!("  Subject: {}", subject);
    }
    if !body.is_empty() {
        println!("  {}", body);
    }
}

/// Truncate a string to at most `max_chars` characters, safely handling
/// multi-byte UTF-8. Returns the original string if it is shorter than
/// `max_chars`.
fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Truncate a string and append `...` if it was truncated.
/// Returns the original string unchanged if it fits within `max_chars`.
fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let truncated = truncate(s, max_chars);
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

/// Print a paging header line in the format:
/// `{label} (offset {start}, showing {count}, total {total})`
fn print_paging_header(label: &str, paging: &Paging) {
    let total_str = paging
        .total
        .map(|t| t.to_string())
        .unwrap_or_else(|| "?".to_string());
    println!(
        "{} (offset {}, showing {}, total {})",
        label, paging.start, paging.count, total_str
    );
}

// NOTE: The old REST-based print_conversation, extract_participant_names,
// extract_message_body, and print_messaging_event functions have been removed.
// They relied on the now-defunct messaging/conversations REST endpoint.
// The GraphQL equivalents are print_graphql_conversation and print_graphql_message.
