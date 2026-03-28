# LinkedIn from your terminal

Check your inbox, reply to messages, browse your feed, react to posts, look up companies, message people by name. No browser needed.

```bash
linkedin-cli inbox                                    # morning check
linkedin-cli feed list --author "Satya" --count 20    # filtered feed
linkedin-cli feed read 3                              # full post + media
linkedin-cli feed react 3 --type CELEBRATION           # react by index
linkedin-cli messages send "Jane Doe" "coffee next week?"   # send by name
linkedin-cli search posts "AI engineering"             # search + links
linkedin-cli search react 2                            # like result #2
```

Built in Rust. Reverse-engineered from the LinkedIn Android app. For personal and educational use only.

> Forked from [eisbaw/linkedin-rs](https://github.com/eisbaw/linkedin-rs) by [Mark Ruvald Pedersen](https://github.com/eisbaw), who built the original client, auth flow, Rest.li protocol handling, and core CLI structure.

## Quick Start

```bash
# 1. Get your li_at cookie from browser DevTools
#    (linkedin.com > F12 > Application > Cookies > li_at)

# 2. Authenticate
linkedin-cli auth login --li-at "AQEDAQx..."

# 3. Go
linkedin-cli inbox
```

**About the cookie:** This CLI piggybacks on your browser session via the `li_at` cookie. It's not a stable API key; LinkedIn can expire or invalidate it at any time (password change, security event, or just because). When it stops working, grab a fresh cookie from your browser. The CLI warns you when your session is getting old.

Requires Rust 1.75+. Install with `cargo install --path linkedin/linkedin-cli`.

## What You Can Do

### Daily workflow
| Command | What it does |
|---------|-------------|
| `inbox` | Unread messages, pending invitations, notifications in one view |
| `feed list` | Your feed, with `--author` and `--keyword` filters |
| `feed read <N>` | Full post: reshares, articles, media URLs, engagement |
| `feed react <N>` | React to post N (LIKE, PRAISE, EMPATHY, CELEBRATION, etc.) |
| `feed comment <N> <text>` | Comment on post N |
| `feed stats` | Your post analytics: views, impressions, engagement |
| `notifications list` | Recent notifications |

### Messages
| Command | What it does |
|---------|-------------|
| `messages list` | Conversations (unread marked) |
| `messages read <id>` | Read a thread with reply context |
| `messages send <recipient> <text>` | Send by name ("Paul Bang"), slug, or URN |
| `messages reply <thread_id> <text>` | Reply to an existing conversation |

### People and companies
| Command | What it does |
|---------|-------------|
| `profile view <slug>` | Profile with headline, positions, education, connections |
| `profile visit <slug>` | Register a profile view ("who viewed my profile") |
| `profile viewers` | Who viewed yours |
| `company view <slug>` | Company info: industry, size, HQ, followers |
| `company followers <slug>` | Follower count + your connections that follow |
| `connections list` | Your connections, with `--keyword` filter and `--all` for export |
| `connections invite <slug>` | Send a connection request |
| `connections invitations` | Pending invitations with accept commands |

### Search
| Command | What it does |
|---------|-------------|
| `search people <keywords>` | Find people |
| `search jobs <keywords>` | Find jobs |
| `search posts <keywords>` | Find posts with clickable LinkedIn links |
| `search react <N>` | React to search result N |
| `search view <N>` | View profile of people result N |

### Posting
| Command | What it does |
|---------|-------------|
| `feed post <text> --yes` | Publish a text post (public or connections-only) |

Every command supports `--json` for machine-readable output. Pipe to `jq` for scripting.

## How It Works

Authentication uses your `li_at` session cookie from the browser. No OAuth app, no API keys, no approval process. One cookie, and you're in.

The client impersonates the LinkedIn Android app (Google Pixel 8, Android 14) with matching headers, CSRF tokens, and device fingerprinting. It speaks both Rest.li 2.0 and LinkedIn's newer GraphQL/Dash protocol, depending on the endpoint.

Built-in resilience: 1 req/sec throttle to stay under rate limits, exponential backoff with Retry-After support on 429/5xx, session age warnings before you hit silent 401s.

## Installation

### Cargo (recommended)

```bash
cd linkedin
cargo build --release
cargo install --path linkedin-cli
```

### Nix

```bash
nix-shell
just build
just e2e    # build + test + lint + format
```

## Limitations

- **TLS fingerprint**: Uses rustls, not BoringSSL. LinkedIn may detect the difference.
- **Query ID brittleness**: GraphQL endpoints use hardcoded IDs from the decompiled APK. App updates can break them.
- **Write operations**: Posts, comments, messages are real. LinkedIn may trigger CAPTCHA or rate limiting.
- **No real-time messaging**: Request/response only; no live notifications.

## Security

Your `li_at` cookie is stored at `~/.local/share/linkedin/session.json` with 0600 permissions. Treat it like a password. Never commit it.

## Project Structure

```
linkedin/
  linkedin-api/     Rust library (client, auth, models)
  linkedin-cli/     Rust binary (clap CLI)
re/                 Reverse engineering docs
secrets/            Tokens and captured responses (gitignored)
```

## Credits

Built on [eisbaw/linkedin-rs](https://github.com/eisbaw/linkedin-rs) by [Mark Ruvald Pedersen](https://github.com/eisbaw). The original project established the client architecture, authentication flow, Rest.li/GraphQL protocol implementation, device fingerprinting, and the core CLI command structure.
