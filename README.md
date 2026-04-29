# LinkedIn Reversed

Rust CLI and API library for LinkedIn, reverse-engineered from the Android app (`com.linkedin.android`). Provides programmatic access to LinkedIn's core features: profiles, messaging, feed, connections, search, and notifications.

This project is for personal and educational use only.

## Features

The CLI (`linkedin-cli`) exposes 44 subcommands across 11 domains:

| Command | Description |
|---------|-------------|
| **Auth** | |
| `auth login` | Authenticate with a `li_at` cookie (from browser or env var) |
| `auth status` | Check session validity (live API call or local-only) |
| `auth logout` | Clear stored session |
| **Profile** | |
| `profile me` | Fetch your own profile |
| `profile view <id>` | View a profile by public identifier (vanity URL slug) |
| `profile visit <id>` | Visit a profile (registers in "who viewed my profile") |
| `profile viewers` | Show who viewed your profile |
| `profile audit` | Audit your profile for staleness and missing sections |
| **Feed** | |
| `feed list` | List feed updates (paginated) |
| `feed read <n>` | Show full post details for item N from the last `feed list` |
| `feed view <urn>` | Display a post by activity URN; scans your top-50 feed first and falls back to LinkedIn's `highlightedFeed` finder for out-of-window posts |
| `feed comments <n>` | Show comments on post N from the last `feed list` |
| `feed react <urn>` | React to a post (LIKE, PRAISE, EMPATHY, etc.) |
| `feed unreact <urn>` | Remove a reaction from a post |
| `feed comment <urn> <text>` | Comment on a feed post |
| `feed my-posts` | List your own posts with engagement analytics |
| `feed reactions <urn>` | Show who reacted to a post (names, headlines, reaction types) |
| `feed stats` | Aggregate engagement stats across your recent posts |
| `feed post <text>` | Create a new text post (public or connections-only) |
| **Messages** | |
| `messages list` | List conversations (cursor-based pagination) |
| `messages read <id>` | Read messages in a conversation (cursor-paginated via `--before`; no `--count`) |
| `messages send <recipient> <text>` | Send a message to a connection (new conversation) |
| `messages reply <id> <text>` | Reply to an existing conversation thread |
| **Connections** | |
| `connections list` | List your connections (paginated) |
| `connections invite <id>` | Send a connection request with optional message |
| `connections invite-batch --from-file <path>` | Send invitations to multiple members from a list (file or stdin) |
| `connections invitations` | List pending received invitations |
| `connections accept <id>` | Accept a pending invitation |
| `connections withdraw <id>` | Withdraw a sent (pending) invitation |
| **Search** | |
| `search people <keywords>` | Search for people by keywords |
| `search jobs <keywords>` | Search for jobs by keywords |
| `search posts <keywords>` | Search for posts/content by keywords |
| `search react <n>` | React to a post from the last search results |
| `search view <n>` | View a profile from the last people search results |
| `search invite <n>` | Send a connection request to a profile from the last people search |
| **Company** | |
| `company view <slug>` | View company info by URL slug |
| `company followers <slug>` | List company page followers (requires admin access) |
| **Events** | |
| `events view <id>` | View event details by event ID |
| `events attendees <id>` | List event attendees (paginated) |
| **Notifications** | |
| `notifications list` | List notification cards (paginated) |
| `notifications mentions <n>` | Show everyone @-mentioned in the post behind notification N |
| **Composite** | |
| `inbox` | Daily inbox: unread messages, pending invitations, recent notifications |
| `who <company>` | Who do you know at a company? Network overlap in one command |
| **Shell** | |
| `completions <shell>` | Generate shell completions (bash, zsh, fish, powershell, elvish) |

Most list commands support `--count`, `--start` (or `--before` for cursor pagination), and `--json` for raw JSON output. Exception: `messages read <id>` returns the whole conversation window LinkedIn hands back and paginates only via `--before <epoch_ms>`; it doesn't take `--count`. Write commands that have a visible side effect on others (`feed react`, `feed comment`, `feed post`, `messages send`, `messages reply`) require `--yes` to confirm.

## Installation

### Prerequisites

This project uses Nix for reproducible builds. All dependencies (Rust toolchain, Android tools, analysis tools) are declared in `shell.nix`.

```bash
# Enter the development shell
nix-shell

# Build the workspace
just build

# Run all checks (build + test + lint + format)
just e2e
```

### Just Recipes

| Recipe | Description |
|--------|-------------|
| `just build` | Build the Rust workspace |
| `just test` | Run all tests |
| `just lint` | Run clippy (warnings are errors) |
| `just fmt` | Format all code |
| `just fmt-check` | Check formatting without modifying |
| `just e2e` | Full gate: build, test, lint, format check |
| `just run <args>` | Run the CLI with arguments |

## Authentication

LinkedIn API access requires a `li_at` session cookie. This is a cookie-based auth approach -- no OAuth app registration needed.

### Getting the li_at cookie

1. Log into linkedin.com in Chrome
2. Open DevTools: F12 (or Cmd+Option+I on macOS)
3. Go to **Application** tab > **Cookies** > `https://www.linkedin.com`
4. Find the `li_at` cookie and copy its value

### Logging in

```bash
# Pass directly
li auth login --li-at "AQEDAQx..."

# Or via environment variable
export LINKEDIN_LI_AT="AQEDAQx..."
li auth login

# Verify the session works
li auth status
```

The session is stored locally at `~/.config/linkedin-cli/session.json`.

## Usage

### Profile

```bash
# Your own profile
li profile me

# View someone's profile
li profile view john-doe-123

# Visit a profile (shows up in "who viewed")
li profile visit john-doe-123

# Who viewed your profile
li profile viewers

# Audit your profile for staleness
li profile audit
```

### Feed

```bash
# List recent feed items
li feed list --count 20

# Read full details of post #3 from the last feed list
li feed read 3

# View a specific post by URN (cache-warm in your top-50 feed,
# or via the highlightedFeed fallback for older posts)
li feed view urn:li:activity:7312345678901234567

# Show comments on post #5 from the last feed list
li feed comments 5

# Your own posts with engagement metrics
li feed my-posts --count 10

# Who reacted to a specific post — use --from-list N after `feed list` /
# `feed my-posts` so the CLI picks the right URN type (ugcPost vs activity;
# LinkedIn's reactions endpoint is picky depending on post backing)
li feed list --count 10
li feed reactions --from-list 3
# Passing a URN directly also works, but activity URNs silently return 0
# for ugcPost-backed posts — the CLI prints a hint when it sees that
li feed reactions urn:li:activity:7312345678901234567

# Aggregate stats across your recent posts
li feed stats

# Like a post
li feed react urn:li:activity:7312345678901234567 --yes

# Celebrate a post
li feed react urn:li:activity:7312345678901234567 --type CELEBRATION --yes

# Remove a reaction
li feed unreact urn:li:activity:7312345678901234567

# Comment on a post
li feed comment urn:li:activity:7312345678901234567 "Great post!" --yes

# Create a post
li feed post "Hello LinkedIn!" --yes
li feed post "Only for my network" --visibility CONNECTIONS_ONLY --yes
```

### Messages

```bash
# List conversations
li messages list --count 20

# Read a conversation
li messages read 2-abc123

# Send a message (new conversation)
li messages send john-doe-123 "Hey, wanted to connect about..." --yes

# Reply to an existing conversation
li messages reply 2-abc123 "Thanks for getting back to me" --yes
```

### Connections

```bash
# List connections
li connections list --count 50

# Send a connection request
li connections invite john-doe-123
li connections invite john-doe-123 --message "Met you at the conference"

# Batch invitations from a file (one public ID or fsd_profile URN per line;
# also accepts stdin via --from-file -)
li connections invite-batch --from-file invites.txt --message "Hi from the meetup"

# List pending received invitations
li connections invitations

# Accept an invitation (get ID and secret from invitations --json)
li connections accept 7312345678901234567 --secret abc123

# Withdraw a sent invitation that hasn't been accepted yet
li connections withdraw 7312345678901234567 --secret abc123
```

### Search

```bash
# Search for people
li search people "rust developer" --count 20

# Search for jobs
li search jobs "senior backend engineer"

# Search for posts
li search posts "MCP server" --count 10

# React to post #2 from the last search results
li search react 2

# View profile #3 from the last people search
li search view 3

# Invite profile #4 from the last people search to connect
li search invite 4 --message "Saw your post on observability"
```

### Company

```bash
# View company info
li company view microsoft

# List company page followers (requires admin access)
li company followers getskillcheck --count 50
```

### Events

```bash
# View event details (ID from the event URL)
li events view 7447661801514938369

# List event attendees
li events attendees 7447661801514938369 --count 50
```

### Notifications

```bash
# List notifications
li notifications list --count 20

# Show everyone @-mentioned alongside you in the post behind notification #3
li notifications mentions 3
```

### Composite Commands

```bash
# Daily inbox: unread messages, invitations, notifications in one shot
li inbox

# Show inbox without spam filtering
li inbox --all

# Who do you know at a company?
li who miro
```

### JSON output

All commands support `--json` for machine-readable output:

```bash
li profile me --json | jq '.firstName'
li feed list --json --count 5 | jq '.elements[].text'
```

### Shell completions

```bash
# Generate completions for your shell
li completions zsh > ~/.zfunc/_li
li completions bash > ~/.local/share/bash-completion/completions/li
li completions fish > ~/.config/fish/completions/li.fish
```

## API Library

The `linkedin-api` crate can be used as a standalone Rust library:

```toml
[dependencies]
linkedin-api = { path = "linkedin/linkedin-api" }
```

It provides:

- `LinkedInClient` -- HTTP client with cookie jar, auth header decoration, and CSRF handling
- `Session` -- session management (load, save, validate)
- Typed request/response models for all supported endpoints
- Rest.li protocol handling (headers, pagination, union unwrapping)

Key dependencies: reqwest (with cookies + JSON), serde, chrono, thiserror, tokio.

## Architecture

### Auth model

Authentication uses cookie-based sessions rather than OAuth2 app tokens:

1. User provides a `li_at` cookie extracted from a browser session
2. The client also requires a CSRF token (`JSESSIONID` cookie) which is echoed as the `csrf-token` header
3. Sessions are persisted locally and reused across CLI invocations

### API transport

LinkedIn uses two API styles, both in active use:

- **Rest.li 2.0** -- LinkedIn's custom REST framework. Requires `X-RestLi-Protocol-Version: 2.0.0` and `X-RestLi-Method` headers. Responses wrap data in `elements` arrays with `paging` metadata.
- **GraphQL (Voyager/Dash)** -- Newer endpoints use GraphQL queries with hardcoded query IDs (`queryId` parameter). The app is progressively migrating from Rest.li to Dash.

### Required headers

Every request includes a set of headers that mimic the Android app: `User-Agent`, `X-Li-Lang`, `X-Li-Track`, `Accept-Language`, and the CSRF token header.

## Limitations

- **TLS fingerprint mismatch**: The library uses rustls, not Chrome/BoringSSL. LinkedIn may detect this difference. Switching to `boring-tls` (BoringSSL bindings for reqwest) would improve fingerprint fidelity. See `re/tls_configuration.md`.
- **Query ID brittleness**: GraphQL (Dash) endpoints use hardcoded `queryId` values extracted from the APK. These may change with app updates, requiring re-extraction.
- **Write operations**: Posting, commenting, messaging, and connection requests hit LinkedIn's live systems. These may trigger additional validation, CAPTCHA challenges, or rate limiting that read-only operations do not.
- **Rate limiting**: LinkedIn actively detects automated access. Excessive requests can lead to CAPTCHA challenges or account restrictions. No built-in rate limiter is provided -- callers should throttle themselves.
- **No real-time messaging**: The current implementation uses request/response only. LinkedIn's real-time messaging system (long-poll / SSE) is documented but not implemented.

## Security

- **Tokens and credentials** are stored in `secrets/` which is gitignored. Never commit cookies, session files, or captured API responses.
- **PII scan** before any push to remote. Captured responses often contain names, emails, profile URLs, and other personal data.
- **Session files** at `~/.config/linkedin-cli/session.json` contain your `li_at` cookie. Protect this file as you would a password.
- **Never commit** APK files, decompiled output, or raw API responses.

## Project Structure

```
linkedin/
  linkedin-api/     Rust library crate (client, auth, models, services)
  linkedin-cli/     Rust binary crate (clap CLI)
re/                 Reverse engineering documentation
secrets/            Tokens, captured responses, PII (gitignored)
shell.nix           Nix development environment
Justfile            Build recipes
```
