# LinkedIn-rs inbox analysis — 2026-05-02

## Scope

Analyze `linkedin-rs` messaging/inbox support against the current live LinkedIn inbox behavior observed through the existing LinkedIn automation stack.

## Current Rust implementation

Repo: `<repo-root>`

Messaging CLI surface:

```bash
linkedin-cli messages list --count 20 [--before TS] [--json]
linkedin-cli messages read <conversation_id> [--before TS] [--json]
linkedin-cli messages send <recipient> <message>
```

Implementation files:

- `linkedin/linkedin-api/src/client.rs`
- `linkedin/linkedin-cli/src/main.rs`

`get_conversations()` uses GraphQL:

- Finder: `messengerConversationsByCategory`
- Query ID: `voyagerMessagingDashMessengerConversations.7dc50d3efc3953190125aca9c05f0af6`
- Query name: `MessengerConversationsByCategory`
- Variables currently generated:

```text
(mailboxUrn:<encoded urn>,category:PRIMARY_INBOX,count:<n>)
```

with optional cursor:

```text
(mailboxUrn:<encoded urn>,category:PRIMARY_INBOX,count:<n>,lastActivityBefore:<epoch_ms>)
```

`get_conversation_events()` uses GraphQL:

- Finder: `messengerMessagesByConversation`
- Query ID: `voyagerMessagingDashMessengerMessages.7cde5843a127bbecc3de900d3894a74a`
- Query name: `MessengerMessagesByConversation`
- It constructs:

```text
urn:li:msg_conversation:(<viewer fsd_profile urn>,<thread id>)
```

and sends:

```text
(conversationUrn:<encoded urn>)
```

with optional cursor:

```text
(conversationUrn:<encoded urn>,deliveredBefore:<epoch_ms>)
```

## Live validation results

Rust test suite passes:

```text
41 unit tests passed
2 integration/smoke tests passed
```

The stored `linkedin-rs` session file exists at:

```text
<session-dir>/session.json
```

but live session validation currently fails:

```text
error: API call failed: HTTP error: error following redirect for url (https://www.linkedin.com/voyager/api/me)
```

Interpretation: the local `linkedin-rs` `li_at` session is stale or insufficient for live API calls. This is not a compile/runtime bug in messaging itself; it blocks live validation of the Rust client until the session is refreshed from an authenticated browser cookie.

Control data from the active LinkedIn automation stack still works for inbox listing via AutoCLI fallback. It returned these visible inbox threads:

- Contact A — May 2 — latest sales/hackathon bump
- Contact B — Apr 30 — “How come can’t contact you”
- Contact C — Apr 28 — unread — “Hope all ok!”
- Contact D — sponsored AI Engineering item
- Contact E — PMP/PDU pitch
- Contact F — Apr 10 — “Seeing this only now”
- Contact G — Apr 2 — “That's great info! Thank you :)”
- Contact H — Mar 30 — InMail opportunity
- Contact I — Mar 23 — “Very helpful, many thanks bro!”
- Contact J — Mar 17 — unread — “Let me know which sections interest you.”

Live browser/API probe confirmed a useful implementation detail:

- `GET /voyager/api/messaging/conversations/<thread>/events` can return events for a thread.
- `GET /voyager/api/messaging/conversations/<thread>` returned HTTP 500.

That matches the code comment that conversation metadata moved away from the old REST endpoint, while event retrieval may still work through legacy events or GraphQL.

## Main inbox gap

`linkedin-rs` currently supports only one mailbox category:

```rust
category:PRIMARY_INBOX
```

The live LinkedIn UI exposes multiple inbox views/filters:

- Focused
- Jobs
- Unread
- Connections
- InMail
- Starred
- Other appears as a separate inbox surface in the UI/toast text

The Rust CLI cannot currently select any of these. `messages list` is effectively “Focused/primary only”, depending on LinkedIn's backend mapping for `PRIMARY_INBOX`.

## Likely implementation direction

Expose inbox/category/filter selection at the API and CLI layers.

Minimal API change:

```rust
pub enum ConversationCategory {
    PrimaryInbox,
    OtherInbox,
}

pub async fn get_conversations_by_category(
    &self,
    category: ConversationCategory,
    count: u32,
    created_before: Option<u64>,
) -> Result<Value, Error>
```

Minimal CLI shape:

```bash
linkedin-cli messages list --inbox primary --count 20 --json
linkedin-cli messages list --inbox other --count 20 --json
```

For UI filters, avoid guessing until captured live:

```bash
linkedin-cli messages list --filter unread
linkedin-cli messages list --filter connections
linkedin-cli messages list --filter inmail
linkedin-cli messages list --filter jobs
linkedin-cli messages list --filter starred
```

These may be separate GraphQL finder variables, not category enums. They need one live request capture per tab/filter before hardcoding.

## Risk notes

- Do not assume LinkedIn UI labels map 1:1 to GraphQL `category`. “Focused” likely maps to `PRIMARY_INBOX`, but “Jobs”, “Unread”, “Connections”, “InMail”, and “Starred” may be filters rather than categories.
- `messages read <conversation_id>` may require the exact thread ID from the conversation object's `backendUrn`; names like “Contact C” are not valid for this Rust command.
- The README says session path is `<session-dir>/session.json`, but the code uses `dirs::data_dir()/linkedin/session.json`, observed as `<session-dir>/session.json`. README is stale.
- The local `backlog` CLI is unavailable in this shell, so I could not add a project backlog task through the required backlog interface.

## Implemented in `feat/linkedin-proxy-inbox-categories`

1. Added `ConversationCategory` support for the live-verified `PRIMARY_INBOX` and `SPAM` categories. Live probes showed `OTHER` and `ARCHIVED` return GraphQL errors: `Conversation category ... is not supported`, so they are intentionally not exposed.
2. Added `LinkedInClient::get_conversations_by_category(...)` while preserving `get_conversations(...)` as the primary-inbox default.
3. Added `LINKEDIN_PROXY_URL` / `HTTPS_PROXY` / `HTTP_PROXY` support to the API client.
4. Added CLI support:

```bash
LINKEDIN_PROXY_URL=http://127.0.0.1:3128 linkedin-cli messages list --category spam --count 20 --json
```

5. Corrected README session path to `<session-dir>/session.json` and documented browser cookie map usage.
6. Updated `auth status` so the live API check uses the same browser-cookie fallback as profile/feed/messaging commands.

Verified:

```text
cargo fmt --check
cargo test -q
45 unit tests passed
2 integration/smoke tests passed
```

Remaining:

1. Refresh/import cookies from the active CloakBrowser Manager LinkedIn profile.
2. Run through the Proton SG proxy:

```bash
LINKEDIN_PROXY_URL=http://127.0.0.1:3128 linkedin-cli auth status
LINKEDIN_PROXY_URL=http://127.0.0.1:3128 linkedin-cli messages list --category primary --count 20 --json
LINKEDIN_PROXY_URL=http://127.0.0.1:3128 linkedin-cli messages list --category spam --count 20 --json
```

3. Capture live GraphQL request payloads for Jobs, Unread, Connections, InMail, and Starred before adding them as `--filter` values.