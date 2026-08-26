# Comments (Comment on a Post)

## Endpoint Discovery

Comments use the Voyager GraphQL mutation endpoint, not REST.

**Source**: `ConversationsGraphQLClient.java` in the decompiled international APK
(`com.linkedin.android.conversations.graphql`).

### Query IDs (from static initializer)

| Operation | Key in hashMap | Query ID |
|-----------|---------------|----------|
| Create comment | `createSocialDashNormComments` | `voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e` |
| Update comment | `updateSocialDashNormComments` | `voyagerSocialDashNormComments.e5d241e394f629991b5784eb2b358e59` |
| Fetch comments | `socialDashCommentsBySocialDetail` | `voyagerSocialDashComments.59bca422f480a4cc0ce56ccd81181488` |
| Fetch replies | `socialDashCommentsByRepliesByCursor` | `voyagerSocialDashComments.8ada653d14b465e4f86d3ed7dcbe6695` |
| Fetch single | `socialDashCommentsBySingleComment` | `voyagerSocialDashComments.a84e91d6baaa2d2018fdc49f21541de5` |
| Hide comment | `doHideSocialDashHideCommentAction` | `voyagerSocialDashHideCommentAction.42dde3771a51671edde117e558ab9d46` |
| Unhide comment | `doUndoHideSocialDashHideCommentAction` | `voyagerSocialDashHideCommentAction.e0b625699dc0ad3e093ac92a0b53dfc0` |

### Routes.java Entries

```
FEED_COMMENTS("feed/comments")
FEED_NORMCOMMENTS("voyagerFeedSocialNormComments")
FEED_SOCIAL_DASH_NORM_COMMENTS("voyagerSocialDashNormComments")
FEED_DASH_COMMENT_SUPPLEMENT("voyagerFeedDashCommentSupplement")
```

## Data Model

### NormCommentForUpdate (Create uses same shape)

From `com.linkedin.android.pegasus.dash.gen.voyager.dash.social.NormCommentForUpdate`:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `commentary` | `TextViewModelForUpdate` | Yes | Comment text (has `text` field) |
| `threadUrn` | `Urn` | Yes | The post/activity URN being commented on |
| `origin` | `CommentOrigin` | No | Where the comment was made: `FEED`, `LOBBY`, `THEATER` |
| `organizationActorUrn` | `Urn` | No | For commenting as a company page |
| `nonMemberActorUrn` | `Urn` | No | For non-member actors |
| `mediaUnion` | `NormCommentMediaForCreate` | No | Attached media (article or vector) |
| `timeOffset` | `Long` | No | Time offset (for live video comments) |
| `tscpUrl` | `String` | No | TSCP URL |

### CommentOrigin enum

From `com.linkedin.android.pegasus.dash.gen.voyager.dash.social.CommentOrigin`:

- `FEED` - Standard feed view
- `LOBBY` - Lobby view
- `THEATER` - Theater/expanded view

### NormCommentMediaForCreate (union)

From `com.linkedin.android.pegasus.dash.gen.voyager.dash.social.NormCommentMediaForCreate`:

- `article` (`NormCommentArticleForCreate`): Article with `articleUrn` and optional `originalUrl`
- `vectorUrn` (`Urn`): Vector media URN

### TextViewModelForCreate

From `com.linkedin.android.pegasus.dash.gen.voyager.dash.common.text.TextViewModelForCreate`:

| Field | Type | Required |
|-------|------|----------|
| `text` | `String` | No (but needed for content) |
| `textDirection` | `TextDirection` | No |
| `accessibilityText` | `String` | No |
| `attributes` | `List<TextAttributeForCreate>` | No |
| `attributesV2` | `List<TextAttributeForCreate>` | No |

## Create Comment

### GraphQL Mutation

```
POST /voyager/api/graphql?action=execute&queryId=voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e&queryName=CreateSocialDashNormComments
Content-Type: application/json
x-li-graphql-pegasus-client: true
Csrf-Token: ajax:{jsessionid}

{
  "queryId": "voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e",
  "queryName": "CreateSocialDashNormComments",
  "variables": {
    "entity": {
      "commentary": {
        "text": "Your comment text here"
      },
      "threadUrn": "urn:li:activity:7312345678901234567",
      "origin": "FEED"
    }
  }
}
```

### Minimum Required Fields

For a text-only comment, only these fields are needed:
- `entity.commentary.text` - The comment text
- `entity.threadUrn` - The post URN
- `entity.origin` - Usually `FEED`

### Commenting as a company page (organization actor)

`NormCommentForUpdate` carries `organizationActorUrn` for page-admin comments,
but two things captured live on 26-08-2026 change how this actually works:

1. **The modern web client no longer uses the voyager GraphQL endpoint for
   comment creation.** It POSTs to the server-driven-UI action endpoint:

   ```
   POST /flagship-web/rsc-action/actions/server-request?sduiid=<...>&parentSpanId=<...>
   requestId: com.linkedin.sdui.comments.createComment
   ```

   The body is an SDUI payload keyed by server-generated render state
   (`commentBoxText-CgsIgMDVwMX3y4rQAQ-...`), so it **cannot be reconstructed
   offline** — the state keys come from the live page render. Replaying this
   endpoint from a headless CLI is not practical.

2. **The acting identity is carried in an `x-li-actor` HTTP header, not a body
   field.** Value is base64 of a query-string form:

   ```
   x-li-actor: b3JnYW5pemF0aW9uSWQ9MTEwNDMyNjc1
   # base64-decodes to: organizationId=110432675
   ```

   So acting-as-page = `x-li-actor: base64("organizationId=<id>")`. The
   `organizationActorUrn` body field is the legacy voyager mechanism; the SDUI
   endpoint uses the header.

**RESOLVED 26-08-2026 — the voyager mutation cannot post as a company page.**
Tested with the full mechanism (`x-li-actor` header + `organizationActor` query
param + `organizationActorUrn` body, all three at once) on a **fresh, un-throttled
post** (Dementiy Besarab, `urn:li:activity:7496146635555733504`, 6 days old,
never previously touched). Result: HTTP 500 `DataFetchingException: Internal
error fetching data from downstream`, `createSocialDashNormComments: null`.
Because the post was fresh, this is **not** the write-throttle below — it is
structural. Summary of every combination tried:

| Payload | Result |
|---|---|
| body `organizationActorUrn: urn:li:organization:<id>` only | 500 downstream error |
| body `organizationActorUrn: urn:li:fsd_company:<id>` only | 403 `UnauthorizedUrnException: Failed to decorate the URN` |
| body + `organizationActor` param + `x-li-actor` header (organization form) | 500 downstream error |

Conclusion: company-page comments go **only** through the SDUI endpoint
(`com.linkedin.sdui.comments.createComment`, item 1 above), whose body carries
server-generated render-state keys a headless client cannot fabricate. There is
**no voyager path** for acting-as-page comments. The `--as-org` flag and its
`x-li-actor`/`organizationActor`/`organizationActorUrn` plumbing are retained in
the code (they are correct against the captured web *identity* encoding, and
harmless for member comments), but do not expect them to succeed against
voyager. Commenting as a member (no `--as-org`) is unaffected. For automated
page comments, the only viable channel is the official LinkedIn Community
Management API (developer app + OAuth), not the internal voyager surface.

**Write-throttle observation (26-08-2026):** after ~5 failed comment-create
attempts on one post in a short window (2 browser, 3 voyager API), *even a
genuine manual comment from the browser stopped persisting* — the create request
fired but the post's comment count never incremented across multiple reloads.
Reactions still persisted. This looks like a per-post/per-session comment-write
soft-block, not a payload problem. The lesson mirrors the beads
receipts-that-lie rule: verify the effect (fresh reload), and back off rather
than retry into an anti-abuse surface.

### Notes

- The operation name follows the Dash convention: `CreateSocialDashNormComments`
  (PascalCase of the hashMap key `createSocialDashNormComments`).
- Media attachments on comments use the `mediaUnion` field with either an
  `article` or `vectorUrn` variant.
- Replying to a comment (nested reply) likely uses the comment's URN as
  `threadUrn` instead of the post URN, but this is not yet confirmed.
- The `origin` field is optional but the Android app always sends it.

## Fetch Comments (for reference)

Comments on a post are fetched via the FINDER query `socialDashCommentsBySocialDetail`:

```
GET /voyager/api/graphql?variables=(socialDetailUrn:...,count:10,start:0)&queryId=voyagerSocialDashComments.59bca422f480a4cc0ce56ccd81181488
```

The `socialDetailUrn` is NOT the activity URN -- it's the `urn:li:fsd_socialDetail:...`
URN found in the feed response's social metadata. This is a different URN format
that maps 1:1 to an activity but uses a different namespace.
