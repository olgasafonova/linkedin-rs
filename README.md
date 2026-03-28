# LinkedIn Reversed

Rust CLI and API library for LinkedIn, reverse-engineered from the Android app (`com.linkedin.android`). Programmatic access to profiles, messaging, feed, connections, search, companies, and notifications.

For personal and educational use only.

> **Forked from [eisbaw/linkedin-rs](https://github.com/eisbaw/linkedin-rs)** by Mark Ruvald Pedersen, who built the original client, auth flow, Rest.li protocol handling, and core CLI structure. This fork adds the features listed under [What's New](#whats-new).

## Features

The CLI exposes 30+ subcommands across 8 domains:

### Auth
| Command | Description |
|---------|-------------|
| `auth login --li-at <value>` | Authenticate with a `li_at` cookie |
| `auth status` | Check session validity (live or `--local`) |
| `auth logout` | Clear stored session |

### Profile
| Command | Description |
|---------|-------------|
| `profile me` | Your own profile |
| `profile view <id>` | View a profile by vanity URL slug |
| `profile visit <id>` | Register a profile view |
| `profile viewers` | Who viewed your profile |

### Feed
| Command | Description |
|---------|-------------|
| `feed list` | List feed updates with `--author` and `--keyword` filters |
| `feed read <index>` | Full post details: reshares, articles, media URLs |
| `feed comments <index>` | Comments on a post |
| `feed react <urn>` | React (LIKE, PRAISE, EMPATHY, INTEREST, APPRECIATION, ENTERTAINMENT, CELEBRATION) |
| `feed unreact <urn>` | Remove a reaction |
| `feed comment <urn> <text>` | Comment on a post (requires `--yes`) |
| `feed post <text>` | Create a text post (requires `--yes`) |
| `feed stats` | Post analytics: views, impressions, engagement |

### Messages
| Command | Description |
|---------|-------------|
| `messages list` | List conversations |
| `messages read <id>` | Read messages with threading context |
| `messages send <recipient> <text>` | Send a message (accepts name, slug, or URN) |
| `messages reply <conversation_id> <text>` | Reply to an existing thread (requires `--yes`) |

### Connections
| Command | Description |
|---------|-------------|
| `connections list` | List connections with `--keyword` filter and `--all` auto-pagination |
| `connections invite <id>` | Send a connection request |
| `connections invitations` | Pending received invitations |
| `connections accept <id>` | Accept an invitation |

### Company
| Command | Description |
|---------|-------------|
| `company view <slug>` | Company info: name, tagline, industry, size, HQ, followers |
| `company followers <slug>` | Follower count and first-degree connections that follow |

### Search
| Command | Description |
|---------|-------------|
| `search people <keywords>` | Search for people |
| `search jobs <keywords>` | Search for jobs |
| `search posts <keywords>` | Search posts with activity URNs and links |
| `search react <index>` | React to a post from last search results |
| `search view <index>` | View a profile from last people search results |

### Notifications
| Command | Description |
|---------|-------------|
| `notifications list` | List notification cards |

### Quick Access
| Command | Description |
|---------|-------------|
| `inbox` | Unread messages, pending invitations, recent notifications |

### Utilities
| Command | Description |
|---------|-------------|
| `completions bash\|zsh\|fish` | Generate shell completions |

All list commands support `--count`, `--start` (or `--before`), and `--json`. Write commands require `--yes`.

## What's New

Features added in this fork:

**Feed enrichment** -- `feed read` shows reshared posts (original author + text), article links (title + URL), media type labels, and actual media URLs (images, videos, documents, carousels). `feed list` shows media type badges inline.

**Filtering** -- `feed list --author "name" --keyword "term"` and `connections list --keyword "engineer"` for case-insensitive client-side filtering.

**Search with links** -- `search posts` results include activity URNs and clickable LinkedIn URLs.

**Company pages** -- `company view` and `company followers` commands.

**Message threading** -- `messages read` shows reply context. `messages reply` sends to existing conversation threads.

**Post analytics** -- `feed stats` fetches views, impressions, and engagement via `identity/socialUpdateAnalytics`.

**Rate limiting** -- Automatic retry with exponential backoff (1s, 2s, 4s) on HTTP 429 and 5xx errors. Respects `Retry-After` headers. Max 3 retries. Proactive throttling at 1 req/sec to stay under LinkedIn's rate limits.

**Auto-pagination** -- `connections list --all` fetches every connection with built-in throttling. Supports `--keyword` filter during pagination and `--json` for bulk export.

**Session lifecycle** -- Detects stale sessions (30/90/365 day thresholds) and warns before API calls. No more silent 401 surprises.

**Structured exit codes** -- Scripts can check `$?`: 0=success, 2=auth error, 3=API error, 4=input error.

**Shell completions** -- `linkedin-cli completions bash|zsh|fish` for tab completion.

**CI pipeline** -- GitHub Actions runs build, test, clippy, format check, and PII scan on every push.

## Installation

### With Nix

```bash
nix-shell
just build
just e2e    # build + test + lint + format
```

### Without Nix

Requires Rust 1.75+.

```bash
cd linkedin
cargo build --release
cargo install --path linkedin-cli
```

### Just Recipes

| Recipe | Description |
|--------|-------------|
| `just build` | Build the workspace |
| `just test` | Run all tests |
| `just lint` | Run clippy (warnings are errors) |
| `just fmt` | Format all code |
| `just e2e` | Full gate: build, test, lint, format |
| `just run <args>` | Run the CLI |

## Authentication

Requires a `li_at` session cookie from your browser. No OAuth app registration needed.

### Getting the cookie

1. Log into linkedin.com in Chrome
2. DevTools (F12) > **Application** > **Cookies** > `https://www.linkedin.com`
3. Copy the `li_at` cookie value

### Login

```bash
linkedin-cli auth login --li-at "AQEDAQx..."

# Or via env var
export LINKEDIN_LI_AT="AQEDAQx..."
linkedin-cli auth login

# Verify
linkedin-cli auth status
```

Session stored at `~/.config/linkedin-cli/session.json`.

## Usage Examples

```bash
# Browse your feed, filter by author
linkedin-cli feed list --count 20 --author "Satya"

# Read a post with full details (media, reshares, articles)
linkedin-cli feed read 3

# Search posts and get links
linkedin-cli search posts "AI engineering" --count 10

# Company info
linkedin-cli company view microsoft

# Reply to a conversation thread
linkedin-cli messages reply 2-abc123 "Sounds great, let's do Thursday" --yes

# Post analytics
linkedin-cli feed stats

# JSON output for scripting
linkedin-cli profile me --json | jq '.miniProfile.firstName'
linkedin-cli connections list --json --count 100 | jq '.elements[].miniProfile.publicIdentifier'
```

## API Library

The `linkedin-api` crate works as a standalone library:

```toml
[dependencies]
linkedin-api = { path = "linkedin/linkedin-api" }
```

Core types:
- `LinkedInClient` -- HTTP client with cookie jar, CSRF, device fingerprinting, and automatic retry
- `Session` -- session persistence (load, save, validate)
- Typed models for feed, connections, search, notifications, messaging
- Rest.li 2.0 + GraphQL protocol handling

## Architecture

**Auth**: Cookie-based (`li_at` + auto-generated `JSESSIONID` for CSRF). No OAuth app needed.

**Transport**: Two API styles in active use:
- **Rest.li 2.0** -- LinkedIn's custom REST framework with `X-RestLi-Protocol-Version: 2.0.0`, parenthesized record encoding, and `elements`/`paging` response wrappers.
- **GraphQL (Dash)** -- Newer endpoints using hardcoded `queryId` values from the decompiled APK.

**Resilience**: Two layers of protection against rate limits:
1. Proactive throttle (1 req/sec minimum interval between API calls)
2. Reactive retry (exponential backoff on 429/5xx, `Retry-After` header support, max 3 retries)

**Session management**: Sessions track creation time. The CLI warns at 30/90/365 day thresholds so you re-authenticate before getting silent 401s.

**Headers**: Every request includes Android app headers (`User-Agent`, `X-Li-Track`, `X-UDID`, `X-Li-Lang`, CSRF token) matching a Google Pixel 8 on Android 14.

## Limitations

- **TLS fingerprint**: Uses rustls, not BoringSSL. LinkedIn may detect the difference. See `re/tls_configuration.md`.
- **Query ID brittleness**: GraphQL `queryId` values are extracted from the APK and may break with app updates.
- **Write operations**: Posts, comments, messages hit LinkedIn's live systems. May trigger CAPTCHA challenges beyond what the retry logic handles.
- **No real-time messaging**: Request/response only. LinkedIn's long-poll/SSE system is documented in `re/` but not implemented.

## Security

- Tokens and credentials go in `secrets/` (gitignored). Never commit cookies or captured responses.
- PII scan before any push. API responses contain names, emails, and profile URLs.
- Session file (`~/.config/linkedin-cli/session.json`) contains your `li_at` cookie. Protect it like a password.

## Project Structure

```
linkedin/
  linkedin-api/     Rust library (client, auth, models)
  linkedin-cli/     Rust binary (clap CLI)
re/                 Reverse engineering docs
secrets/            Tokens, responses, PII (gitignored)
shell.nix           Nix dev environment
flake.nix           Nix flake
Justfile            Build recipes
```

## Credits

Built on [eisbaw/linkedin-rs](https://github.com/eisbaw/linkedin-rs) by [Mark Ruvald Pedersen](https://github.com/eisbaw). The original project established the client architecture, authentication flow, Rest.li/GraphQL protocol implementation, device fingerprinting, and the core CLI command structure.
