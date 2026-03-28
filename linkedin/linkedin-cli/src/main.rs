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
    /// Company / organization info
    Company {
        #[command(subcommand)]
        action: CompanyAction,
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

        /// Filter by author name (case-insensitive substring match)
        #[arg(long)]
        author: Option<String>,

        /// Filter by keyword in post text (case-insensitive substring match)
        #[arg(long)]
        keyword: Option<String>,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Show comments on a post by index from last `feed list`
    Comments {
        /// 1-based index from the most recent `feed list` output
        index: usize,

        /// Number of comments to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Show full post details for item N from the last `feed list`
    Read {
        /// 1-based index from the most recent `feed list` output
        index: usize,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// React to a post (like, celebrate, etc.)
    React {
        /// Post/activity URN or 1-based index from last `feed list`
        post_urn: String,

        /// Reaction type: LIKE, PRAISE, EMPATHY, INTEREST, APPRECIATION, ENTERTAINMENT, CELEBRATION
        #[arg(long = "type", default_value = "LIKE")]
        reaction_type: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Remove a reaction from a post
    Unreact {
        /// Post/activity URN or 1-based index from last `feed list`
        post_urn: String,

        /// Reaction type to remove: LIKE, PRAISE, EMPATHY, INTEREST, APPRECIATION, ENTERTAINMENT, CELEBRATION
        #[arg(long = "type", default_value = "LIKE")]
        reaction_type: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Comment on a feed post
    ///
    /// WARNING: This creates a REAL COMMENT on a LinkedIn post.
    /// Use --yes to skip the confirmation prompt.
    Comment {
        /// Post/activity URN or 1-based index from last `feed list`
        post_urn: String,

        /// The comment text
        text: String,

        /// Skip confirmation prompt (required for non-interactive use)
        #[arg(long)]
        yes: bool,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Show analytics for your recent posts (views, impressions)
    Stats {
        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Create a new text post on your LinkedIn feed
    ///
    /// WARNING: This creates a REAL PUBLIC post on your LinkedIn account.
    /// Use --yes to skip the confirmation prompt.
    Post {
        /// The text content of the post
        text: String,

        /// Post visibility: ANYONE (public) or CONNECTIONS_ONLY
        #[arg(long, default_value = "ANYONE")]
        visibility: String,

        /// Skip confirmation prompt (required for non-interactive use)
        #[arg(long)]
        yes: bool,

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
    /// Visit a profile (registers you in "who viewed my profile")
    Visit {
        /// LinkedIn public identifier (vanity URL slug, e.g. john-doe-123)
        public_id: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Show who viewed your profile
    Viewers {
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

        /// Filter by name or headline (case-insensitive substring match)
        #[arg(long)]
        keyword: Option<String>,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Send a connection request (invitation) to another member
    Invite {
        /// LinkedIn public identifier (vanity URL slug) or fsd_profile URN
        public_id_or_urn: String,

        /// Optional custom message to include with the invitation
        #[arg(long)]
        message: Option<String>,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// List pending (received) connection invitations
    Invitations {
        /// Number of invitations to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Accept a pending connection invitation
    Accept {
        /// Invitation ID (numeric portion of the invitation URN, e.g. 7312345678901234567)
        invitation_id: String,

        /// Shared secret from the invitation (required for CSRF protection).
        /// Obtain from `connections invitations --json`.
        #[arg(long)]
        secret: String,

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
    /// Search for jobs by keywords
    Jobs {
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
    /// Search for posts/content by keywords
    Posts {
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
enum CompanyAction {
    /// View company info by universal name (URL slug)
    View {
        /// Company URL slug (e.g. "microsoft" from linkedin.com/company/microsoft)
        slug: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// List company page followers (requires page admin access)
    Followers {
        /// Company URL slug (e.g. "getskillcheck")
        slug: String,

        /// Number of followers to fetch (default: 50)
        #[arg(long, default_value = "50")]
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
    /// Send a message to a connection (new conversation)
    Send {
        /// LinkedIn public identifier (vanity URL slug, e.g. john-doe-123)
        recipient: String,

        /// Message text to send
        message: String,

        /// Output raw JSON response instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Reply to an existing conversation thread
    ///
    /// WARNING: This sends a REAL MESSAGE in a LinkedIn conversation.
    /// Use --yes to skip the confirmation prompt.
    Reply {
        /// Conversation ID (from `messages list`, e.g. 2-abc123)
        conversation_id: String,

        /// The reply text
        message: String,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,

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
            ProfileAction::Visit { public_id, json } => {
                if let Err(e) = cmd_profile_visit(&public_id, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            ProfileAction::Viewers { json } => {
                if let Err(e) = cmd_profile_viewers(json).await {
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
            MessagesAction::Reply {
                conversation_id,
                message,
                yes,
                json,
            } => {
                if let Err(e) = cmd_messages_reply(&conversation_id, &message, yes, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Feed { action } => match action {
            FeedAction::List {
                count,
                start,
                author,
                keyword,
                json,
            } => {
                if let Err(e) =
                    cmd_feed_list(start, count, author.as_deref(), keyword.as_deref(), json).await
                {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            FeedAction::Comments { index, count, json } => {
                if let Err(e) = cmd_feed_comments(index, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            FeedAction::Read { index, json } => {
                if let Err(e) = cmd_feed_read(index, json) {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            FeedAction::React {
                post_urn,
                reaction_type,
                json,
            } => {
                if let Err(e) = cmd_feed_react(&post_urn, &reaction_type, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            FeedAction::Unreact {
                post_urn,
                reaction_type,
                json,
            } => {
                if let Err(e) = cmd_feed_unreact(&post_urn, &reaction_type, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            FeedAction::Comment {
                post_urn,
                text,
                yes,
                json,
            } => {
                if let Err(e) = cmd_feed_comment(&post_urn, &text, yes, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            FeedAction::Stats { json } => {
                if let Err(e) = cmd_feed_stats(json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            FeedAction::Post {
                text,
                visibility,
                yes,
                json,
            } => {
                if let Err(e) = cmd_feed_post(&text, &visibility, yes, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Connections { action } => match action {
            ConnectionsAction::List {
                count,
                start,
                keyword,
                json,
            } => {
                if let Err(e) = cmd_connections_list(start, count, keyword.as_deref(), json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            ConnectionsAction::Invite {
                public_id_or_urn,
                message,
                json,
            } => {
                if let Err(e) =
                    cmd_connections_invite(&public_id_or_urn, message.as_deref(), json).await
                {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            ConnectionsAction::Invitations { count, start, json } => {
                if let Err(e) = cmd_connections_invitations(start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            ConnectionsAction::Accept {
                invitation_id,
                secret,
                json,
            } => {
                if let Err(e) = cmd_connections_accept(&invitation_id, &secret, json).await {
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
            SearchAction::Jobs {
                keywords,
                count,
                start,
                json,
            } => {
                if let Err(e) = cmd_search_jobs(&keywords, start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            SearchAction::Posts {
                keywords,
                count,
                start,
                json,
            } => {
                if let Err(e) = cmd_search_posts(&keywords, start, count, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Company { action } => match action {
            CompanyAction::View { slug, json } => {
                if let Err(e) = cmd_company_view(&slug, json).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            CompanyAction::Followers {
                slug,
                count,
                start,
                json,
            } => {
                if let Err(e) = cmd_company_followers(&slug, start, count, json).await {
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

/// Handle `profile visit <public_id> [--json]`.
///
/// Visits a profile so the target sees you in "who viewed my profile".
/// Uses the web client's GraphQL query ID which registers the view as a
/// side effect. See `re/profile_visit.md` for the mechanism.
async fn cmd_profile_visit(public_id: &str, raw_json: bool) -> Result<(), String> {
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
async fn cmd_profile_viewers(raw_json: bool) -> Result<(), String> {
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
        .or_else(|| {
            profile
                .get("connectionsCount")
                .and_then(|v| v.as_u64())
        });
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

/// Handle `feed list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/feed/updates?q=findFeed with
/// pagination params, and prints the results.
async fn cmd_feed_list(
    start: u32,
    count: u32,
    author_filter: Option<&str>,
    keyword_filter: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_feed(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    // Cache the raw feed response for `feed read` / `feed react` / `feed comment` by index.
    if let Err(e) = save_feed_cache(&value) {
        eprintln!("warning: failed to cache feed: {e}");
    }

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
    let filtering = author_filter.is_some() || keyword_filter.is_some();
    if let Some(ref paging) = feed.paging {
        print_paging_header("Feed updates", paging);
    }
    if let Some(author) = author_filter {
        println!("  filter: author contains \"{}\"", author);
    }
    if let Some(keyword) = keyword_filter {
        println!("  filter: text contains \"{}\"", keyword);
    }
    println!("---");

    if feed.elements.is_empty() {
        println!("(no feed items)");
        return Ok(());
    }

    let author_lower = author_filter.map(|s| s.to_lowercase());
    let keyword_lower = keyword_filter.map(|s| s.to_lowercase());

    let mut shown = 0;
    for (i, element) in feed.elements.iter().enumerate() {
        let idx = start as usize + i + 1;

        // Apply filters on the cached data.
        if filtering {
            let update = element
                .get("value")
                .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
                .unwrap_or(element);

            if let Some(ref author_q) = author_lower {
                let actor = update
                    .get("actor")
                    .and_then(|a| a.get("name"))
                    .and_then(|n| n.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if !actor.to_lowercase().contains(author_q) {
                    continue;
                }
            }

            if let Some(ref kw_q) = keyword_lower {
                let commentary = update
                    .get("commentary")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if !commentary.to_lowercase().contains(kw_q) {
                    continue;
                }
            }
        }

        shown += 1;
        print_feed_item(idx, element);
        println!();
    }

    if shown == 0 {
        println!("(no matching feed items)");
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

    let media_label = extract_media_type_label(update);

    println!(
        "[{}] {} {}",
        index,
        actor_name,
        if !urn.is_empty() { urn } else { "" }
    );
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    if !media_label.is_empty() {
        print!("    [{}]  ", media_label);
    } else {
        print!("    ");
    }
    println!("likes: {}  comments: {}", likes, comments);
}

/// Handle `feed react <post_urn> [--type LIKE] [--json]`.
///
/// Reacts to a feed post with the specified reaction type.
/// Reaction type validation is handled by the API layer.
async fn cmd_feed_react(post_urn: &str, reaction_type: &str, raw_json: bool) -> Result<(), String> {
    let resolved_urn = resolve_post_urn(post_urn)?;
    let rt_upper = reaction_type.to_uppercase();
    let (client, _path) = load_session_client()?;

    eprintln!("Reacting to {} with {}...", resolved_urn, rt_upper);
    let result = client
        .react_to_post(&resolved_urn, &rt_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Reacted with {} to {}", rt_upper, resolved_urn);
    }

    Ok(())
}

/// Handle `feed unreact <post_urn> [--type LIKE] [--json]`.
///
/// Removes a reaction from a feed post.
async fn cmd_feed_unreact(
    post_urn: &str,
    reaction_type: &str,
    raw_json: bool,
) -> Result<(), String> {
    let resolved_urn = resolve_post_urn(post_urn)?;
    let rt_upper = reaction_type.to_uppercase();
    let (client, _path) = load_session_client()?;

    eprintln!("Removing {} reaction from {}...", resolved_urn, rt_upper);
    let result = client
        .unreact_from_post(&resolved_urn, &rt_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Removed {} reaction from {}", rt_upper, resolved_urn);
    }

    Ok(())
}

/// Handle `feed comment <post_urn> <text> [--yes] [--json]`.
///
/// Creates a comment on a feed post. Requires `--yes` to confirm,
/// since this creates a REAL COMMENT on a LinkedIn post.
async fn cmd_feed_comment(
    post_urn: &str,
    text: &str,
    confirmed: bool,
    raw_json: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err("this will create a REAL COMMENT on a LinkedIn post. \
             Pass --yes to confirm."
            .to_string());
    }

    let resolved_urn = resolve_post_urn(post_urn)?;
    let (client, _path) = load_session_client()?;

    eprintln!("Commenting on {}...", resolved_urn);
    let result = client
        .comment_on_post(&resolved_urn, text)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Commented on {}", resolved_urn);
    }

    Ok(())
}

/// Handle `feed stats [--json]`.
///
/// Fetches analytics for the user's recent posts (views, impressions).
/// Uses the `identity/socialUpdateAnalytics` endpoint. This may require
/// Premium or Creator Mode for full data.
async fn cmd_feed_stats(raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // Try the analytics endpoint. It may return 403/404 without Premium.
    let header_result = client.get_post_analytics_header().await;
    let analytics_result = client.get_post_analytics().await;

    if raw_json {
        let combined = serde_json::json!({
            "header": header_result.as_ref().ok(),
            "analytics": analytics_result.as_ref().ok(),
        });
        let pretty = serde_json::to_string_pretty(&combined)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // Print header summary if available.
    match header_result {
        Ok(header) => {
            println!("Post Analytics Summary");
            println!("---");
            // Try to extract summary metrics from the header response.
            if let Some(text) = header
                .get("headline")
                .and_then(|h| h.get("text"))
                .and_then(|t| t.as_str())
            {
                println!("{}", text);
            }
            if let Some(elements) = header.get("elements").and_then(|e| e.as_array()) {
                for el in elements {
                    let title = el
                        .get("title")
                        .and_then(|t| t.get("text").and_then(|v| v.as_str()))
                        .or_else(|| el.get("title").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let value = el
                        .get("value")
                        .and_then(|v| v.get("text").and_then(|t| t.as_str()))
                        .or_else(|| el.get("value").and_then(|v| v.as_str()))
                        .or_else(|| {
                            el.get("numericValue")
                                .and_then(|n| n.as_u64())
                                .map(|_| "")
                        })
                        .unwrap_or("");
                    let numeric = el
                        .get("numericValue")
                        .and_then(|v| v.as_u64());
                    if !title.is_empty() {
                        if let Some(n) = numeric {
                            println!("  {}: {}", title, n);
                        } else if !value.is_empty() {
                            println!("  {}: {}", title, value);
                        } else {
                            println!("  {}", title);
                        }
                    }
                }
            }
            println!();
        }
        Err(e) => {
            eprintln!(
                "Analytics header unavailable (may require Premium/Creator): {}",
                e
            );
        }
    }

    // Print detailed analytics if available.
    match analytics_result {
        Ok(analytics) => {
            if let Some(elements) = analytics.get("elements").and_then(|e| e.as_array()) {
                println!("Recent Post Analytics ({} posts)", elements.len());
                println!("---");
                for (i, el) in elements.iter().enumerate() {
                    let views = el
                        .get("totalShareStatistics")
                        .and_then(|s| s.get("uniqueImpressionsCount"))
                        .and_then(|v| v.as_u64())
                        .or_else(|| {
                            el.get("impressionCount").and_then(|v| v.as_u64())
                        })
                        .unwrap_or(0);
                    let likes = el
                        .get("totalShareStatistics")
                        .and_then(|s| s.get("likeCount"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let comments = el
                        .get("totalShareStatistics")
                        .and_then(|s| s.get("commentCount"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let shares = el
                        .get("totalShareStatistics")
                        .and_then(|s| s.get("shareCount"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    let text_preview = el
                        .get("commentary")
                        .and_then(|c| c.get("text"))
                        .and_then(|t| t.get("text").and_then(|v| v.as_str()))
                        .or_else(|| el.get("text").and_then(|v| v.as_str()))
                        .unwrap_or("(post)");

                    println!(
                        "[{}] {} views, {} likes, {} comments, {} shares",
                        i + 1,
                        views,
                        likes,
                        comments,
                        shares
                    );
                    println!("    {}", truncate_with_ellipsis(text_preview, 100));
                    println!();
                }
            } else {
                println!("(no analytics data in response)");
            }
        }
        Err(e) => {
            eprintln!("Post analytics unavailable (may require Premium/Creator): {}", e);
        }
    }

    Ok(())
}

/// Handle `feed post <text> [--visibility ANYONE] [--yes] [--json]`.
///
/// Creates a new text-only post on the authenticated user's LinkedIn feed.
/// Requires `--yes` to confirm, since this creates a REAL PUBLIC post.
async fn cmd_feed_post(
    text: &str,
    visibility: &str,
    confirmed: bool,
    raw_json: bool,
) -> Result<(), String> {
    let vis_upper = visibility.to_uppercase();
    if vis_upper != "ANYONE" && vis_upper != "CONNECTIONS_ONLY" {
        return Err(format!(
            "invalid visibility '{}'. Must be ANYONE or CONNECTIONS_ONLY",
            visibility
        ));
    }

    if !confirmed {
        // Show what would be posted and require confirmation.
        eprintln!("WARNING: This will create a REAL post on your LinkedIn account!");
        eprintln!();
        eprintln!("  Visibility: {}", vis_upper);
        eprintln!("  Text: {}", truncate_with_ellipsis(text, 200));
        eprintln!();
        eprintln!("Use --yes to confirm and publish this post.");
        return Err("post not confirmed (use --yes to publish)".to_string());
    }

    let (client, _path) = load_session_client()?;

    eprintln!("Creating post (visibility: {})...", vis_upper);
    let result = client
        .create_post(text, &vis_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        // Try to extract the share URN from the response.
        let urn = result
            .get("data")
            .and_then(|d| d.get("createContentcreationDashShares"))
            .and_then(|c| c.get("entityUrn"))
            .and_then(|v| v.as_str())
            .or_else(|| result.get("entityUrn").and_then(|v| v.as_str()))
            .unwrap_or("(unknown)");
        println!("Post created successfully!");
        println!("  URN: {}", urn);
        println!("  Visibility: {}", vis_upper);
        println!("  Text: {}", truncate_with_ellipsis(text, 100));
    }

    Ok(())
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

/// Handle `messages reply <conversation_id> <message> [--yes] [--json]`.
///
/// Sends a reply to an existing conversation thread.
async fn cmd_messages_reply(
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
                        .and_then(|f| f.get("text").and_then(|v| v.as_str()).or_else(|| f.as_str()))
                        .unwrap_or("");
                    let last = m
                        .get("lastName")
                        .and_then(|l| l.get("text").and_then(|v| v.as_str()).or_else(|| l.as_str()))
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
                        b.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
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

/// Handle `connections list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/relationships/connections with
/// pagination params sorted by RECENTLY_ADDED, and prints the results.
async fn cmd_connections_list(
    start: u32,
    count: u32,
    keyword_filter: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
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

/// Handle `connections invite <public_id_or_urn> [--message "text"] [--json]`.
///
/// Resolves the target to a profile URN if a public identifier is given,
/// then sends a connection request via the normInvitations endpoint.
async fn cmd_connections_invite(
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

/// Handle `connections invitations [--count N] [--start N] [--json]`.
///
/// Lists pending (received) connection invitations using the Dash GraphQL
/// `voyagerRelationshipsDashInvitationViews` endpoint.
async fn cmd_connections_invitations(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
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
async fn cmd_connections_accept(
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

/// Handle `company view <slug> [--json]`.
///
/// Fetches company info by universal name (URL slug) and prints a summary.
async fn cmd_company_view(slug: &str, raw_json: bool) -> Result<(), String> {
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
async fn cmd_company_followers(
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

    let company_name = company
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(slug);

    // Extract numeric company ID from entityUrn.
    let company_id = company
        .get("entityUrn")
        .and_then(|v| v.as_str())
        .and_then(|urn| urn.rsplit(':').next())
        .or_else(|| company.get("companyId").and_then(|v| v.as_u64()).map(|_| ""))
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
            return Ok(());
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
                    println!(
                        "{} of your connections follow this page:",
                        urns.len()
                    );
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
        .or_else(|| {
            company
                .get("localizedName")
                .and_then(|v| v.as_str())
        })
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
    if let Some(followers) = company.get("followingInfo").and_then(|f| {
        f.get("followerCount").and_then(|v| v.as_u64())
    }) {
        println!("Followers: {}", followers);
    }

    // Headquarters
    if let Some(hq) = company.get("headquarter").or_else(|| company.get("confirmedLocations").and_then(|v| v.as_array()).and_then(|arr| arr.first())) {
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

/// Handle `search jobs <keywords> [--count N] [--start N] [--json]`.
///
/// Loads the session, calls the Voyager GraphQL `searchDashClustersByAll`
/// endpoint with `resultType:List(JOBS)` and prints the results.
async fn cmd_search_jobs(
    keywords: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .search_jobs(keywords, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // The jobsDashJobCardsByJobSearch response has the standard collection
    // shape: elements (array of job cards), paging, metadata.
    let resp: SearchResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse search response: {e}"))?;

    if let Some(ref paging) = resp.paging {
        print_paging_header(&format!("Job search results for '{}'", keywords), paging);
    }
    println!("---");

    let mut result_idx = start as usize;
    let mut any_results = false;
    for element in &resp.elements {
        // Each element has jobCard.jobPostingCard with the display fields.
        let card = element
            .get("jobCard")
            .and_then(|jc| jc.get("jobPostingCard"));
        if let Some(card) = card {
            result_idx += 1;
            any_results = true;
            print_job_card(result_idx, card);
            println!();
        }
    }

    if !any_results {
        println!("(no results)");
    }

    Ok(())
}

/// Print a brief human-readable summary of a single job search card.
///
/// The GraphQL `jobsDashJobCardsByJobSearch` response returns elements
/// with `jobCard.jobPostingCard` objects containing:
/// - `jobPosting.title`: job title
/// - `primaryDescription.text`: company name
/// - `secondaryDescription.text`: location
/// - `cardActionV2.navigationAction.actionTarget`: job URL
/// - `footerItems[].timeAt`: posted date (epoch millis)
fn print_job_card(index: usize, card: &serde_json::Value) {
    let title = card
        .get("jobPosting")
        .and_then(|jp| jp.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");

    let company = card
        .get("primaryDescription")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let location = card
        .get("secondaryDescription")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Extract job URL from the card action.
    let job_url = card
        .get("cardActionV2")
        .and_then(|a| a.get("navigationAction"))
        .and_then(|na| na.get("actionTarget"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    print!("[{}] {}", index, title);
    println!();

    if !company.is_empty() {
        println!("    company: {}", company);
    }
    if !location.is_empty() {
        println!("    location: {}", location);
    }
    if !job_url.is_empty() {
        // Show just the job view path, not the full URL with tracking params
        if let Some(path) = job_url.strip_prefix("https://www.linkedin.com") {
            let clean = path.split('?').next().unwrap_or(path);
            println!("    url: {}", clean);
        }
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
///
/// Supports threading: if a message is a reply, shows the parent message
/// context indented above the reply.
fn print_graphql_message(msg: &serde_json::Value) {
    // Check for reply/thread context first.
    let reply_to = msg
        .get("replyMessage")
        .or_else(|| msg.get("parentMessage"))
        .or_else(|| msg.get("quotedMessage"));

    if let Some(parent) = reply_to {
        let parent_sender = parent
            .get("sender")
            .and_then(|s| s.get("hostIdentityUrn"))
            .and_then(|u| u.as_str())
            .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
            .unwrap_or("unknown");
        let parent_body = parent
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
        if !parent_body.is_empty() {
            println!(
                "  > {} said: {}",
                parent_sender,
                truncate_with_ellipsis(&parent_body, 80)
            );
        }
    }

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

    // Sender -- try to get the name first, fall back to URN.
    let sender = msg.get("sender");
    let sender_name = sender
        .and_then(|s| {
            // Try member.firstName + lastName via participantType path.
            s.get("participantType")
                .and_then(|pt| pt.get("member"))
                .and_then(|m| {
                    let first = m
                        .get("firstName")
                        .and_then(|f| f.get("text").and_then(|v| v.as_str()).or_else(|| f.as_str()))
                        .unwrap_or("");
                    let last = m
                        .get("lastName")
                        .and_then(|l| l.get("text").and_then(|v| v.as_str()).or_else(|| l.as_str()))
                        .unwrap_or("");
                    if first.is_empty() && last.is_empty() {
                        None
                    } else {
                        Some(format!("{} {}", first, last).trim().to_string())
                    }
                })
        });

    let sender_display = sender_name.unwrap_or_else(|| {
        sender
            .and_then(|s| s.get("hostIdentityUrn"))
            .and_then(|u| u.as_str())
            .and_then(|u| u.strip_prefix("urn:li:fsd_profile:"))
            .unwrap_or("unknown")
            .to_string()
    });

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

    // Render content attachments (shared links, etc.).
    let render_content = msg
        .get("renderContentUnions")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|rc| {
                    rc.get("externalMedia")
                        .and_then(|em| {
                            let title = em
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let url = em
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if !url.is_empty() {
                                Some(format!("  [link] {} {}", title, url))
                            } else {
                                None
                            }
                        })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

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
    for content in &render_content {
        println!("{}", content);
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

// ---------------------------------------------------------------------------
// Feed cache utilities
// ---------------------------------------------------------------------------

/// Returns the path to the feed cache file.
fn feed_cache_path() -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir.join("linkedin").join("last_feed.json"))
}

/// Save raw feed JSON to cache for `feed read` / index-based react/comment.
fn save_feed_cache(value: &serde_json::Value) -> Result<(), String> {
    let path = feed_cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json =
        serde_json::to_string(value).map_err(|e| format!("failed to serialize feed cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write feed cache: {e}"))?;
    Ok(())
}

/// Load cached feed JSON. Returns an error if no cache exists.
fn load_feed_cache() -> Result<serde_json::Value, String> {
    let path = feed_cache_path()?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| "no cached feed. Run `feed list` first.".to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse feed cache: {e}"))
}

/// Extract the inner `urn:li:activity:XXXXX` from a feed element's entityUrn.
///
/// Feed entityUrns have formats like:
///   `urn:li:fs_feedUpdate:(V2&FOLLOW_FEED,urn:li:activity:7312345678901234567)`
///   `urn:li:activity:7312345678901234567`
fn extract_activity_urn(feed_entity_urn: &str) -> Option<String> {
    if let Some(start) = feed_entity_urn.find("urn:li:activity:") {
        let rest = &feed_entity_urn[start..];
        let end = rest.find(')').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

/// Resolve a post URN from either a literal URN string or a 1-based feed index.
///
/// If `post_urn_or_index` parses as a positive integer, loads the feed cache
/// and extracts the activity URN. Otherwise returns the string as-is.
fn resolve_post_urn(post_urn_or_index: &str) -> Result<String, String> {
    if let Ok(index) = post_urn_or_index.parse::<usize>() {
        if index == 0 {
            return Err("index must be >= 1".to_string());
        }
        let cache = load_feed_cache()?;
        let feed: FeedResponse = serde_json::from_value(cache)
            .map_err(|e| format!("failed to parse cached feed: {e}"))?;
        let element = feed.elements.get(index - 1).ok_or_else(|| {
            format!(
                "index {} out of range (feed has {} items)",
                index,
                feed.elements.len()
            )
        })?;
        let entity_urn = element
            .get("entityUrn")
            .or_else(|| element.get("urn"))
            .and_then(|u| u.as_str())
            .ok_or_else(|| "feed item has no entityUrn".to_string())?;
        extract_activity_urn(entity_urn)
            .ok_or_else(|| format!("could not extract activity URN from: {}", entity_urn))
    } else {
        Ok(post_urn_or_index.to_string())
    }
}

// ---------------------------------------------------------------------------
// feed read
// ---------------------------------------------------------------------------

/// Handle `feed read <index> [--json]`.
///
/// Shows full post details for item N from the last `feed list`.
fn cmd_feed_read(index: usize, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let cache = load_feed_cache()?;
    let feed: FeedResponse = serde_json::from_value(cache.clone())
        .map_err(|e| format!("failed to parse cached feed: {e}"))?;

    let element = feed.elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (feed has {} items)",
            index,
            feed.elements.len()
        )
    })?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(element).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    print_feed_item_full(index, element);
    Ok(())
}

/// Print full details of a single feed item (expanded view for `feed read`).
fn print_feed_item_full(index: usize, item: &serde_json::Value) {
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    let actor_name = update
        .get("actor")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(unknown author)");

    let actor_description = update
        .get("actor")
        .and_then(|a| a.get("description"))
        .and_then(|d| d.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(no text)");

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
    let shares = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"))
        .and_then(|c| c.get("numShares"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let entity_urn = item.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");
    let permalink = item
        .get("permalink")
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let activity_urn = extract_activity_urn(entity_urn).unwrap_or_default();

    println!("[{}] {}", index, actor_name);
    if !actor_description.is_empty() {
        println!("    {}", actor_description);
    }
    println!();
    println!("{}", commentary);

    // Reshared post content: LinkedIn puts reshared content in several locations.
    let reshared_update = update
        .get("resharedUpdate")
        .or_else(|| {
            update
                .get("content")
                .and_then(|c| c.get("resharedUpdate"))
        })
        .or_else(|| {
            // Also check inside the UpdateV2 union value
            update
                .get("content")
                .and_then(|c| {
                    c.get("com.linkedin.voyager.feed.render.UpdateV2")
                })
        });
    if let Some(reshared) = reshared_update {
        let orig_author = reshared
            .get("actor")
            .and_then(|a| a.get("name"))
            .and_then(|n| n.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("(unknown)");
        let orig_text = reshared
            .get("commentary")
            .and_then(|c| c.get("text"))
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        println!();
        println!("  [reshared from {}]", orig_author);
        if !orig_text.is_empty() {
            println!("  {}", truncate_with_ellipsis(orig_text, 300));
        }
    }

    // Article links: extract from content component.
    if let Some(article) = extract_article_info(update) {
        println!();
        if !article.title.is_empty() {
            println!("Article: {}", article.title);
        }
        if !article.url.is_empty() {
            println!("  {}", article.url);
        }
    }

    // Media type labels + URLs.
    let media_label = extract_media_type_label(update);
    if !media_label.is_empty() {
        println!("Media: {}", media_label);
    }
    let media_urls = extract_media_urls(update);
    for url in &media_urls {
        println!("  {}", url);
    }

    println!();
    println!(
        "likes: {}  comments: {}  shares: {}",
        likes, comments, shares
    );
    if !activity_urn.is_empty() {
        println!("URN: {}", activity_urn);
    }
    if !permalink.is_empty() {
        println!("Link: {}", permalink);
    }
}

// ---------------------------------------------------------------------------
// feed read helpers
// ---------------------------------------------------------------------------

/// Article info extracted from a feed item's content component.
struct ArticleInfo {
    title: String,
    url: String,
}

/// Extract article title and URL from a feed item's content component.
///
/// LinkedIn wraps article content in several possible locations:
/// - `content.articleComponent` (standard articles)
/// - `content.navigationContext` (link previews)
/// - `content["com.linkedin.voyager.feed.render.ArticleComponent"]` (Rest.li union)
fn extract_article_info(update: &serde_json::Value) -> Option<ArticleInfo> {
    let content = update.get("content")?;

    // Try articleComponent first (most common for shared articles).
    if let Some(article) = content
        .get("articleComponent")
        .or_else(|| {
            content.get("com.linkedin.voyager.feed.render.ArticleComponent")
        })
    {
        let title = article
            .get("title")
            .and_then(|t| t.get("text").and_then(|v| v.as_str()).or_else(|| t.as_str()))
            .unwrap_or("")
            .to_string();
        let url = article
            .get("navigationContext")
            .and_then(|n| n.get("actionTarget"))
            .and_then(|v| v.as_str())
            .or_else(|| article.get("url").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();
        if !title.is_empty() || !url.is_empty() {
            return Some(ArticleInfo { title, url });
        }
    }

    // Try top-level navigationContext on the content node.
    if let Some(nav) = content.get("navigationContext") {
        let title = nav
            .get("accessibilityText")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let url = nav
            .get("actionTarget")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !url.is_empty() {
            return Some(ArticleInfo { title, url });
        }
    }

    None
}

/// Determine the media type label for a feed item's content.
///
/// Returns a label like "image", "video", "document", "poll", "article",
/// or an empty string if no media is detected.
fn extract_media_type_label(update: &serde_json::Value) -> String {
    let content = match update.get("content") {
        Some(c) => c,
        None => return String::new(),
    };

    // Check for specific component types (Rest.li union keys or direct fields).
    let type_checks: &[(&str, &str)] = &[
        ("com.linkedin.voyager.feed.render.ImageComponent", "image"),
        ("com.linkedin.voyager.feed.render.LinkedInVideoComponent", "video"),
        ("com.linkedin.voyager.feed.render.DocumentComponent", "document"),
        ("com.linkedin.voyager.feed.render.PollComponent", "poll"),
        ("com.linkedin.voyager.feed.render.ArticleComponent", "article"),
        ("com.linkedin.voyager.feed.render.CelebrationComponent", "celebration"),
        ("com.linkedin.voyager.feed.render.CarouselComponent", "carousel"),
        ("imageComponent", "image"),
        ("videoComponent", "video"),
        ("documentComponent", "document"),
        ("pollComponent", "poll"),
        ("articleComponent", "article"),
        ("celebrationComponent", "celebration"),
        ("carouselComponent", "carousel"),
    ];

    for (key, label) in type_checks {
        if content.get(*key).is_some() {
            return label.to_string();
        }
    }

    // Check for $type field (some responses use this).
    if let Some(type_str) = content.get("$type").and_then(|t| t.as_str()) {
        if type_str.contains("Image") {
            return "image".to_string();
        }
        if type_str.contains("Video") {
            return "video".to_string();
        }
        if type_str.contains("Document") {
            return "document".to_string();
        }
        if type_str.contains("Poll") {
            return "poll".to_string();
        }
        if type_str.contains("Article") {
            return "article".to_string();
        }
    }

    String::new()
}

/// Extract media URLs (images, videos, documents) from a feed item's content.
///
/// LinkedIn stores media URLs in various nested locations depending on type.
/// Returns a vec of URL strings found.
fn extract_media_urls(update: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();
    let content = match update.get("content") {
        Some(c) => c,
        None => return urls,
    };

    // Image URLs: look in imageComponent or the union variant.
    let image_comp = content
        .get("imageComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.ImageComponent"));
    if let Some(img) = image_comp {
        // Images are in images[].attributes[].imageUrl or
        // images[].attributes[].vectorImage.rootUrl + artifacts[].fileIdentifyingUrlPathSegment
        collect_image_urls(img, &mut urls);
    }

    // Video URLs: look in videoComponent or the union variant.
    let video_comp = content
        .get("videoComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.LinkedInVideoComponent"));
    if let Some(vid) = video_comp {
        // progressiveStreams[].streamingLocations[].url or videoPlayMetadata.progressiveStreams
        if let Some(play_meta) = vid
            .get("videoPlayMetadata")
            .or_else(|| vid.get("videoPlay"))
        {
            if let Some(streams) = play_meta
                .get("progressiveStreams")
                .and_then(|s| s.as_array())
            {
                for stream in streams {
                    if let Some(url) = stream
                        .get("streamingLocations")
                        .and_then(|sl| sl.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|loc| loc.get("url"))
                        .and_then(|v| v.as_str())
                    {
                        urls.push(url.to_string());
                        break; // One stream URL is enough.
                    }
                }
            }
            // Also try mediaUrl directly.
            if let Some(url) = play_meta.get("media").and_then(|v| v.as_str()) {
                urls.push(url.to_string());
            }
        }
        // Try thumbnail/poster.
        if let Some(poster) = vid
            .get("thumbnail")
            .and_then(|t| {
                t.get("url")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        t.get("rootUrl")
                            .and_then(|v| v.as_str())
                    })
            })
        {
            if urls.is_empty() {
                urls.push(format!("(thumbnail) {}", poster));
            }
        }
    }

    // Document URLs: look in documentComponent or the union variant.
    let doc_comp = content
        .get("documentComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.DocumentComponent"));
    if let Some(doc) = doc_comp {
        if let Some(url) = doc
            .get("document")
            .and_then(|d| d.get("transcribedDocumentUrl").and_then(|v| v.as_str()))
            .or_else(|| {
                doc.get("document")
                    .and_then(|d| d.get("downloadUrl").and_then(|v| v.as_str()))
            })
        {
            urls.push(url.to_string());
        }
    }

    // Carousel images.
    let carousel_comp = content
        .get("carouselComponent")
        .or_else(|| content.get("com.linkedin.voyager.feed.render.CarouselComponent"));
    if let Some(carousel) = carousel_comp {
        if let Some(pages) = carousel.get("pages").and_then(|p| p.as_array()) {
            for page in pages.iter().take(5) {
                // Each page may have an imageComponent.
                if let Some(img) = page.get("imageComponent") {
                    collect_image_urls(img, &mut urls);
                }
            }
        }
    }

    urls
}

/// Collect image URLs from an image component into the urls vec.
fn collect_image_urls(img: &serde_json::Value, urls: &mut Vec<String>) {
    // Try images[].attributes[].imageUrl first.
    if let Some(images) = img.get("images").and_then(|i| i.as_array()) {
        for image in images {
            if let Some(attrs) = image.get("attributes").and_then(|a| a.as_array()) {
                for attr in attrs {
                    if let Some(url) = attr.get("imageUrl").and_then(|v| v.as_str()) {
                        urls.push(url.to_string());
                        return;
                    }
                    // Try vectorImage: rootUrl + largest artifact.
                    if let Some(vi) = attr.get("vectorImage") {
                        if let Some(root) = vi.get("rootUrl").and_then(|v| v.as_str()) {
                            let segment = vi
                                .get("artifacts")
                                .and_then(|a| a.as_array())
                                .and_then(|arr| arr.last())
                                .and_then(|a| {
                                    a.get("fileIdentifyingUrlPathSegment")
                                        .and_then(|v| v.as_str())
                                })
                                .unwrap_or("");
                            urls.push(format!("{}{}", root, segment));
                            return;
                        }
                    }
                }
            }
        }
    }
    // Fallback: try a direct url field.
    if let Some(url) = img.get("url").and_then(|v| v.as_str()) {
        urls.push(url.to_string());
    }
}

// ---------------------------------------------------------------------------
// feed comments
// ---------------------------------------------------------------------------

/// Handle `feed comments <index> [--count N] [--json]`.
///
/// Fetches comments on a post by index from the cached feed.
async fn cmd_feed_comments(index: usize, count: u32, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let cache = load_feed_cache()?;
    let feed: FeedResponse = serde_json::from_value(cache)
        .map_err(|e| format!("failed to parse cached feed: {e}"))?;

    let element = feed.elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (feed has {} items)",
            index,
            feed.elements.len()
        )
    })?;

    // Extract the socialDetail URN needed for the comments API.
    let update = element
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(element);

    let social_detail_urn = update
        .get("socialDetail")
        .and_then(|sd| sd.get("dashEntityUrn").or_else(|| sd.get("entityUrn")))
        .and_then(|u| u.as_str())
        .ok_or_else(|| "feed item has no socialDetail URN".to_string())?;

    let (client, _path) = load_session_client()?;

    let value = client
        .get_comments(social_detail_urn, 0, count)
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
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    if elements.is_empty() {
        println!("(no comments)");
        return Ok(());
    }

    println!("Comments on post [{}]:", index);
    println!("---");

    for (i, comment) in elements.iter().enumerate() {
        let commenter = comment
            .get("commenter")
            .and_then(|c| {
                // Try title.text first, then fall back to accessibilityText
                c.get("title")
                    .and_then(|t| t.get("text"))
                    .and_then(|v| v.as_str())
                    .or_else(|| c.get("accessibilityText").and_then(|v| v.as_str()))
            })
            .unwrap_or("(unknown)");

        // Commentary text is directly at commentary.text (not nested)
        let text = comment
            .get("commentary")
            .and_then(|c| {
                c.get("text")
                    .and_then(|t| {
                        // Could be a string directly or a nested {text: "..."} object
                        t.as_str().or_else(|| t.get("text").and_then(|v| v.as_str()))
                    })
            })
            .unwrap_or("");

        let likes = comment
            .get("socialDetail")
            .and_then(|sd| sd.get("totalSocialActivityCounts"))
            .and_then(|c| c.get("numLikes"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0);

        println!("[{}] {}", i + 1, commenter);
        println!("    {}", text);
        if likes > 0 {
            println!("    likes: {}", likes);
        }
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// search posts
// ---------------------------------------------------------------------------

/// Handle `search posts <keywords> [--count N] [--start N] [--json]`.
async fn cmd_search_posts(
    keywords: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .search_content(keywords, start, count)
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
        print_paging_header(&format!("Post results for '{}'", keywords), paging);
    }
    println!("---");

    // Content search returns `searchFeedUpdate` items (not `entityResult`).
    let mut result_idx = start as usize;
    let mut any_results = false;
    for cluster in &resp.elements {
        let items = cluster
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            let sfu = item_wrapper
                .get("item")
                .and_then(|i| i.get("searchFeedUpdate"));
            if let Some(sfu) = sfu {
                result_idx += 1;
                any_results = true;
                print_search_content(result_idx, sfu);
                println!();
            }
        }
    }

    if !any_results {
        println!("(no results)");
    }

    Ok(())
}

/// Print a human-readable summary of a content search result.
///
/// Content search results use `searchFeedUpdate` which contains an `update`
/// field with the same structure as feed `UpdateV2` items.
fn print_search_content(index: usize, sfu: &serde_json::Value) {
    let update = sfu.get("update").unwrap_or(sfu);

    let actor_name = update
        .get("actor")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(unknown)");

    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let commentary_display = truncate_with_ellipsis(commentary, 200);

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

    // Extract the activity URN for react/comment support.
    // Primary source: update.metadata.backendUrn (contains urn:li:activity:XXX).
    // Fallback: update.entityUrn, socialDetail.entityUrn, or permalink.
    let activity_urn = update
        .get("metadata")
        .and_then(|m| m.get("backendUrn"))
        .and_then(|u| u.as_str())
        .and_then(|u| {
            if u.starts_with("urn:li:activity:") {
                Some(u.to_string())
            } else {
                extract_activity_urn(u)
            }
        })
        .or_else(|| {
            update
                .get("entityUrn")
                .and_then(|u| u.as_str())
                .and_then(|u| extract_activity_urn(u))
        })
        .or_else(|| {
            update
                .get("socialDetail")
                .and_then(|sd| sd.get("entityUrn"))
                .and_then(|u| u.as_str())
                .and_then(|u| extract_activity_urn(u))
        })
        .unwrap_or_default();

    let permalink = sfu
        .get("permalink")
        .and_then(|p| p.as_str())
        .unwrap_or("");

    println!("[{}] {}", index, actor_name);
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    println!("    likes: {}  comments: {}", likes, comments);
    if !activity_urn.is_empty() {
        // Generate a LinkedIn post URL from the activity URN.
        let activity_id = activity_urn
            .strip_prefix("urn:li:activity:")
            .unwrap_or(&activity_urn);
        println!(
            "    https://www.linkedin.com/feed/update/urn:li:activity:{}",
            activity_id
        );
        println!("    URN: {}", activity_urn);
    }
    if !permalink.is_empty() && activity_urn.is_empty() {
        println!("    {}", permalink);
    }
}
