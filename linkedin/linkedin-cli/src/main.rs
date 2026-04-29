use std::fs;
use std::process;

use chrono::Datelike;
use clap::{CommandFactory, Parser, Subcommand};
use linkedin_api::auth::Session;
use linkedin_api::client::LinkedInClient;
use linkedin_api::models::{
    ConnectionsResponse, FeedResponse, NotificationCardsResponse, Paging, SearchResponse,
};

/// Exit codes for structured error reporting.
///
/// Scripts can check `$?` to distinguish error types:
/// - 0: success
/// - 1: general/unknown error
/// - 2: authentication error (expired session, invalid cookie)
/// - 3: API error (rate limited, server error, bad response)
/// - 4: input error (invalid arguments, missing required flags)
mod exit_code {
    pub const AUTH: i32 = 2;
    pub const API: i32 = 3;
    pub const INPUT: i32 = 4;
}

/// Classify an error string into an exit code.
///
/// Inspects the error message for known patterns to determine the
/// appropriate exit code.
fn classify_error(msg: &str) -> i32 {
    if msg.contains("session")
        || msg.contains("auth")
        || msg.contains("li_at")
        || msg.contains("401")
        || msg.contains("not logged in")
    {
        exit_code::AUTH
    } else if msg.contains("API")
        || msg.contains("HTTP")
        || msg.contains("429")
        || msg.contains("500")
        || msg.contains("failed to send")
    {
        exit_code::API
    } else if msg.contains("index must be")
        || msg.contains("invalid")
        || msg.contains("must not be empty")
        || msg.contains("out of range")
        || msg.contains("not confirmed")
    {
        exit_code::INPUT
    } else {
        1
    }
}

/// Suggest a one-line next step the user can take, based on the shape of
/// an error message. Returns `None` if no specific hint applies.
///
/// Pattern matching is intentionally string-based so it works on errors
/// surfaced through `format!("…: {e}")` chains where the original
/// `Error` variant is already lost. Each branch picks the most actionable
/// next move first; ordering matters when multiple substrings match.
fn error_hint(msg: &str) -> Option<&'static str> {
    if msg.contains("HTTP 401") || msg.contains("session expired") || msg.contains("li_at") {
        Some("Hint: session looks stale. Refresh with: li auth login <li_at_cookie>")
    } else if msg.contains("HTTP 301") {
        Some("Hint: HTTP 301 from a Voyager endpoint usually means LinkedIn retired it. Check re/ docs for the modern path; if the path is still listed there, file an issue.")
    } else if msg.contains("HTTP 429") {
        Some("Hint: rate limited. Wait a few minutes; consider lowering --pacing-ms below 2000 only if you've checked your quota.")
    } else if msg.contains("Internal error fetching data from downstream")
        || msg.contains("Failed to get response from server")
    {
        Some("Hint: transient GraphQL error. The client already retried up to MAX_RETRIES times — if you see this often, LinkedIn's mesh is degraded. Try again in a minute.")
    } else if msg.contains("failed to resolve profile") {
        Some("Hint: the slug→URN resolver flaked. Pass the fsd_profile URN directly if you have it, or use 'li search invite N' once the person is in your last search results.")
    } else if msg.contains("HTTP 403") {
        Some("Hint: LinkedIn blocked the request. This often means a captcha challenge — open the site in a browser and complete any pending verification.")
    } else if msg.contains("out of range") {
        Some("Hint: rerun the parent command (e.g. 'li search people \"…\"' or 'li feed list') first to populate the cache.")
    } else {
        None
    }
}

#[derive(Parser)]
#[command(name = "li")]
#[command(about = "LinkedIn in the terminal", version)]
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
    /// LinkedIn Events
    Events {
        #[command(subcommand)]
        action: EventsAction,
    },
    /// Notifications
    Notifications {
        #[command(subcommand)]
        action: NotificationsAction,
    },
    /// Daily inbox: unread messages, pending invitations, recent notifications
    Inbox {
        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,

        /// Show all messages including likely recruiter spam
        #[arg(long)]
        all: bool,
    },
    /// Who do you know at a company? Network overlap in one command.
    Who {
        /// Company URL slug (e.g. "miro" from linkedin.com/company/miro)
        company: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
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
    /// Display a post by activity URN
    ///
    /// First scans the top 50 items of your feed for a match (cache-warm
    /// path, no extra network call). On miss, falls back to LinkedIn's
    /// `highlightedFeed` finder, which fetches by URN. For posts that are
    /// already in your last `feed list`, `feed read N` is faster — it
    /// pulls from the cached listing without any network call.
    View {
        /// Activity URN (urn:li:activity:1234...) or just the numeric ID
        activity_urn: String,

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
    /// List your own posts with engagement analytics
    ///
    /// Shows your recent posts with impressions, reactions, comments, and
    /// shares. Data comes from LinkedIn's content analytics ("Me" tab).
    MyPosts {
        /// Number of posts to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Show who reacted to a specific post
    ///
    /// Lists reactor names, headlines, and reaction types (like, celebrate,
    /// love, insightful, support, funny) for a given post.
    ///
    /// LinkedIn's reactions endpoint requires different URN types for
    /// different post backings (ugcPost for own posts, activity for reshares).
    /// Prefer `--from-list N` after a `feed list` or `feed my-posts` call to
    /// let the CLI pick the right URN from the cached listing.
    Reactions {
        /// Post/activity URN (e.g. urn:li:activity:7312345678901234567).
        /// Omit when using --from-list.
        post_urn: Option<String>,

        /// Use item N from the last `feed list` or `feed my-posts` listing.
        /// The CLI extracts the correct URN (ugcPost vs activity) from the
        /// cached element.
        #[arg(long, conflicts_with = "post_urn")]
        from_list: Option<usize>,

        /// Number of reactions to fetch (default: 50)
        #[arg(long, default_value = "50")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Show aggregate engagement stats across your recent posts
    ///
    /// Fetches your last 20 posts and computes totals and averages for
    /// views, reactions, comments, and reposts.
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
        #[arg(long, conflicts_with = "summary")]
        json: bool,

        /// Output a compact JSON summary (URN, public ID, name, headline,
        /// location, relationship state, follower count). Useful for scripting
        /// and batch flows. Suppresses the full ~150KB profile dump.
        #[arg(long)]
        summary: bool,
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
    /// Audit your profile for staleness and missing sections
    Audit {
        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ConnectionsAction {
    /// List connections
    List {
        /// Number of connections to fetch per page (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Pagination offset (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,

        /// Fetch all connections (auto-paginate with built-in throttling)
        #[arg(long)]
        all: bool,

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
    /// Send connection invitations to multiple members from a list
    ///
    /// Reads slugs/URNs (one per line) from --from-file or stdin and sends
    /// each. Lines starting with `#` and blank lines are ignored. The
    /// command keeps going on per-line failures and reports a tab-separated
    /// status line per input. Sleeps between calls to respect rate limits.
    InviteBatch {
        /// Path to a file with one slug or URN per line. Use `-` for stdin.
        #[arg(long, default_value = "-")]
        from_file: String,

        /// Optional custom message included with every invitation
        #[arg(long)]
        message: Option<String>,

        /// Milliseconds to sleep between calls (default 2000)
        #[arg(long, default_value = "2000")]
        pacing_ms: u64,

        /// Stop on the first failure instead of continuing through the list
        #[arg(long)]
        stop_on_error: bool,
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
    /// Withdraw a sent (pending) connection invitation
    ///
    /// Cancels an outgoing invitation that hasn't been accepted yet. Pairs
    /// with `connections invite` to undo invites you sent. The invitation
    /// URN and shared secret come from the sent-invitations list (capture
    /// from devtools or a future `connections sent` command).
    Withdraw {
        /// Invitation ID (numeric portion of the invitation URN, e.g. 7312345678901234567)
        invitation_id: String,

        /// Shared secret from the invitation (required for CSRF protection).
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
    /// React to a post from the last search results
    React {
        /// 1-based index from the most recent `search posts` results
        index: usize,

        /// Reaction type: LIKE, PRAISE, EMPATHY, INTEREST, APPRECIATION, ENTERTAINMENT, CELEBRATION
        #[arg(long = "type", default_value = "LIKE")]
        reaction_type: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// View a profile from the last people search results
    View {
        /// 1-based index from the most recent `search people` results
        index: usize,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// Send a connection invitation to a profile from the last people
    /// search results, by index. Uses the cached entity URN directly,
    /// bypassing the slug-resolver path that depends on the GraphQL profile
    /// endpoint.
    Invite {
        /// 1-based index from the most recent `search people` results
        index: usize,

        /// Optional custom message to include with the invitation
        #[arg(long)]
        message: Option<String>,

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
    /// Show everyone @-mentioned in the post behind notification N
    ///
    /// Indexes into the most recent `notifications list` output, extracts
    /// the underlying post URN, fetches the post body, and prints the
    /// fsd_profile URNs of every member mentioned (yourself plus anyone
    /// else tagged in the same post).
    Mentions {
        /// 1-based index from the most recent `notifications list` output
        index: usize,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum EventsAction {
    /// View event details by event ID
    View {
        /// Event ID (numeric, from the event URL)
        event_id: String,

        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
    },
    /// List event attendees
    Attendees {
        /// Event ID (numeric, from the event URL)
        event_id: String,

        /// Number of attendees to fetch (default: 50)
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

/// Run a command and exit with a classified error code on failure.
fn exit_on_err(result: Result<(), String>) {
    if let Err(e) = result {
        let code = classify_error(&e);
        eprintln!("error: {e}");
        if let Some(hint) = error_hint(&e) {
            eprintln!("{hint}");
        }
        process::exit(code);
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { action } => match action {
            AuthAction::Login { li_at } => exit_on_err(cmd_auth_login(li_at).await),
            AuthAction::Status { local } => exit_on_err(cmd_auth_status(local).await),
            AuthAction::Logout => exit_on_err(cmd_auth_logout()),
        },
        Commands::Profile { action } => match action {
            ProfileAction::Me { json } => exit_on_err(cmd_profile_me(json).await),
            ProfileAction::View {
                public_id,
                json,
                summary,
            } => exit_on_err(cmd_profile_view(&public_id, json, summary).await),
            ProfileAction::Visit { public_id, json } => {
                exit_on_err(cmd_profile_visit(&public_id, json).await)
            }
            ProfileAction::Viewers { json } => exit_on_err(cmd_profile_viewers(json).await),
            ProfileAction::Audit { json } => exit_on_err(cmd_profile_audit(json).await),
        },
        Commands::Messages { action } => match action {
            MessagesAction::List {
                count,
                before,
                json,
            } => exit_on_err(cmd_messages_list(count, before, json).await),
            MessagesAction::Read {
                conversation_id,
                before,
                json,
            } => exit_on_err(cmd_messages_read(&conversation_id, before, json).await),
            MessagesAction::Send {
                recipient,
                message,
                json,
            } => exit_on_err(cmd_messages_send(&recipient, &message, json).await),
            MessagesAction::Reply {
                conversation_id,
                message,
                yes,
                json,
            } => exit_on_err(cmd_messages_reply(&conversation_id, &message, yes, json).await),
        },
        Commands::Feed { action } => match action {
            FeedAction::List {
                count,
                start,
                author,
                keyword,
                json,
            } => exit_on_err(
                cmd_feed_list(start, count, author.as_deref(), keyword.as_deref(), json).await,
            ),
            FeedAction::Comments { index, count, json } => {
                exit_on_err(cmd_feed_comments(index, count, json).await)
            }
            FeedAction::Read { index, json } => exit_on_err(cmd_feed_read(index, json)),
            FeedAction::View { activity_urn, json } => {
                exit_on_err(cmd_feed_view(&activity_urn, json).await)
            }
            FeedAction::React {
                post_urn,
                reaction_type,
                json,
            } => exit_on_err(cmd_feed_react(&post_urn, &reaction_type, json).await),
            FeedAction::Unreact {
                post_urn,
                reaction_type,
                json,
            } => exit_on_err(cmd_feed_unreact(&post_urn, &reaction_type, json).await),
            FeedAction::Comment {
                post_urn,
                text,
                yes,
                json,
            } => exit_on_err(cmd_feed_comment(&post_urn, &text, yes, json).await),
            FeedAction::MyPosts { count, start, json } => {
                exit_on_err(cmd_feed_my_posts(start, count, json).await)
            }
            FeedAction::Reactions {
                post_urn,
                from_list,
                count,
                start,
                json,
            } => exit_on_err(
                cmd_feed_reactions(post_urn.as_deref(), from_list, start, count, json).await,
            ),
            FeedAction::Stats { json } => exit_on_err(cmd_feed_stats(json).await),
            FeedAction::Post {
                text,
                visibility,
                yes,
                json,
            } => exit_on_err(cmd_feed_post(&text, &visibility, yes, json).await),
        },
        Commands::Connections { action } => match action {
            ConnectionsAction::List {
                count,
                start,
                all,
                keyword,
                json,
            } => {
                exit_on_err(cmd_connections_list(start, count, all, keyword.as_deref(), json).await)
            }
            ConnectionsAction::Invite {
                public_id_or_urn,
                message,
                json,
            } => exit_on_err(
                cmd_connections_invite(&public_id_or_urn, message.as_deref(), json).await,
            ),
            ConnectionsAction::InviteBatch {
                from_file,
                message,
                pacing_ms,
                stop_on_error,
            } => exit_on_err(
                cmd_connections_invite_batch(
                    &from_file,
                    message.as_deref(),
                    pacing_ms,
                    stop_on_error,
                )
                .await,
            ),
            ConnectionsAction::Invitations { count, start, json } => {
                exit_on_err(cmd_connections_invitations(start, count, json).await)
            }
            ConnectionsAction::Accept {
                invitation_id,
                secret,
                json,
            } => exit_on_err(cmd_connections_accept(&invitation_id, &secret, json).await),
            ConnectionsAction::Withdraw {
                invitation_id,
                secret,
                json,
            } => exit_on_err(cmd_connections_withdraw(&invitation_id, &secret, json).await),
        },
        Commands::Search { action } => match action {
            SearchAction::People {
                keywords,
                count,
                start,
                json,
            } => exit_on_err(cmd_search_people(&keywords, start, count, json).await),
            SearchAction::Jobs {
                keywords,
                count,
                start,
                json,
            } => exit_on_err(cmd_search_jobs(&keywords, start, count, json).await),
            SearchAction::Posts {
                keywords,
                count,
                start,
                json,
            } => exit_on_err(cmd_search_posts(&keywords, start, count, json).await),
            SearchAction::React {
                index,
                reaction_type,
                json,
            } => exit_on_err(cmd_search_react(index, &reaction_type, json).await),
            SearchAction::View { index, json } => exit_on_err(cmd_search_view(index, json).await),
            SearchAction::Invite {
                index,
                message,
                json,
            } => exit_on_err(cmd_search_invite(index, message.as_deref(), json).await),
        },
        Commands::Events { action } => match action {
            EventsAction::View { event_id, json } => {
                exit_on_err(cmd_event_view(&event_id, json).await)
            }
            EventsAction::Attendees {
                event_id,
                count,
                start,
                json,
            } => exit_on_err(cmd_event_attendees(&event_id, start, count, json).await),
        },
        Commands::Company { action } => match action {
            CompanyAction::View { slug, json } => exit_on_err(cmd_company_view(&slug, json).await),
            CompanyAction::Followers {
                slug,
                count,
                start,
                json,
            } => exit_on_err(cmd_company_followers(&slug, start, count, json).await),
        },
        Commands::Notifications { action } => match action {
            NotificationsAction::List { count, start, json } => {
                exit_on_err(cmd_notifications_list(start, count, json).await)
            }
            NotificationsAction::Mentions { index, json } => {
                exit_on_err(cmd_notifications_mentions(index, json).await)
            }
        },
        Commands::Inbox { json, all } => exit_on_err(cmd_inbox(json, all).await),
        Commands::Who { company, json } => exit_on_err(cmd_who(&company, json).await),
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "li", &mut std::io::stdout());
        }
    }
}

/// Spam-detection patterns for recruiter messages and invitations.
///
/// Returns `true` if the text looks like recruiter spam. Patterns are
/// case-insensitive substring matches against common recruiter template
/// phrases and headline keywords.
fn looks_like_spam(text: &str) -> bool {
    let lower = text.to_lowercase();

    // Recruiter headline patterns (invitation subtitles / sender occupations).
    const HEADLINE_PATTERNS: &[&str] = &[
        "recruiter",
        "talent acquisition",
        "staffing",
        "headhunter",
        "hiring manager",
        "sourcer",
        "recruitment",
        "people partner",
    ];

    // Message body template phrases.
    const BODY_PATTERNS: &[&str] = &[
        "exciting opportunity",
        "perfect fit",
        "open role",
        "open position",
        "i came across your profile",
        "i found your profile",
        "your background caught",
        "impressive profile",
        "impressive background",
        "great match",
        "thriving team",
        "i'd love to connect",
        "competitive salary",
        "competitive compensation",
        "base salary",
        "reach out to discuss",
        "we are currently hiring",
        "we're currently hiring",
        "looking for a",
        "we are looking",
        "we're looking",
        "wonderful opportunity",
        "are you open to",
    ];

    for pat in HEADLINE_PATTERNS.iter().chain(BODY_PATTERNS.iter()) {
        if lower.contains(pat) {
            return true;
        }
    }
    false
}

/// Check if a conversation element looks like recruiter spam.
///
/// Checks participant names/headlines and the last message body.
fn is_spam_conversation(conv: &serde_json::Value) -> bool {
    // Check title field.
    if let Some(title) = conv.get("title").and_then(|v| v.as_str()) {
        if looks_like_spam(title) {
            return true;
        }
    }

    // Check participant headlines/occupations.
    if let Some(participants) = conv
        .get("conversationParticipants")
        .and_then(|p| p.as_array())
    {
        for p in participants {
            if let Some(member) = p.get("participantType").and_then(|pt| pt.get("member")) {
                if let Some(headline) = member.get("headline").and_then(|h| {
                    h.get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| h.as_str())
                }) {
                    if looks_like_spam(headline) {
                        return true;
                    }
                }
                if let Some(occ) = member.get("occupation").and_then(|v| v.as_str()) {
                    if looks_like_spam(occ) {
                        return true;
                    }
                }
            }
        }
    }

    // Check last message body.
    let last_msg_text = conv
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

    looks_like_spam(&last_msg_text)
}

/// Check if an invitation element looks like recruiter spam.
fn is_spam_invitation(inv: &serde_json::Value) -> bool {
    if let Some(headline) = inv
        .get("subtitle")
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
    {
        if looks_like_spam(headline) {
            return true;
        }
    }
    if let Some(msg) = inv
        .get("invitation")
        .and_then(|i| i.get("message"))
        .and_then(|v| v.as_str())
    {
        if looks_like_spam(msg) {
            return true;
        }
    }
    false
}

/// Handle `inbox [--json] [--all]`.
///
/// Shows a daily summary: unread messages, pending invitations, and
/// recent unread notifications in one view. Filters likely recruiter
/// spam by default; use `--all` to see everything.
async fn cmd_inbox(raw_json: bool, show_all: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // Fetch all three in sequence (rate limiter handles throttling).
    let conversations = client
        .get_conversations(10, None)
        .await
        .map_err(|e| format!("failed to fetch messages: {e}"))?;
    let invitations = client
        .get_invitations(0, 10)
        .await
        .map_err(|e| format!("failed to fetch invitations: {e}"))?;
    let notifications = client
        .get_notifications(0, 10)
        .await
        .map_err(|e| format!("failed to fetch notifications: {e}"))?;

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
async fn cmd_who(company_slug: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    // 1. Fetch company info.
    eprintln!("Looking up {}...", company_slug);
    let company = client
        .get_company(company_slug)
        .await
        .map_err(|e| format!("company lookup failed: {e}"))?;

    let company_name = company
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(company_slug);
    let hq = company
        .get("headquarter")
        .and_then(|h| h.get("city"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let hq_country = company
        .get("headquarter")
        .and_then(|h| h.get("country"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
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
        .unwrap_or("");
    let tagline = company
        .get("tagline")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Build search terms: company name and common variants.
    let name_lower = company_name.to_lowercase();

    // 2. Scan connections (all pages) for people at this company.
    eprintln!("Scanning connections...");
    let mut my_connections: Vec<(String, String, String)> = Vec::new(); // (name, headline, public_id)
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
            let pub_id = mini
                .and_then(|m| m.get("publicIdentifier").and_then(|v| v.as_str()))
                .unwrap_or("");

            if headline.to_lowercase().contains(&name_lower)
                || headline
                    .to_lowercase()
                    .contains(&company_slug.to_lowercase())
            {
                my_connections.push((
                    format!("{} {}", first, last).trim().to_string(),
                    headline.to_string(),
                    pub_id.to_string(),
                ));
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

    // 3. Check profile viewers for anyone at this company.
    eprintln!("Checking profile viewers...");
    let mut viewers_at_company: Vec<(String, String)> = Vec::new(); // (name, headline)
    if let Ok(viewers_data) = client.get_profile_viewers().await {
        let viewers_str = serde_json::to_string(&viewers_data).unwrap_or_default();
        // Quick scan: if company name appears anywhere in the viewers response,
        // extract the individual viewer cards.
        if viewers_str.to_lowercase().contains(&name_lower) {
            if let Some(elements) = viewers_data.get("elements").and_then(|e| e.as_array()) {
                for el in elements {
                    let el_str = serde_json::to_string(el).unwrap_or_default();
                    if el_str.to_lowercase().contains(&name_lower) {
                        // Try to extract name and headline from the deeply nested structure.
                        let name = el
                            .pointer("/value/com.linkedin.voyager.identity.me.WvmpViewersCard/insightCards")
                            .and_then(|cards| cards.as_array())
                            .and_then(|arr| arr.iter().find(|c| {
                                serde_json::to_string(c).unwrap_or_default().to_lowercase().contains(&name_lower)
                            }))
                            .and_then(|card| {
                                // Extract from card text
                                card.pointer("/value/com.linkedin.voyager.identity.me.WvmpSummaryInsightCard/cards")
                                    .and_then(|c| c.as_array())
                                    .and_then(|arr| arr.iter().find(|c| {
                                        serde_json::to_string(c).unwrap_or_default().to_lowercase().contains(&name_lower)
                                    }))
                            });
                        if name.is_some() {
                            viewers_at_company.push((
                                "(viewer from company)".to_string(),
                                company_name.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    // 4. Check recent messages for conversations with people at this company.
    eprintln!("Checking messages...");
    let mut messages_from_company: Vec<(String, String, String)> = Vec::new(); // (name, last_msg, conv_id)
    if let Ok(conversations) = client.get_conversations(20, None).await {
        if let Some(elements) = conversations.get("elements").and_then(|e| e.as_array()) {
            for conv in elements {
                let conv_str = serde_json::to_string(conv).unwrap_or_default();
                if conv_str.to_lowercase().contains(&name_lower) {
                    let names = extract_conversation_names(conv);
                    let display = names.join(", ");
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
                    let conv_id = conv
                        .get("backendUrn")
                        .and_then(|u| u.as_str())
                        .and_then(|u| u.strip_prefix("urn:li:messagingThread:"))
                        .unwrap_or("")
                        .to_string();
                    messages_from_company.push((display, last_msg, conv_id));
                }
            }
        }
    }

    // 5. Search for notable people at the company.
    eprintln!("Searching people at {}...", company_name);
    let search_results = client
        .search_people(company_name, 0, 5)
        .await
        .unwrap_or_default();
    let mut notable_people: Vec<(String, String, String, String)> = Vec::new(); // (name, headline, degree, pub_id)
    if let Some(elements) = search_results.get("elements").and_then(|e| e.as_array()) {
        for el in elements {
            if let Some(items) = el.get("items").and_then(|i| i.as_array()) {
                for item in items {
                    let er = item
                        .pointer("/item/entityResult")
                        .unwrap_or(&serde_json::Value::Null);
                    let title = er
                        .get("title")
                        .and_then(|t| t.get("text").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let headline = er
                        .get("primarySubtitle")
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
                    let pub_id = nav_url
                        .strip_prefix("https://www.linkedin.com/in/")
                        .and_then(|s| s.split('?').next())
                        .unwrap_or("");

                    // Only include people whose headline mentions the company.
                    if headline.to_lowercase().contains(&name_lower)
                        || headline
                            .to_lowercase()
                            .contains(&company_slug.to_lowercase())
                    {
                        notable_people.push((
                            title.to_string(),
                            headline.to_string(),
                            badge.to_string(),
                            pub_id.to_string(),
                        ));
                    }
                }
            }
        }
    }

    // --- Output ---

    if raw_json {
        let output = serde_json::json!({
            "company": {
                "name": company_name,
                "slug": company_slug,
                "hq": format!("{}, {}", hq, hq_country),
                "staff": staff,
                "industry": industry,
            },
            "connections": my_connections.iter().map(|(n, h, p)| serde_json::json!({"name": n, "headline": h, "publicId": p})).collect::<Vec<_>>(),
            "viewers": viewers_at_company.iter().map(|(n, h)| serde_json::json!({"name": n, "headline": h})).collect::<Vec<_>>(),
            "messages": messages_from_company.iter().map(|(n, m, c)| serde_json::json!({"name": n, "lastMessage": m, "conversationId": c})).collect::<Vec<_>>(),
            "people": notable_people.iter().map(|(n, h, d, p)| serde_json::json!({"name": n, "headline": h, "degree": d, "publicId": p})).collect::<Vec<_>>(),
        });
        let pretty =
            serde_json::to_string_pretty(&output).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    // Header.
    println!("{}", company_name);
    let mut meta = Vec::new();
    if !hq.is_empty() {
        if !hq_country.is_empty() {
            meta.push(format!("{}, {}", hq, hq_country));
        } else {
            meta.push(hq.to_string());
        }
    }
    if staff > 0 {
        meta.push(format!("{} employees", staff));
    }
    if !industry.is_empty() {
        meta.push(industry.to_string());
    }
    if !meta.is_empty() {
        println!("  {}", meta.join("  |  "));
    }
    if !tagline.is_empty() {
        println!("  \"{}\"", truncate_with_ellipsis(tagline, 80));
    }
    println!();

    // Connections.
    println!("Your connections there: {}", my_connections.len());
    if !my_connections.is_empty() {
        for (name, headline, pub_id) in &my_connections {
            println!("  {} ({})", name, pub_id);
            println!("    {}", truncate_with_ellipsis(headline, 80));
        }
    }
    println!();

    // Profile viewers.
    if !viewers_at_company.is_empty() {
        println!(
            "Recent profile viewers from {}: {}",
            company_name,
            viewers_at_company.len()
        );
        for (name, headline) in &viewers_at_company {
            println!("  {} — {}", name, headline);
        }
        println!();
    }

    // Messages.
    if !messages_from_company.is_empty() {
        println!("Recent messages:");
        for (name, last_msg, conv_id) in &messages_from_company {
            println!("  {}", name);
            println!("    \"{}\"", truncate_with_ellipsis(last_msg, 80));
            println!("    read: messages read {}", conv_id);
        }
        println!();
    }

    // Notable people (from search).
    if !notable_people.is_empty() {
        println!("People at {} (from search):", company_name);
        for (name, headline, degree, pub_id) in &notable_people {
            println!("  {} {} ({})", name, degree, pub_id);
            println!("    {}", truncate_with_ellipsis(headline, 80));
        }
    } else {
        println!("No people found at {} in search.", company_name);
    }

    Ok(())
}

/// Extract participant names from a conversation element.
fn extract_conversation_names(conv: &serde_json::Value) -> Vec<String> {
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

/// Handle `profile view <public_id> [--json|--summary]`.
///
/// Loads the session, creates a client, calls the identity/profiles endpoint
/// with decoration for full field projection, and prints the result.
async fn cmd_profile_view(public_id: &str, raw_json: bool, summary: bool) -> Result<(), String> {
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

/// Handle `profile audit [--json]`.
///
/// Fetches the authenticated user's full profile and checks for common
/// staleness signals: missing headline, empty about section, stale
/// positions (no current role or current role older than 2 years without
/// updates), missing education, low connection count.
async fn cmd_profile_audit(raw_json: bool) -> Result<(), String> {
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

/// Handle `feed my-posts [--start N] [--count N] [--json]`.
///
/// Fetches the authenticated user's own posts with engagement metrics.
/// Uses `identity/profileUpdatesV2?q=memberShareFeed` which returns
/// standard UpdateV2 records with full social detail.
async fn cmd_feed_my_posts(start: u32, count: u32, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let value = client
        .get_my_posts(start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if let Err(e) = save_feed_cache(&value) {
        eprintln!("warning: failed to cache feed: {}", e);
    }

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let feed: FeedResponse = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse response: {e}"))?;

    if let Some(ref paging) = feed.paging {
        print_paging_header("My posts", paging);
    }
    println!("---");

    if feed.elements.is_empty() {
        println!("(no posts found)");
        return Ok(());
    }

    for (i, element) in feed.elements.iter().enumerate() {
        let idx = start as usize + i + 1;
        print_my_post(idx, element);
        println!();
    }

    Ok(())
}

/// Print a human-readable summary of one of the user's own posts.
///
/// Same UpdateV2 format as general feed items, but we also show view count
/// since it's available for own posts.
fn print_my_post(index: usize, item: &serde_json::Value) {
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    // Commentary text.
    let commentary = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let commentary_display = truncate_with_ellipsis(commentary, 120);

    // Activity URN -- extract from the composite entityUrn.
    // The profileUpdatesV2 response uses MEMBER_SHARES context, so we need to
    // trim at the first comma to get just the activity URN.
    let entity_urn = item.get("entityUrn").and_then(|u| u.as_str()).unwrap_or("");
    let activity_urn = extract_activity_urn(entity_urn)
        .map(|u| u.split(',').next().unwrap_or(&u).to_string())
        .unwrap_or_default();

    // Social counts.
    let social = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"));

    let likes = social
        .and_then(|c| c.get("numLikes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let comments = social
        .and_then(|c| c.get("numComments"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let shares = social
        .and_then(|c| c.get("numShares"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let views = social
        .and_then(|c| c.get("numViews"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    // Timestamp from actor subDescription (e.g. "3d • ").
    let time_desc = update
        .get("actor")
        .and_then(|a| a.get("subDescription"))
        .and_then(|s| s.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .trim_end_matches("• \u{a0}\u{a0}")
        .trim_end_matches("•")
        .trim();

    println!(
        "[{}] {}",
        index,
        if !activity_urn.is_empty() {
            &activity_urn
        } else {
            entity_urn
        }
    );
    if !time_desc.is_empty() {
        println!("    posted: {}", time_desc);
    }
    if !commentary_display.is_empty() {
        println!("    {}", commentary_display);
    }
    println!(
        "    views: {}  reactions: {}  comments: {}  reposts: {}",
        views, likes, comments, shares
    );

    // Reaction type breakdown (if available).
    if let Some(rtc) = social
        .and_then(|c| c.get("reactionTypeCounts"))
        .and_then(|r| r.as_array())
    {
        if !rtc.is_empty() {
            let parts: Vec<String> = rtc
                .iter()
                .filter_map(|r| {
                    let rtype = r.get("reactionType").and_then(|t| t.as_str())?;
                    let count = r.get("count").and_then(|c| c.as_u64())?;
                    Some(format!("{}: {}", reaction_emoji(rtype), count))
                })
                .collect();
            println!("    {}", parts.join("  "));
        }
    }
}

/// Map reaction type strings to compact display labels.
fn reaction_emoji(reaction_type: &str) -> &str {
    match reaction_type {
        "LIKE" => "like",
        "PRAISE" | "CELEBRATION" => "celebrate",
        "EMPATHY" => "love",
        "INTEREST" | "CURIOSITY" => "insightful",
        "APPRECIATION" => "support",
        "ENTERTAINMENT" => "funny",
        _ => reaction_type,
    }
}

/// Handle `feed reactions <post_urn> [--start N] [--count N] [--json]`.
///
/// Fetches and displays the list of people who reacted to a specific post.
async fn cmd_feed_reactions(
    post_urn: Option<&str>,
    from_list: Option<usize>,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let urn = match (post_urn, from_list) {
        (Some(u), None) => normalize_reactions_urn(u),
        (None, Some(index)) => reactions_urn_from_cache(index)?,
        (None, None) => {
            return Err(
                "provide a post URN or use --from-list N after `feed list`/`feed my-posts`"
                    .to_string(),
            )
        }
        (Some(_), Some(_)) => unreachable!("clap conflicts_with guards this"),
    };

    let (client, _path) = load_session_client()?;
    let value = client
        .get_post_reactions(&urn, start, count)
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

    let total = value
        .get("paging")
        .and_then(|p| p.get("total"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    println!("Reactions on {} ({} total)", urn, total);
    println!("---");

    if elements.is_empty() {
        println!("(no reactions)");
        if from_list.is_none() && urn.starts_with("urn:li:activity:") {
            println!();
            println!(
                "Hint: LinkedIn's reactions endpoint uses different URN types \
                 depending on the post backing (ugcPost for most posts, \
                 activity for reshares). If you expected reactions here, try \
                 `feed list` or `feed my-posts` first, then run \
                 `feed reactions --from-list N` — the CLI will pick the \
                 right URN. See re/reactions.md."
            );
        }
        return Ok(());
    }

    for element in &elements {
        let lockup = element.get("reactorLockup").unwrap_or(element);
        let name = lockup
            .get("title")
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("(unknown)");
        let subtitle = lockup
            .get("subtitle")
            .and_then(|s| s.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        let rtype = element
            .get("reactionType")
            .and_then(|r| r.as_str())
            .unwrap_or("?");

        let subtitle_display = truncate_with_ellipsis(subtitle, 60);

        println!(
            "  {:12} {} {}",
            reaction_emoji(rtype),
            name,
            if subtitle_display.is_empty() {
                String::new()
            } else {
                format!("- {}", subtitle_display)
            }
        );
    }

    Ok(())
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
/// Aggregates engagement across the user's last 20 posts by summing the
/// per-post counts embedded in `socialDetail.totalSocialActivityCounts` on
/// the `identity/profileUpdatesV2?q=memberShareFeed` response.
///
/// The legacy `identity/socialUpdateAnalytics{,Header}` endpoints return
/// HTTP 400 and are not used -- see `re/my_posts.md` for details.
async fn cmd_feed_stats(raw_json: bool) -> Result<(), String> {
    const POST_COUNT: u32 = 20;

    let (client, _path) = load_session_client()?;

    let value = client
        .get_my_posts(0, POST_COUNT)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let posts: Vec<PostStat> = elements.iter().map(extract_post_stat).collect();
    let totals = PostStat::sum(&posts);
    let n = posts.len() as u64;

    if raw_json {
        let averages = match (
            totals.views.checked_div(n),
            totals.likes.checked_div(n),
            totals.comments.checked_div(n),
            totals.shares.checked_div(n),
        ) {
            (Some(v), Some(l), Some(c), Some(s)) => serde_json::json!({
                "views": v,
                "likes": l,
                "comments": c,
                "shares": s,
            }),
            _ => serde_json::json!({}),
        };
        let out = serde_json::json!({
            "post_count": n,
            "totals": {
                "views": totals.views,
                "likes": totals.likes,
                "comments": totals.comments,
                "shares": totals.shares,
            },
            "averages": averages,
            "posts": posts.iter().map(PostStat::to_json).collect::<Vec<_>>(),
        });
        let pretty =
            serde_json::to_string_pretty(&out).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    if n == 0 {
        println!("(no posts found)");
        return Ok(());
    }

    println!("Engagement across last {} posts", n);
    println!("---");
    println!("Totals:");
    println!("  views:    {}", totals.views);
    println!("  likes:    {}", totals.likes);
    println!("  comments: {}", totals.comments);
    println!("  shares:   {}", totals.shares);
    println!();
    println!("Averages per post:");
    println!("  views:    {}", totals.views / n);
    println!("  likes:    {}", totals.likes / n);
    println!("  comments: {}", totals.comments / n);
    println!("  shares:   {}", totals.shares / n);
    println!();

    // Show top 5 posts by views as context.
    let mut ranked = posts.clone();
    ranked.sort_by_key(|p| std::cmp::Reverse(p.views));
    let top = ranked.iter().take(5);
    println!("Top posts by views:");
    for (i, p) in top.enumerate() {
        println!(
            "[{}] {} views, {} likes, {} comments, {} shares",
            i + 1,
            p.views,
            p.likes,
            p.comments,
            p.shares
        );
        if !p.preview.is_empty() {
            println!("    {}", truncate_with_ellipsis(&p.preview, 100));
        }
    }

    Ok(())
}

#[derive(Clone)]
struct PostStat {
    views: u64,
    likes: u64,
    comments: u64,
    shares: u64,
    preview: String,
}

impl PostStat {
    fn sum(posts: &[PostStat]) -> PostStat {
        posts.iter().fold(
            PostStat {
                views: 0,
                likes: 0,
                comments: 0,
                shares: 0,
                preview: String::new(),
            },
            |mut acc, p| {
                acc.views += p.views;
                acc.likes += p.likes;
                acc.comments += p.comments;
                acc.shares += p.shares;
                acc
            },
        )
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "views": self.views,
            "likes": self.likes,
            "comments": self.comments,
            "shares": self.shares,
            "preview": self.preview,
        })
    }
}

fn extract_post_stat(item: &serde_json::Value) -> PostStat {
    let update = item
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(item);

    let counts = update
        .get("socialDetail")
        .and_then(|s| s.get("totalSocialActivityCounts"));

    let get_u64 = |key: &str| -> u64 {
        counts
            .and_then(|c| c.get(key))
            .and_then(|n| n.as_u64())
            .unwrap_or(0)
    };

    let preview = update
        .get("commentary")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    PostStat {
        views: get_u64("numViews"),
        likes: get_u64("numLikes"),
        comments: get_u64("numComments"),
        shares: get_u64("numShares"),
        preview,
    }
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

/// Handle `connections list [--count N] [--start N] [--json]`.
///
/// Loads the session, calls GET /voyager/api/relationships/connections with
/// pagination params sorted by RECENTLY_ADDED, and prints the results.
async fn cmd_connections_list(
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
async fn cmd_connections_list_all(
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
async fn cmd_connections_invite_batch(
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

async fn cmd_connections_withdraw(
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

    // Cache results for index-based profile view.
    if let Err(e) = save_search_cache("people", &value) {
        eprintln!("warning: failed to cache search results: {e}");
    }

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

    save_notifications_cache(&value)?;

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

/// Handle `notifications mentions <index> [--json]`.
///
/// Indexes into the most recent `notifications list` cache, extracts the
/// underlying activity URN from `cardAction.actionTarget`, fetches the post
/// body, and walks the commentary's `attributesV2` array for entries whose
/// type carries an `fsd_profile` URN. The shape mirrors LinkedIn's Pemberly
/// attributed-text format used elsewhere in the API: each attribute has a
/// `start`, `length`, and a discriminator under `type` containing the
/// mentioned member's URN. The exact discriminator key is unverified
/// against a captured response, so the handler walks all entries
/// permissively and surfaces any URN it finds.
async fn cmd_notifications_mentions(index: usize, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let cache = load_notifications_cache()?;
    let elements = cache
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "cached notifications response has no elements array".to_string())?;
    let card = elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (cached notifications has {} items)",
            index,
            elements.len()
        )
    })?;

    let action_target = card
        .get("cardAction")
        .and_then(|a| a.get("actionTarget"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let activity_urn = extract_activity_urn_from_url(action_target).ok_or_else(|| {
        format!(
            "notification {} has no activity URN in cardAction.actionTarget ({})",
            index, action_target
        )
    })?;

    let (client, _path) = load_session_client()?;
    let post = client
        .get_post(&activity_urn)
        .await
        .map_err(|e| format!("failed to fetch post {}: {e}", activity_urn))?;

    let mentions = collect_post_mentions(&post);

    if raw_json {
        let payload = serde_json::json!({
            "activityUrn": activity_urn,
            "mentions": mentions,
        });
        let pretty = serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    if mentions.is_empty() {
        println!("(no mentions found in {})", activity_urn);
        return Ok(());
    }

    println!("Mentions in {}:", activity_urn);
    for m in &mentions {
        let urn = m.get("urn").and_then(|u| u.as_str()).unwrap_or("(no urn)");
        let name = m.get("text").and_then(|t| t.as_str()).unwrap_or("");
        if name.is_empty() {
            println!("  {}", urn);
        } else {
            println!("  {}  ({})", name, urn);
        }
    }

    Ok(())
}

/// Pull the activity URN out of a notification cardAction URL like
/// `/feed/update/urn:li:activity:7312345/?...` (URL- or percent-encoded).
fn extract_activity_urn_from_url(url: &str) -> Option<String> {
    let decoded = url.replace("%3A", ":").replace("%2F", "/");
    let prefix = "urn:li:activity:";
    let start = decoded.find(prefix)?;
    let tail = &decoded[start..];
    let end = tail
        .find(|c: char| !c.is_ascii_digit() && c != ':' && !c.is_ascii_alphabetic())
        .unwrap_or(tail.len());
    let urn = &tail[..end];
    if urn.len() > prefix.len() {
        Some(urn.to_string())
    } else {
        None
    }
}

/// Walk a post's commentary attributes and pull any fsd_profile URNs out
/// of the structured mention data. The Pemberly attributed-text format
/// puts mentions under `commentary.text.attributesV2[].type.*.urn`; we
/// scan permissively because the discriminator key (e.g.
/// `com.linkedin.pemberly.text.Profile`, `MEMBER_MENTION`) varies by
/// payload version. Returns one JSON object per mention with `urn` and
/// optional `text` (the @mention literal as it appears in the post).
fn collect_post_mentions(post: &serde_json::Value) -> Vec<serde_json::Value> {
    let update = post
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(post);

    let commentary = match update.get("commentary").and_then(|c| c.get("text")) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let body_text = commentary
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let attrs = commentary
        .get("attributesV2")
        .or_else(|| commentary.get("attributes"))
        .and_then(|a| a.as_array());
    let attrs = match attrs {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    for attr in attrs {
        // Walk every leaf string under this attribute looking for an
        // fsd_profile URN. The exact key path varies; this catches:
        //   type.com.linkedin.pemberly.text.Profile.urn
        //   type.MEMBER_MENTION.profileUrn
        //   miniProfile.entityUrn
        let urn = find_fsd_profile_urn(attr);
        if let Some(urn) = urn {
            // Try to splice the mention text out of the body using start+length.
            let start = attr.get("start").and_then(|s| s.as_u64()).unwrap_or(0) as usize;
            let length = attr.get("length").and_then(|l| l.as_u64()).unwrap_or(0) as usize;
            let snippet = body_text
                .get(start..start.saturating_add(length))
                .unwrap_or("");
            out.push(serde_json::json!({
                "urn": urn,
                "text": snippet,
            }));
        }
    }
    out
}

/// Recursively search a JSON value for a string that starts with
/// `urn:li:fsd_profile:`. Returns the first match.
fn find_fsd_profile_urn(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) if s.starts_with("urn:li:fsd_profile:") => Some(s.clone()),
        serde_json::Value::Object(map) => map.values().find_map(find_fsd_profile_urn),
        serde_json::Value::Array(arr) => arr.iter().find_map(find_fsd_profile_urn),
        _ => None,
    }
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

    // kicker.text -- time indicator (e.g. "2h ago"). Currently unused since
    // publishedAt provides a more precise timestamp.
    let _kicker = card
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

    // contentPrimaryText -- the actual post body for reaction/mention notifications.
    let content_text = card
        .get("contentPrimaryText")
        .and_then(|arr| arr.as_array())
        .and_then(|a| a.first())
        .and_then(|t| t.get("text").and_then(|v| v.as_str()))
        .unwrap_or("");

    // socialActivityCounts -- engagement numbers.
    let social = card.get("socialActivityCounts");
    let num_likes = social
        .and_then(|s| s.get("numLikes").and_then(|n| n.as_u64()))
        .unwrap_or(0);
    let num_comments = social
        .and_then(|s| s.get("numComments").and_then(|n| n.as_u64()))
        .unwrap_or(0);

    // Post URL from cardAction.actionTarget.
    let post_url = card
        .get("cardAction")
        .and_then(|a| a.get("actionTarget").and_then(|t| t.as_str()))
        .unwrap_or("");

    // Truncate long headlines for summary view.
    let headline_display = truncate_with_ellipsis(headline, 120);

    print!("[{}]{} {}", index, unread_marker, headline_display);
    println!();

    // Show post preview for content notifications.
    if !content_text.is_empty() {
        // Show first line of the post as a preview.
        let first_line = content_text.lines().next().unwrap_or("");
        let preview = truncate_with_ellipsis(first_line, 100);
        println!("    \"{}\"", preview);
    }

    if !sub_headline.is_empty() {
        println!("    {}", sub_headline);
    }

    // Engagement stats + timestamp on one line.
    let mut meta_parts = Vec::new();
    if num_likes > 0 || num_comments > 0 {
        meta_parts.push(format!("likes: {}  comments: {}", num_likes, num_comments));
    }
    if !content_type.is_empty() {
        meta_parts.push(format!("type: {}", content_type));
    }
    if !published_at.is_empty() {
        meta_parts.push(published_at);
    }
    if !meta_parts.is_empty() {
        println!("    {}", meta_parts.join("  |  "));
    }

    // Post link.
    if !post_url.is_empty() && post_url.contains("/feed/") {
        println!(
            "    https://www.linkedin.com{}",
            post_url.replace("%3A", ":").replace("%2F", "/")
        );
    }
}

/// Load the stored session or return a descriptive error.
///
/// Checks for session validity and prints a warning to stderr if the
/// session is old enough to be potentially expired.
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

    // Warn about potentially expired sessions.
    if let Some(warning) = session.expiry_warning() {
        eprintln!("warning: {}", warning);
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
    let sender_name = sender.and_then(|s| {
        // Try member.firstName + lastName via participantType path.
        s.get("participantType")
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
                    rc.get("externalMedia").and_then(|em| {
                        let title = em.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let url = em.get("url").and_then(|v| v.as_str()).unwrap_or("");
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
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
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
        .map_err(|_| "no cached feed. Run `feed list` or `feed my-posts` first.".to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse feed cache: {e}"))
}

fn notifications_cache_path() -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir.join("linkedin").join("last_notifications.json"))
}

fn save_notifications_cache(value: &serde_json::Value) -> Result<(), String> {
    let path = notifications_cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json = serde_json::to_string(value)
        .map_err(|e| format!("failed to serialize notifications cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write notifications cache: {e}"))?;
    Ok(())
}

fn load_notifications_cache() -> Result<serde_json::Value, String> {
    let path = notifications_cache_path()?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| "no cached notifications. Run `notifications list` first.".to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse notifications cache: {e}"))
}

/// Normalize a user-supplied reactions URN. Accepts full URNs (any type) or a
/// bare activity ID (digits only), which is wrapped as `urn:li:activity:...`.
fn normalize_reactions_urn(input: &str) -> String {
    if input.starts_with("urn:li:") {
        input.to_string()
    } else {
        format!("urn:li:activity:{}", input)
    }
}

/// Extract the right thread URN for the reactions endpoint from a cached feed
/// element. LinkedIn's `socialDashReactionsByReactionType` is URN-type-picky:
/// ugcPost-backed posts require the ugcPost URN, share-backed posts require
/// the activity URN. `updateMetadata.shareUrn` carries the underlying object
/// URN, so we use that when it's a `ugcPost:`; otherwise we fall back to the
/// activity URN. See `re/reactions.md` for the investigation.
fn extract_reactions_urn(element: &serde_json::Value) -> Option<String> {
    let update = element
        .get("value")
        .and_then(|v| v.get("com.linkedin.voyager.feed.render.UpdateV2"))
        .unwrap_or(element);
    let metadata = update.get("updateMetadata")?;
    let activity_urn = metadata.get("urn").and_then(|u| u.as_str());
    let share_urn = metadata.get("shareUrn").and_then(|u| u.as_str());

    match (share_urn, activity_urn) {
        (Some(s), _) if s.starts_with("urn:li:ugcPost:") => Some(s.to_string()),
        (_, Some(a)) => Some(a.to_string()),
        (Some(s), None) => Some(s.to_string()),
        (None, None) => None,
    }
}

/// Resolve a 1-based index from the last cached feed listing into a reactions
/// thread URN.
fn reactions_urn_from_cache(index: usize) -> Result<String, String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }
    let cache = load_feed_cache()?;
    let elements = cache
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "cached feed has no elements array".to_string())?;
    let element = elements.get(index - 1).ok_or_else(|| {
        format!(
            "index {} out of range (cached feed has {} items)",
            index,
            elements.len()
        )
    })?;
    extract_reactions_urn(element).ok_or_else(|| {
        format!(
            "could not extract a reactions URN from cached item {}",
            index
        )
    })
}

// ---------------------------------------------------------------------------
// Search cache utilities
// ---------------------------------------------------------------------------

fn search_cache_path(kind: &str) -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir
        .join("linkedin")
        .join(format!("last_search_{}.json", kind)))
}

fn save_search_cache(kind: &str, value: &serde_json::Value) -> Result<(), String> {
    let path = search_cache_path(kind)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json = serde_json::to_string(value)
        .map_err(|e| format!("failed to serialize search cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write search cache: {e}"))?;
    Ok(())
}

fn load_search_cache(kind: &str) -> Result<serde_json::Value, String> {
    let path = search_cache_path(kind)?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| format!("no cached {} search. Run `search {}` first.", kind, kind))?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse search cache: {e}"))
}

/// Extract the Nth post's activity URN from cached search results.
fn resolve_search_post_urn(index: usize) -> Result<String, String> {
    let cache = load_search_cache("posts")?;
    let resp: SearchResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached search: {e}"))?;

    let mut result_idx = 0usize;
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
                if result_idx == index {
                    let update = sfu.get("update").unwrap_or(sfu);
                    return update
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
                        .ok_or_else(|| format!("search result {} has no activity URN", index));
                }
            }
        }
    }

    Err(format!(
        "index {} out of range (search has {} results)",
        index, result_idx
    ))
}

/// Extract the Nth person's profile slug from cached people search results.
fn resolve_search_person_slug(index: usize) -> Result<String, String> {
    let cache = load_search_cache("people")?;
    let resp: SearchResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached search: {e}"))?;

    let mut result_idx = 0usize;
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
                if result_idx == index {
                    return entity
                        .get("navigationUrl")
                        .and_then(|v| v.as_str())
                        .and_then(|url| {
                            url.strip_prefix("https://www.linkedin.com/in/")
                                .map(|rest| rest.split('?').next().unwrap_or(rest).to_string())
                        })
                        .ok_or_else(|| format!("search result {} has no profile URL", index));
                }
            }
        }
    }

    Err(format!(
        "index {} out of range (search has {} results)",
        index, result_idx
    ))
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

/// Handle `feed view <activity_urn> [--json]`.
///
/// Fetches a single post by activity URN from the API and displays it.
async fn cmd_feed_view(activity_urn: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let post = client
        .get_post(activity_urn)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&post).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    print_feed_item_full(0, &post);
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
    let permalink = item.get("permalink").and_then(|u| u.as_str()).unwrap_or("");
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
        .or_else(|| update.get("content").and_then(|c| c.get("resharedUpdate")))
        .or_else(|| {
            // Also check inside the UpdateV2 union value
            update
                .get("content")
                .and_then(|c| c.get("com.linkedin.voyager.feed.render.UpdateV2"))
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
        .or_else(|| content.get("com.linkedin.voyager.feed.render.ArticleComponent"))
    {
        let title = article
            .get("title")
            .and_then(|t| {
                t.get("text")
                    .and_then(|v| v.as_str())
                    .or_else(|| t.as_str())
            })
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
        (
            "com.linkedin.voyager.feed.render.LinkedInVideoComponent",
            "video",
        ),
        (
            "com.linkedin.voyager.feed.render.DocumentComponent",
            "document",
        ),
        ("com.linkedin.voyager.feed.render.PollComponent", "poll"),
        (
            "com.linkedin.voyager.feed.render.ArticleComponent",
            "article",
        ),
        (
            "com.linkedin.voyager.feed.render.CelebrationComponent",
            "celebration",
        ),
        (
            "com.linkedin.voyager.feed.render.CarouselComponent",
            "carousel",
        ),
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
        if let Some(poster) = vid.get("thumbnail").and_then(|t| {
            t.get("url")
                .and_then(|v| v.as_str())
                .or_else(|| t.get("rootUrl").and_then(|v| v.as_str()))
        }) {
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
    let feed: FeedResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached feed: {e}"))?;

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
                c.get("text").and_then(|t| {
                    // Could be a string directly or a nested {text: "..."} object
                    t.as_str()
                        .or_else(|| t.get("text").and_then(|v| v.as_str()))
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

    // Cache results for index-based react/comment.
    if let Err(e) = save_search_cache("posts", &value) {
        eprintln!("warning: failed to cache search results: {e}");
    }

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

/// Handle `search react <index> [--type LIKE] [--json]`.
///
/// Reacts to a post from cached search results.
async fn cmd_search_react(index: usize, reaction_type: &str, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let activity_urn = resolve_search_post_urn(index)?;
    let rt_upper = reaction_type.to_uppercase();
    let (client, _path) = load_session_client()?;

    eprintln!("Reacting to {} with {}...", activity_urn, rt_upper);
    let result = client
        .react_to_post(&activity_urn, &rt_upper)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&result).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
    } else {
        println!("Reacted with {} to search result [{}]", rt_upper, index);
    }

    Ok(())
}

/// Handle `search view <index> [--json]`.
///
/// Views a profile from cached people search results.
async fn cmd_search_view(index: usize, raw_json: bool) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let slug = resolve_search_person_slug(index)?;
    eprintln!("Loading profile '{}'...", slug);
    cmd_profile_view(&slug, raw_json, false).await
}

/// Handle `search invite <index> [--message ...] [--json]`.
///
/// Sends a connection invitation to the Nth person in the most recent
/// `search people` results. Pulls the entity URN out of the cached search
/// response so the request never has to resolve a slug — bypasses the
/// flaky GraphQL profile resolver entirely. Falls back to slug-based
/// invite if the cached entry doesn't carry a usable URN (older cache
/// shapes, or LinkedIn omitting the field for a particular result).
async fn cmd_search_invite(
    index: usize,
    message: Option<&str>,
    raw_json: bool,
) -> Result<(), String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }

    let target = resolve_search_person_target(index)?;
    eprintln!(
        "Inviting {} via cached search result #{}",
        target.label(),
        index
    );
    cmd_connections_invite(&target.invite_arg(), message, raw_json).await
}

/// Resolved target for a cached search result.
///
/// Carries both the URN (preferred for invites) and the slug (fallback +
/// human-readable label). One of these is always present; the other may
/// be absent depending on the cached payload shape.
struct SearchPersonTarget {
    urn: Option<String>,
    slug: Option<String>,
    name: Option<String>,
}

impl SearchPersonTarget {
    /// Pick the best argument to pass to `cmd_connections_invite`. Prefers
    /// URN (skips the resolver) but falls through to slug when needed.
    fn invite_arg(&self) -> String {
        self.urn
            .clone()
            .or_else(|| self.slug.clone())
            .expect("SearchPersonTarget must have either urn or slug")
    }

    fn label(&self) -> String {
        match (&self.name, &self.slug) {
            (Some(n), Some(s)) => format!("{} ({})", n, s),
            (Some(n), None) => n.clone(),
            (None, Some(s)) => s.clone(),
            (None, None) => self.urn.clone().unwrap_or_default(),
        }
    }
}

/// Resolve a target struct from the cached people-search response by index.
///
/// The cache mirrors the GraphQL `searchDashClustersByAll` envelope:
/// each cluster has an `items` array; entries with an `item.entityResult`
/// represent a person hit. Sponsored, query-clarification, and feedback
/// cards are skipped so the index lines up with what the user sees in the
/// printed list.
fn resolve_search_person_target(index: usize) -> Result<SearchPersonTarget, String> {
    let cache = load_search_cache("people")?;
    let resp: SearchResponse =
        serde_json::from_value(cache).map_err(|e| format!("failed to parse cached search: {e}"))?;

    let mut result_idx = 0usize;
    for cluster in &resp.elements {
        let items = cluster
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            let entity = match item_wrapper.get("item").and_then(|i| i.get("entityResult")) {
                Some(e) => e,
                None => continue,
            };
            result_idx += 1;
            if result_idx != index {
                continue;
            }

            let urn = entity
                .get("entityUrn")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("urn:li:fsd_profile:"))
                .map(|s| s.to_string())
                .or_else(|| {
                    entity
                        .get("trackingUrn")
                        .and_then(|v| v.as_str())
                        .filter(|s| s.starts_with("urn:li:fsd_profile:"))
                        .map(|s| s.to_string())
                });

            let slug = entity
                .get("navigationUrl")
                .and_then(|v| v.as_str())
                .and_then(|url| {
                    url.strip_prefix("https://www.linkedin.com/in/")
                        .map(|rest| rest.split('?').next().unwrap_or(rest).to_string())
                });

            let name = entity
                .get("title")
                .and_then(|t| t.get("text"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if urn.is_none() && slug.is_none() {
                return Err(format!(
                    "search result {} has neither entityUrn nor navigationUrl",
                    index
                ));
            }

            return Ok(SearchPersonTarget { urn, slug, name });
        }
    }

    Err(format!(
        "index {} out of range (search has {} results)",
        index, result_idx
    ))
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
                .and_then(extract_activity_urn)
        })
        .or_else(|| {
            update
                .get("socialDetail")
                .and_then(|sd| sd.get("entityUrn"))
                .and_then(|u| u.as_str())
                .and_then(extract_activity_urn)
        })
        .unwrap_or_default();

    let permalink = sfu.get("permalink").and_then(|p| p.as_str()).unwrap_or("");

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

// ── Events commands ─────────────────────────────────────────────────────────

async fn cmd_event_view(event_id: &str, raw_json: bool) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let event = client
        .get_event(event_id)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&event).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let elements = event
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let ev = elements.first().unwrap_or(&event);

    let name = ev
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)");
    let time = ev
        .get("displayEventTime")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let state = ev
        .get("lifecycleState")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let external_url = ev.get("externalUrl").and_then(|v| v.as_str()).unwrap_or("");
    let vanity = ev.get("vanityName").and_then(|v| v.as_str()).unwrap_or("");
    let host = ev
        .get("viewerHost")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    println!("{}", name);
    println!("{}", "=".repeat(name.len()));
    println!("When: {}", time);
    println!("Status: {}", state);
    if host {
        println!("Role: Host");
    }
    if !external_url.is_empty() {
        println!("Link: {}", external_url);
    }
    if !vanity.is_empty() {
        println!("URL: https://www.linkedin.com/events/{}/", vanity);
    }

    // Speakers.
    if let Some(speakers) = ev.get("speakers").and_then(|s| s.as_array()) {
        if !speakers.is_empty() {
            println!("\nSpeakers:");
            for speaker in speakers {
                let first = speaker
                    .pointer("/assigneeProfile/firstName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let last = speaker
                    .pointer("/assigneeProfile/lastName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let state = speaker.get("state").and_then(|v| v.as_str()).unwrap_or("");
                println!("  - {} {} ({})", first, last, state.to_lowercase());
            }
        }
    }

    Ok(())
}

async fn cmd_event_attendees(
    event_id: &str,
    start: u32,
    count: u32,
    raw_json: bool,
) -> Result<(), String> {
    let (client, _path) = load_session_client()?;

    let resp = client
        .get_event_attendees(event_id, start, count)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    if raw_json {
        let pretty =
            serde_json::to_string_pretty(&resp).map_err(|e| format!("JSON format error: {e}"))?;
        println!("{}", pretty);
        return Ok(());
    }

    let elements = resp
        .get("elements")
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let total = resp
        .pointer("/paging/total")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut idx = start;
    let mut printed = 0u32;

    for cluster in elements {
        let items = cluster
            .get("items")
            .and_then(|i| i.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for item_wrapper in items {
            // REST.li search uses `itemUnion.entityResult`.
            let entity = item_wrapper
                .get("itemUnion")
                .and_then(|iu| iu.get("entityResult"))
                .or_else(|| item_wrapper.get("item").and_then(|i| i.get("entityResult")));
            if let Some(entity) = entity {
                idx += 1;
                printed += 1;
                let name = entity
                    .pointer("/title/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let headline = entity
                    .pointer("/primarySubtitle/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                println!("{:3}. {} — {}", idx, name, headline);
            }
        }
    }

    if printed == 0 {
        match client.get_event(event_id).await {
            Ok(_) => {
                eprintln!("Warning: the event exists but no attendees were returned.");
                eprintln!(
                    "This usually means your LinkedIn session has expired. \
                     Browse LinkedIn in Chrome to refresh cookies, then run \
                     `li auth login` to update the session."
                );
            }
            Err(_) => {
                println!("(no attendees found)");
            }
        }
    } else {
        println!("---");
        println!(
            "Showing {}-{} of {} attendees",
            start + 1,
            start + printed,
            total
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_reactions_urn_passes_full_urn_through() {
        assert_eq!(
            normalize_reactions_urn("urn:li:activity:7450808005048094720"),
            "urn:li:activity:7450808005048094720"
        );
        assert_eq!(
            normalize_reactions_urn("urn:li:ugcPost:7450808001881534464"),
            "urn:li:ugcPost:7450808001881534464"
        );
    }

    #[test]
    fn extract_activity_urn_from_url_handles_plain_path() {
        assert_eq!(
            extract_activity_urn_from_url("/feed/update/urn:li:activity:7312345/"),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_from_url_handles_percent_encoded() {
        assert_eq!(
            extract_activity_urn_from_url(
                "/feed/update/urn%3Ali%3Aactivity%3A7312345/?trackingId=foo"
            ),
            Some("urn:li:activity:7312345".to_string())
        );
    }

    #[test]
    fn extract_activity_urn_from_url_returns_none_for_unrelated() {
        assert_eq!(extract_activity_urn_from_url("/in/some-profile/"), None);
    }

    #[test]
    fn collect_post_mentions_walks_attributesv2_for_fsd_profile_urns() {
        let post = json!({
            "commentary": {
                "text": {
                    "text": "Great work @Alice and @Bob!",
                    "attributesV2": [
                        {
                            "start": 11,
                            "length": 6,
                            "type": {
                                "com.linkedin.pemberly.text.Profile": {
                                    "urn": "urn:li:fsd_profile:ACoAAAAAAAAAAA"
                                }
                            }
                        },
                        {
                            "start": 22,
                            "length": 4,
                            "type": {
                                "com.linkedin.pemberly.text.Profile": {
                                    "urn": "urn:li:fsd_profile:ACoAAABBBBBBBB"
                                }
                            }
                        }
                    ]
                }
            }
        });
        let mentions = collect_post_mentions(&post);
        assert_eq!(mentions.len(), 2);
        assert_eq!(
            mentions[0].get("urn").and_then(|v| v.as_str()),
            Some("urn:li:fsd_profile:ACoAAAAAAAAAAA")
        );
        assert_eq!(
            mentions[0].get("text").and_then(|v| v.as_str()),
            Some("@Alice")
        );
        assert_eq!(
            mentions[1].get("text").and_then(|v| v.as_str()),
            Some("@Bob")
        );
    }

    #[test]
    fn collect_post_mentions_returns_empty_when_no_attributes() {
        let post = json!({
            "commentary": { "text": { "text": "Plain post, no mentions" } }
        });
        assert!(collect_post_mentions(&post).is_empty());
    }

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
    fn hint_for_401_suggests_relogin() {
        let hint = error_hint("API call failed: API error (HTTP 401): session expired");
        assert!(hint.unwrap().contains("li auth login"));
    }

    #[test]
    fn hint_for_301_suggests_re_doc_check() {
        let hint = error_hint("API call failed: API error (HTTP 301): {\"status\":301}");
        assert!(hint.unwrap().contains("retired"));
    }

    #[test]
    fn hint_for_429_suggests_backoff() {
        let hint = error_hint("API call failed: API error (HTTP 429): too many requests");
        assert!(hint.unwrap().contains("rate limited"));
    }

    #[test]
    fn hint_for_resolver_failure_suggests_alternatives() {
        let hint =
            error_hint("failed to resolve profile: API error (HTTP 200): GraphQL errors: ...");
        assert!(hint.unwrap().contains("URN") || hint.unwrap().contains("search invite"));
    }

    #[test]
    fn hint_for_index_out_of_range_suggests_repopulating_cache() {
        let hint = error_hint("index 5 out of range (search has 3 results)");
        assert!(hint.unwrap().contains("cache"));
    }

    #[test]
    fn hint_for_unknown_error_returns_none() {
        assert!(error_hint("some random unrecognised error").is_none());
    }

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

    #[test]
    fn search_target_prefers_entity_urn() {
        let target = SearchPersonTarget {
            urn: Some("urn:li:fsd_profile:ACoAAA111".to_string()),
            slug: Some("jane-doe".to_string()),
            name: Some("Jane Doe".to_string()),
        };
        assert_eq!(target.invite_arg(), "urn:li:fsd_profile:ACoAAA111");
        assert_eq!(target.label(), "Jane Doe (jane-doe)");
    }

    #[test]
    fn search_target_falls_back_to_slug() {
        let target = SearchPersonTarget {
            urn: None,
            slug: Some("jane-doe".to_string()),
            name: Some("Jane Doe".to_string()),
        };
        assert_eq!(target.invite_arg(), "jane-doe");
    }

    #[test]
    fn search_target_label_with_only_slug() {
        let target = SearchPersonTarget {
            urn: None,
            slug: Some("jane-doe".to_string()),
            name: None,
        };
        assert_eq!(target.label(), "jane-doe");
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

    #[test]
    fn normalize_reactions_urn_wraps_bare_id_as_activity() {
        assert_eq!(
            normalize_reactions_urn("7450808005048094720"),
            "urn:li:activity:7450808005048094720"
        );
    }

    #[test]
    fn extract_reactions_urn_prefers_ugcpost_sharureurn() {
        // UGC-backed post (e.g. an own text post): reactions endpoint
        // requires the ugcPost URN, not the activity URN.
        let element = json!({
            "value": {
                "com.linkedin.voyager.feed.render.UpdateV2": {
                    "updateMetadata": {
                        "urn": "urn:li:activity:7450891298279972864",
                        "shareUrn": "urn:li:ugcPost:7450888187100639232"
                    }
                }
            }
        });
        assert_eq!(
            extract_reactions_urn(&element),
            Some("urn:li:ugcPost:7450888187100639232".to_string())
        );
    }

    #[test]
    fn extract_reactions_urn_uses_activity_for_share_backed() {
        // Share-backed post (reshare): reactions endpoint requires the
        // activity URN; passing the share URN silently returns total=0.
        let element = json!({
            "value": {
                "com.linkedin.voyager.feed.render.UpdateV2": {
                    "updateMetadata": {
                        "urn": "urn:li:activity:7450895500045598720",
                        "shareUrn": "urn:li:share:7450895499496202240"
                    }
                }
            }
        });
        assert_eq!(
            extract_reactions_urn(&element),
            Some("urn:li:activity:7450895500045598720".to_string())
        );
    }

    #[test]
    fn extract_reactions_urn_handles_unwrapped_update_element() {
        // `feed my-posts` returns elements without the `value.UpdateV2`
        // wrapper — the updateMetadata sits at the root.
        let element = json!({
            "updateMetadata": {
                "urn": "urn:li:activity:7450808005048094720",
                "shareUrn": "urn:li:ugcPost:7450808001881534464"
            }
        });
        assert_eq!(
            extract_reactions_urn(&element),
            Some("urn:li:ugcPost:7450808001881534464".to_string())
        );
    }

    #[test]
    fn extract_reactions_urn_returns_none_when_metadata_missing() {
        let element = json!({"foo": "bar"});
        assert_eq!(extract_reactions_urn(&element), None);
    }
}
