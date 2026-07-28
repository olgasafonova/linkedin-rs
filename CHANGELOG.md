# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). LinkedIn's underlying APIs are not a stable contract; minor versions may break when LinkedIn rotates query IDs or retires endpoints.

## [Unreleased]

### Changed

- **Profile resolution** — `resolve_profile_urn` no longer collapses every failure into `could not extract entityUrn from any profile endpoint`. It now returns `Error::ProfileResolution`, which carries a `ProfileResolutionFailure` classification (`RateLimited` / `SelfSlugUnsupported` / `NotFound` / `Inconclusive`) plus the per-endpoint outcomes that produced it, because those failures need opposite recoveries: wait, use a different endpoint, or fix the slug. Rate limiting is keyed on a real HTTP 429 that survived the client's retry loop or on LinkedIn's 999 bot-challenge status, never on response shape. `NotFound` requires every endpoint to report a clean absence (404 or empty); one odd status downgrades the verdict to `Inconclusive` rather than laundering it into "not found". An `Error::Auth` from any endpoint now short-circuits the whole resolver instead of being swallowed and reported as a missing profile. The CLI maps the classification to distinct exit codes (`NotFound` / `SelfSlugUnsupported` exit 4, `RateLimited` / `Inconclusive` exit 3) and distinct hints, and the three call sites that flattened the error into `CliError::Other(format!(…))` now propagate it typed.
- **Self-slug awareness** — `LinkedInClient` caches the signed-in member's own vanity slug (`known_self_public_id`, `set_self_public_id`, `my_public_id`), filled for free from any `/me` fetch and persisted in the session file as the optional `self_public_id` field (`serde(default)`, so older session files still load). `li profile me` and `li auth status` write it back. The self-slug check is three-valued, and "unknown" is never read as "not a self-slug". The slug resolvers are other-member endpoints (`identity/miniprofiles?q=memberIdentity`, `preload/custom-invite`), so a self-slug fails against them by construction — the resolver therefore short-circuits a known self-slug and resolves it via the `/me` profile URN instead, turning that by-construction failure into a working resolve. When the self slug is unknown up front, the four endpoints are tried first (so an endpoint that does answer for a self-slug still wins) and a single `/me` fetch on the failure path settles the verdict, resolving the URN when the slug turns out to be the caller's own.
- **Build** — added a tuned `[profile.release]` block (`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`). Release `li` binary measures 6.2MB after the change (down from ~12MB at v0.1.0). Pruned `linkedin-cli`'s tokio feature set from `["full"]` to `["macros", "rt-multi-thread", "time"]` — the only features actually used by the CLI (`#[tokio::main]`, `tokio::join!`, `tokio::time::sleep`).

### Security

- **API errors** — `Error::Api` and `Error::Auth` now sanitize their Display output: LinkedIn URNs collapse to `urn:li:<type>:[…]`, email-shaped substrings collapse to `[email]`, and the body truncates to 200 bytes (at a UTF-8 char boundary) with a `…[N more bytes]` suffix. The raw `body` field on `Error::Api` remains unsanitized for programmatic inspection (the GraphQL retry classifier substring-matches on it). LinkedIn 4xx/5xx bodies routinely include connection names, conversation snippets, and member URNs; the CLI's `format!("API call failed: {e}")` no longer leaks them.
- **Auth** — browser cookies are now resolved from the session directory (`<data_dir>/linkedin/browser_cookies.json`), not the current working directory. Set `LINKEDIN_COOKIES_FILE` to override. Running `li` from a directory that happens to contain `secrets/browser_cookies.json` no longer silently swaps in those cookies. The "Using browser cookies from ..." log message now prints the absolute path so the active identity is verifiable.
- **Auth** — `LinkedInClient::with_browser_cookies` now validates cookie names against `[A-Za-z0-9_-]+` before passing them into the cookie jar. Names containing semicolons, equals signs, commas, control characters, or whitespace are silently skipped. Defense-in-depth posture; reqwest already rejected most malformed cookies in practice.

## [v0.1.0] - 2026-04-29

First public release. The CLI exposes 44 subcommands across 11 domains and the `linkedin-api` crate can be used as a standalone library.

### Added

- **Auth** — cookie-based session management (`auth login`, `auth status`, `auth logout`); `li_at` cookie loaded from flag, env var, or `secrets/browser_cookies.json`.
- **Profile** — `profile me`, `profile view`, `profile visit`, `profile viewers`, `profile audit`. Supports both Voyager REST and the Dash GraphQL `identityDashProfilesByMemberIdentity` finder; preload-page HTML scraping fallback for vanity slugs that the GraphQL endpoint won't resolve.
- **Feed** — `feed list`, `feed read`, `feed view`, `feed comments`, `feed react`, `feed unreact`, `feed comment`, `feed my-posts`, `feed reactions`, `feed stats`, `feed post`. Cache-warm path for index-based commands via `last_feed.json`. `feed view <urn>` falls back to the `highlightedFeed` finder for posts outside the top-50 window.
- **Messages** — `messages list`, `messages read`, `messages send`, `messages reply`. Cursor-based pagination via `--before <epoch_ms>`. Recipient resolution accepts public IDs, fsd_profile URNs, and full names (fuzzy-matched against connections).
- **Connections** — `connections list`, `connections invite`, `connections invite-batch`, `connections invitations`, `connections accept`, `connections withdraw`. Batch invites read from a file or stdin with configurable pacing. Send/withdraw use the Dash `voyagerRelationshipsDashInvitations` action endpoints.
- **Search** — `search people`, `search jobs`, `search posts`, `search react`, `search view`, `search invite`. Index-based actions read from per-kind cache (`last_search_<kind>.json`).
- **Company** — `company view`, `company followers`. Follower listing tries multiple endpoints since admin and non-admin paths differ.
- **Events** — `events view`, `events attendees`.
- **Notifications** — `notifications list`, `notifications mentions <N>`. Mentions extracts fsd_profile URNs from the underlying post's `attributesV2` after fetching via `get_post`.
- **Composite** — `inbox` (parallel fetch of conversations + invitations + notifications with optional spam filtering), `who <company>` (network overlap query).
- **Shell completions** — `completions <shell>` for bash, zsh, fish, powershell, elvish.
- **Confirmation gates** — `--yes` required on every command with a visible side effect on third parties: `feed react`, `feed comment`, `feed post`, `messages send`, `messages reply`, `connections withdraw`.
- **API library** — `LinkedInClient` with cookie jar, CSRF handling, automatic retry on transient GraphQL errors, request throttling (1 req/s default, mutex-guarded), exponential backoff (1-30s) with `Retry-After` honoring.

### Security

- Tokens, captured responses, and PII live in `secrets/` (gitignored). No real names committed to source.
- Session files written with `0o600` permissions at `~/.config/linkedin-cli/session.json`.

### Known limitations

- TLS fingerprint uses rustls (not Chrome/BoringSSL); LinkedIn may detect the difference.
- GraphQL query IDs are extracted from the APK and may rotate between LinkedIn app releases, requiring re-extraction.
- Real-time messaging (long-poll / SSE) is documented in `re/realtime_system.md` but not implemented.
- Some endpoint paths (notably the `feed view` permalink fallback and `notifications mentions` post-body shape) are best-effort against LinkedIn's documented action enums and have not been verified against fresh live captures. Errors will be informative.
