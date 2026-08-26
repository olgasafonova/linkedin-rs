use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "li")]
#[command(about = "LinkedIn in the terminal", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
pub enum AuthAction {
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
pub enum FeedAction {
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
    ///
    /// WARNING: This places a REAL reaction visible to the post's author
    /// and their network. Use --yes to skip the confirmation prompt.
    React {
        /// Post/activity URN or 1-based index from last `feed list`
        post_urn: String,

        /// Reaction type: LIKE, PRAISE, EMPATHY, INTEREST, APPRECIATION, ENTERTAINMENT, CELEBRATION
        #[arg(long = "type", default_value = "LIKE")]
        reaction_type: String,

        /// Skip confirmation prompt (required for non-interactive use)
        #[arg(long)]
        yes: bool,

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

        /// Comment as a company page you administer: numeric organization
        /// ID or full urn:li:organization:<id> URN
        #[arg(long = "as-org")]
        as_org: Option<String>,

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
pub enum ProfileAction {
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
    /// List recent posts by a member (vanity slug). Drops most-recent-first.
    ///
    /// Time-window filtering is NOT done here; the CLI returns up to `--count`
    /// posts and exposes a relative-time label (e.g., "2d") plus the activity
    /// URN. Date arithmetic is the caller's job.
    Posts {
        /// LinkedIn public identifier (vanity URL slug, e.g. john-doe-123)
        public_id: String,

        /// Number of posts to fetch (default: 20; LinkedIn caps the page size).
        #[arg(long, default_value = "20")]
        count: u32,

        /// Also fetch comments per post and surface URLs from the author's own
        /// first comment under `firstCommentByAuthor`. Doubles the API calls,
        /// so opt-in.
        #[arg(long)]
        with_first_comment: bool,

        /// Output raw JSON instead of a human-readable summary.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ConnectionsAction {
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
pub enum SearchAction {
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
pub enum NotificationsAction {
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
pub enum EventsAction {
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
pub enum CompanyAction {
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
pub enum MessagesAction {
    /// List conversations
    List {
        /// Number of conversations to fetch (default: 10)
        #[arg(long, default_value = "10")]
        count: u32,

        /// Cursor for pagination: epoch-millis timestamp to fetch conversations created before
        #[arg(long)]
        before: Option<u64>,

        /// Inbox category to read: "primary" (default) or "spam"
        #[arg(long, default_value = "primary")]
        category: String,

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
    ///
    /// WARNING: This sends a REAL DIRECT MESSAGE that the recipient will
    /// see in their LinkedIn inbox. Use --yes to skip the confirmation prompt.
    Send {
        /// LinkedIn public identifier (vanity URL slug, e.g. john-doe-123)
        recipient: String,

        /// Message text to send
        message: String,

        /// Skip confirmation prompt (required for non-interactive use)
        #[arg(long)]
        yes: bool,

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
