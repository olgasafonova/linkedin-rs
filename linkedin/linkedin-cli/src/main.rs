use std::fs;
use std::process;

use clap::{Parser, Subcommand};
use linkedin_api::auth::Session;
use linkedin_api::client::LinkedInClient;
use linkedin_api::models::{
    ConnectionsResponse, ConversationsResponse, FeedResponse, NotificationCardsResponse,
    SearchResponse,
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

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Read messages in a conversation
    Read {
        /// Conversation ID (thread ID portion of URN, e.g. 2-abc123)
        conversation_id: String,

        /// Number of messages to fetch (default: 20)
        #[arg(long, default_value = "20")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
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
            MessagesAction::List { count, start, json } => {
                if let Err(e) = cmd_messages_list(start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            MessagesAction::Read {
                conversation_id,
                count,
                start,
                json,
            } => {
                if let Err(e) = cmd_messages_read(&conversation_id, start, count, json).await {
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
    println!("JSESSIONID: {}...", &session.jsessionid[..10]);
    println!(
        "li_at: {}...",
        &session.li_at[..session.li_at.len().min(10)]
    );
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
    println!(
        "JSESSIONID: {}...",
        &session.jsessionid[..session.jsessionid.len().min(10)]
    );
    println!(
        "li_at: {}...",
        &session.li_at[..session.li_at.len().min(10)]
    );

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

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

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
    let (session, _path) = load_session()?;

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

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

/// Print a human-readable summary of an identity/profiles response.
///
/// Extracts name, headline, location, current position, and education.
/// The response may be a full `Profile` object or may have nested
/// structures depending on the decoration recipe. We handle both
/// gracefully by probing known field paths.
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

    // Headline.
    if let Some(headline) = profile.get("headline").and_then(|v| v.as_str()) {
        println!("Headline: {}", headline);
    }

    // Location.
    if let Some(loc) = profile.get("locationName").and_then(|v| v.as_str()) {
        println!("Location: {}", loc);
    } else if let Some(geo) = profile.get("geoLocationName").and_then(|v| v.as_str()) {
        println!("Location: {}", geo);
    }

    // Industry.
    if let Some(industry) = profile.get("industryName").and_then(|v| v.as_str()) {
        println!("Industry: {}", industry);
    }

    // Summary / About.
    if let Some(summary) = profile.get("summary").and_then(|v| v.as_str()) {
        let display = if summary.len() > 200 {
            format!("{}...", &summary[..200])
        } else {
            summary.to_string()
        };
        println!("About: {}", display);
    }

    // Entity URN.
    if let Some(urn) = profile.get("entityUrn").and_then(|v| v.as_str()) {
        println!("URN: {}", urn);
    }

    // MiniProfile fields (if embedded).
    if let Some(mini) = profile.get("miniProfile") {
        if let Some(pub_id) = mini.get("publicIdentifier").and_then(|v| v.as_str()) {
            println!("Public ID: {}", pub_id);
        }
    }

    // Current position -- look for positions in various shapes.
    // The response may include `positions` as an inline collection
    // (with `elements`) or as a direct array.
    let positions = profile.get("positions").and_then(|p| {
        p.get("elements")
            .and_then(|e| e.as_array())
            .or_else(|| p.as_array())
    });
    if let Some(pos_list) = positions {
        if !pos_list.is_empty() {
            println!("\nPositions:");
            for pos in pos_list {
                let title = pos.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let company = pos
                    .get("companyName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let period = format_time_period(pos.get("timePeriod"));
                if !title.is_empty() || !company.is_empty() {
                    println!("  - {} at {}{}", title, company, period);
                }
            }
        }
    }

    // Education.
    let educations = profile.get("educations").and_then(|e| {
        e.get("elements")
            .and_then(|el| el.as_array())
            .or_else(|| e.as_array())
    });
    if let Some(edu_list) = educations {
        if !edu_list.is_empty() {
            println!("\nEducation:");
            for edu in edu_list {
                let school = edu.get("schoolName").and_then(|v| v.as_str()).unwrap_or("");
                let degree = edu.get("degreeName").and_then(|v| v.as_str()).unwrap_or("");
                let field = edu
                    .get("fieldOfStudy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let period = format_time_period(edu.get("timePeriod"));
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

    // Print top-level keys for discoverability.
    if let Some(obj) = profile.as_object() {
        let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        if !keys.is_empty() {
            println!("\nResponse keys: {}", keys.join(", "));
        }
    }
}

/// Format a `timePeriod` object into a human-readable string like " (2020 - 2023)".
///
/// The `timePeriod` has shape `{ "startDate": { "year": N, "month": N }, "endDate": ... }`.
/// Returns an empty string if the input is `None` or lacks date fields.
fn format_time_period(time_period: Option<&serde_json::Value>) -> String {
    let tp = match time_period {
        Some(v) => v,
        None => return String::new(),
    };

    let start_year = tp
        .get("startDate")
        .and_then(|d| d.get("year"))
        .and_then(|y| y.as_u64());
    let end_year = tp
        .get("endDate")
        .and_then(|d| d.get("year"))
        .and_then(|y| y.as_u64());

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

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

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
        let total_str = paging
            .total
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "Feed updates (offset {}, showing {}, total {})",
            paging.start, paging.count, total_str
        );
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
    // Try to extract actor name.
    let actor_name = item
        .get("actor")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(unknown author)");

    // Try to extract commentary text.
    let commentary = item
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Truncate long commentary for the summary view.
    let commentary_display = if commentary.len() > 120 {
        format!("{}...", &commentary[..120])
    } else {
        commentary.to_string()
    };

    // Entity URN for reference.
    let urn = item.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");

    // Social counts if available.
    let likes = item
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numLikes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let comments = item
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
async fn cmd_messages_list(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (session, _path) = load_session()?;

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

    let value = client
        .get_conversations(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let resp: ConversationsResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse conversations response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        let total_str = paging
            .total
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "Conversations (offset {}, showing {}, total {})",
            paging.start, paging.count, total_str
        );
    }
    println!("---");

    if resp.elements.is_empty() {
        println!("(no conversations)");
        return Ok(());
    }

    for (i, element) in resp.elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_conversation(idx, element);
        println!();
    }

    Ok(())
}

/// Handle `messages read <conversation_id> [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/messaging/conversations/{id}/events
/// with pagination params, and prints the messages.
async fn cmd_messages_read(
    conversation_id: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (session, _path) = load_session()?;

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

    let value = client
        .get_conversation_events(conversation_id, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    if let Some(paging) = value.get("paging") {
        let pg_start = paging.get("start").and_then(|v| v.as_u64()).unwrap_or(0);
        let pg_count = paging.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        let total_str = paging
            .get("total")
            .and_then(|v| v.as_u64())
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "Messages in {} (offset {}, showing {}, total {})",
            conversation_id, pg_start, pg_count, total_str
        );
    }
    println!("---");

    if elements.is_empty() {
        println!("(no messages)");
        return Ok(());
    }

    for event in &elements {
        print_messaging_event(event);
        println!();
    }

    Ok(())
}

/// Handle `connections list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/relationships/connections with
/// pagination params sorted by RECENTLY_ADDED, and prints the results.
async fn cmd_connections_list(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (session, _path) = load_session()?;

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

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
        let total_str = paging
            .total
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "Connections (offset {}, showing {}, total {})",
            paging.start, paging.count, total_str
        );
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
/// Loads the session, calls GET /voyager/api/search/hits with guided people
/// search query, and prints the results.
async fn cmd_search_people(
    keywords: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (session, _path) = load_session()?;

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

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

    let resp: SearchResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse search response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        let total_str = paging
            .total
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "Search results for '{}' (offset {}, showing {}, total {})",
            keywords, paging.start, paging.count, total_str
        );
    }
    println!("---");

    if resp.elements.is_empty() {
        println!("(no results)");
        return Ok(());
    }

    for (i, element) in resp.elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_search_hit(idx, element);
        println!();
    }

    Ok(())
}

/// Print a brief human-readable summary of a single search hit.
///
/// Search hits have a polymorphic `hitInfo` field (Rest.li union). For people
/// results, we look for the `SearchProfile` variant which contains a
/// `miniProfile` with name and headline, plus `location` and `industry` fields.
///
/// The exact union key varies -- it may be a fully qualified class name like
/// `com.linkedin.voyager.search.SearchProfile` or a short form. We try both
/// patterns and fall back to extracting fields from the element root.
fn print_search_hit(index: usize, hit: &serde_json::Value) {
    // Try to find the SearchProfile inside hitInfo (union type).
    let profile = hit
        .get("hitInfo")
        .and_then(|hi| {
            // Try fully-qualified union key first.
            hi.get("com.linkedin.voyager.search.SearchProfile")
                .or_else(|| hi.get("searchProfile"))
                // If hitInfo is directly the profile data (flat structure).
                .or(Some(hi))
        })
        .unwrap_or(hit);

    // Extract name from miniProfile within the search profile.
    let mini = profile.get("miniProfile");

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

    // Headline: miniProfile.occupation or profile-level headline.
    let headline = mini
        .and_then(|m| m.get("occupation").and_then(|v| v.as_str()))
        .or_else(|| profile.get("headline").and_then(|v| v.as_str()))
        .unwrap_or("");

    // Location: profile-level location field.
    let location = profile
        .get("location")
        .and_then(|v| v.as_str())
        .or_else(|| {
            profile
                .get("subline")
                .and_then(|s| s.get("text"))
                .and_then(|t| t.as_str())
        })
        .unwrap_or("");

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
    if !location.is_empty() {
        println!("    location: {}", location);
    }
}

/// Handle `notifications list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/identity/notificationCards with
/// pagination params, and prints the results.
async fn cmd_notifications_list(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (session, _path) = load_session()?;

    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;

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
        let total_str = paging
            .total
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "Notifications (offset {}, showing {}, total {})",
            paging.start, paging.count, total_str
        );
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
    let headline_display = if headline.len() > 120 {
        format!("{}...", &headline[..120])
    } else {
        headline.to_string()
    };

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

/// Print a brief human-readable summary of a single conversation.
fn print_conversation(index: usize, conv: &serde_json::Value) {
    let urn = conv.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");

    // Extract conversation ID from URN for easy use with `messages read`.
    let conv_id = urn.strip_prefix("urn:li:messagingThread:").unwrap_or(urn);

    let read = conv.get("read").and_then(|r| r.as_bool()).unwrap_or(true);
    let unread_marker = if read { " " } else { "*" };

    let unread_count = conv
        .get("unreadCount")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    // Try to extract participant names.
    let participants = extract_participant_names(conv);

    // Try to get the last message preview from inline events.
    let last_message = conv
        .get("events")
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(extract_message_body)
        .unwrap_or_default();

    let last_msg_display = if last_message.len() > 80 {
        format!("{}...", &last_message[..80])
    } else {
        last_message
    };

    // Group chat name.
    let name = conv.get("name").and_then(|n| n.as_str()).unwrap_or("");

    let display_name = if !name.is_empty() {
        name.to_string()
    } else if !participants.is_empty() {
        participants.join(", ")
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

/// Extract participant names from a conversation's participants array.
fn extract_participant_names(conv: &serde_json::Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(participants) = conv.get("participants").and_then(|p| p.as_array()) {
        for participant in participants {
            // Participants are a union type; try MessagingMember path.
            let name = participant
                .get("com.linkedin.voyager.messaging.MessagingMember")
                .or_else(|| participant.get("messagingMember"))
                .and_then(|member| member.get("miniProfile"))
                .and_then(|profile| {
                    let first = profile
                        .get("firstName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let last = profile
                        .get("lastName")
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
    names
}

/// Extract message body text from an event's eventContent.
fn extract_message_body(event: &serde_json::Value) -> Option<String> {
    let content = event.get("eventContent")?;
    // eventContent is a union; try MessageEvent variant.
    let msg = content
        .get("com.linkedin.voyager.messaging.event.MessageEvent")
        .or_else(|| content.get("messageEvent"))?;

    // Prefer attributedBody.text, fall back to body.
    let body = msg
        .get("attributedBody")
        .and_then(|ab| ab.get("text"))
        .and_then(|t| t.as_str())
        .or_else(|| msg.get("body").and_then(|b| b.as_str()))?;

    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

/// Print a single messaging event in human-readable format.
fn print_messaging_event(event: &serde_json::Value) {
    let subtype = event
        .get("subtype")
        .and_then(|s| s.as_str())
        .unwrap_or("UNKNOWN");

    // Timestamp.
    let created_at = event.get("createdAt").and_then(|c| c.as_u64()).unwrap_or(0);
    let time_str = if created_at > 0 {
        // Convert epoch millis to a readable format.
        let secs = (created_at / 1000) as i64;
        let nanos = ((created_at % 1000) * 1_000_000) as u32;
        chrono::DateTime::from_timestamp(secs, nanos)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| created_at.to_string())
    } else {
        String::new()
    };

    // Sender name.
    let sender = event
        .get("from")
        .and_then(|from| {
            from.get("com.linkedin.voyager.messaging.MessagingMember")
                .or_else(|| from.get("messagingMember"))
        })
        .and_then(|member| member.get("miniProfile"))
        .and_then(|profile| {
            let first = profile
                .get("firstName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = profile
                .get("lastName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if first.is_empty() && last.is_empty() {
                None
            } else {
                Some(format!("{} {}", first, last).trim().to_string())
            }
        })
        .unwrap_or_else(|| "(unknown)".to_string());

    // Message body.
    let body = extract_message_body(event).unwrap_or_else(|| format!("[{} event]", subtype));

    if !time_str.is_empty() {
        println!("[{}] {}", time_str, sender);
    } else {
        println!("{}", sender);
    }
    println!("  {}", body);
}
