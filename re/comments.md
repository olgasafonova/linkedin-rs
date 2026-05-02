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

### Notes

- The operation name follows the Dash convention: `CreateSocialDashNormComments`
  (PascalCase of the hashMap key `createSocialDashNormComments`).
- Media attachments on comments use the `mediaUnion` field with either an
  `article` or `vectorUrn` variant.
- Replying to a comment (nested reply) uses the same mutation with the parent
  comment URN as `entity.threadUrn` instead of the post URN. The CLI exposes
  this as `linkedin-cli feed reply '<comment_urn>' '<text>' --yes --json`.
- The `origin` field is optional but the Android app always sends it.
- GraphQL mutation errors can be returned as HTTP 200 with `value.errors`; the
  client treats those as API errors so failed comments/replies exit non-zero.

## Fetch Comments

Comments on a post are fetched via the FINDER query `socialDashCommentsBySocialDetail`:

```
GET /voyager/api/graphql?variables=(socialDetailUrn:...,count:10,start:0,sortOrder:RELEVANCE)&queryId=voyagerSocialDashComments.59bca422f480a4cc0ce56ccd81181488&queryName=SocialDashCommentsBySocialDetail
```

The `socialDetailUrn` is NOT the activity URN -- it's the `urn:li:fsd_socialDetail:...`
URN found in the feed response's `socialDetail.dashEntityUrn`. This is a different
URN format that maps 1:1 to an activity but uses a different namespace.

CLI:

```bash
linkedin-cli feed comments 'urn:li:fsd_socialDetail:(...)' --count 20 --json
```

Live-validated on 2026-05-02 through Proton SG proxy: returned `elements`, `metadata`,
and `paging` for a group post social detail URN.

## Fetch Replies

Replies use the FINDER query `socialDashCommentsByRepliesByCursor`:

```
GET /voyager/api/graphql?variables=(commentUrn:...,count:10,cursor:...)&queryId=voyagerSocialDashComments.8ada653d14b465e4f86d3ed7dcbe6695&queryName=SocialDashCommentsByRepliesByCursor
```

This endpoint requires a non-null `cursor` variable. Calling it without a cursor returns:

```text
Variable 'cursor' has coerced Null value for NonNull type 'String!'
```

CLI:

```bash
linkedin-cli feed replies 'urn:li:fsd_comment:(...)' --cursor '<replyCursor>' --count 20 --json
```

## Fetch Single Comment

Single-comment fetch uses `socialDashCommentsBySingleComment`:

```
GET /voyager/api/graphql?variables=(commentUrn:...,updateThreadUrn:...)&queryId=voyagerSocialDashComments.a84e91d6baaa2d2018fdc49f21541de5&queryName=SocialDashCommentsBySingleComment
```

The `updateThreadUrn` variable is required; calling without it returns:

```text
Variable 'updateThreadUrn' has coerced Null value for NonNull type 'String!'
```

CLI:

```bash
linkedin-cli feed comment-get 'urn:li:fsd_comment:(...)' --thread-urn 'urn:li:activity:...' --json
```
