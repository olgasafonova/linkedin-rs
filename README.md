# LinkedIn Reversed

Rust CLI and API library for LinkedIn, reverse-engineered from the Android app (`com.linkedin.android`). Provides programmatic access to LinkedIn's core features: profiles, messaging, feed, connections, search, and notifications.

This project is for personal and educational use only.

## Features

The CLI (`linkedin-cli`) exposes 22 subcommands across 7 domains:

| Command | Description |
|---------|-------------|
| `auth login` | Authenticate with a `li_at` cookie (from browser or env var) |
| `auth status` | Check session validity (live API call or local-only) |
| `auth logout` | Clear stored session |
| `profile me` | Fetch your own profile |
| `profile view <id>` | View a profile by public identifier (vanity URL slug) |
| `profile visit <id>` | Visit a profile (registers in "who viewed my profile") |
| `profile viewers` | Show who viewed your profile |
| `feed list` | List feed updates (paginated) |
| `feed react <urn>` | React to a post (LIKE, PRAISE, EMPATHY, etc.) |
| `feed unreact <urn>` | Remove a reaction from a post |
| `feed comment <urn> <text>` | Comment on a feed post |
| `feed post <text>` | Create a new text post (public or connections-only) |
| `connections list` | List your connections (paginated) |
| `connections invite <id>` | Send a connection request with optional message |
| `connections invitations` | List pending received invitations |
| `connections accept <id>` | Accept a pending invitation |
| `search people <keywords>` | Search for people by keywords |
| `search jobs <keywords>` | Search for jobs by keywords |
| `messages list` | List conversations (cursor-based pagination) |
| `messages read <id>` | Read messages in a conversation |
| `messages send <recipient> <text>` | Send a message to a connection |
| `notifications list` | List notification cards (paginated) |

All list commands support `--count`, `--start` (or `--before` for cursor pagination), and `--json` for raw JSON output. Write commands (`comment`, `post`) require `--yes` to skip the confirmation prompt.

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
linkedin-cli auth login --li-at "AQEDAQx..."

# Or via environment variable
export LINKEDIN_LI_AT="AQEDAQx..."
linkedin-cli auth login

# Verify the session works
linkedin-cli auth status
```

The session is stored locally at `~/.config/linkedin-cli/session.json`.

## Usage

### Profile

```bash
# Your own profile
linkedin-cli profile me

# View someone's profile
linkedin-cli profile view john-doe-123

# Visit a profile (shows up in "who viewed")
linkedin-cli profile visit john-doe-123

# Who viewed your profile
linkedin-cli profile viewers
```

### Feed

```bash
# List recent feed items
linkedin-cli feed list --count 20

# Like a post
linkedin-cli feed react urn:li:activity:7312345678901234567

# Celebrate a post
linkedin-cli feed react urn:li:activity:7312345678901234567 --type CELEBRATION

# Remove a reaction
linkedin-cli feed unreact urn:li:activity:7312345678901234567

# Comment on a post
linkedin-cli feed comment urn:li:activity:7312345678901234567 "Great post!" --yes

# Create a post
linkedin-cli feed post "Hello LinkedIn!" --yes
linkedin-cli feed post "Only for my network" --visibility CONNECTIONS_ONLY --yes
```

### Messages

```bash
# List conversations
linkedin-cli messages list --count 20

# Read a conversation
linkedin-cli messages read 2-abc123

# Send a message
linkedin-cli messages send john-doe-123 "Hey, wanted to connect about..."
```

### Connections

```bash
# List connections
linkedin-cli connections list --count 50

# Send a connection request
linkedin-cli connections invite john-doe-123
linkedin-cli connections invite john-doe-123 --message "Met you at the conference"

# List pending invitations
linkedin-cli connections invitations

# Accept an invitation (get ID and secret from invitations --json)
linkedin-cli connections accept 7312345678901234567 --secret abc123
```

### Search

```bash
# Search for people
linkedin-cli search people "rust developer" --count 20

# Search for jobs
linkedin-cli search jobs "senior backend engineer"
```

### Notifications

```bash
# List notifications
linkedin-cli notifications list --count 20
```

### JSON output

All commands support `--json` for machine-readable output:

```bash
linkedin-cli profile me --json | jq '.firstName'
linkedin-cli feed list --json --count 5 | jq '.elements[].text'
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

## Reverse Engineering Docs

All reverse engineering artifacts live in `re/`. These document what was learned from decompiling the Android APK and observing live API traffic.

| Document | Description |
|----------|-------------|
| `prd.md` | Project scope, milestones, and security rules |
| `architecture_overview.md` | Android app architecture and module layout |
| `auth_flow.md` | Authentication flow: OAuth2, CSRF, cookie-based sessions |
| `api_endpoint_catalog.md` | Discovered API endpoints across all domains |
| `restli_protocol.md` | Rest.li protocol details: headers, methods, encoding |
| `pegasus_models.md` | LinkedIn's Pegasus data models and type system |
| `search_protocol.md` | Search API protocol (GraphQL Dash queries) |
| `tls_configuration.md` | TLS fingerprint analysis and BoringSSL comparison |
| `device_fingerprinting.md` | Device fingerprinting and anti-automation signals |
| `rate_limiting.md` | Rate limiting behavior and thresholds |
| `realtime_system.md` | Real-time messaging system (long-poll / SSE) |
| `media_upload.md` | Media upload flow and endpoints |
| `serialization_edge_cases.md` | Rest.li serialization quirks and edge cases |
| `lix_feature_flags.md` | LinkedIn's LIX feature flag system |
| `model_corrections.md` | Corrections to decompiled models after live validation |
| `manifest_analysis.md` | AndroidManifest.xml analysis |
| `file_analysis.md` | APK file structure analysis |
| `js_bundle_analysis.md` | Embedded JavaScript bundle analysis |
| `apk_variant_comparison.md` | Comparison of APK variants (intl vs standard) |
| `intl_vs_zephyr_diff.md` | Differences between LinkedIn Intl and Zephyr builds |
| `reactions.md` | Reaction API protocol and types |
| `create_post.md` | Post creation API details |
| `comments.md` | Comment API protocol |
| `connection_request.md` | Connection request / invitation API |
| `invitations.md` | Invitation listing and acceptance API |
| `profile_visit.md` | Profile visit tracking API |
| `profile_viewers.md` | "Who viewed your profile" API |
| `send_message.md` | Message sending API details |

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
