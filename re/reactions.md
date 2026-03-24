# Reactions (Like/Celebrate/etc.)

## Endpoint Discovery

Reactions use the Voyager GraphQL mutation endpoint, not REST.

**Source**: `FeedFrameworkGraphQLClient.java` in the decompiled international APK
(`com.linkedin.android.feed.framework.graphql`).

### Query IDs (from static initializer)

| Operation | Key in hashMap | Query ID |
|-----------|---------------|----------|
| Create reaction | `createSocialDashReactions` | `voyagerSocialDashReactions.fd68eadaf15da416b0d839e21399b763` |
| Delete reaction | `doDeleteReactionSocialDashReactions` | `voyagerSocialDashReactions.315cef4773de8e3a0ddad7655cc1685f` |
| Update reaction | `doUpdateReactionSocialDashReactions` | `voyagerSocialDashReactions.846a42c007e6a1741763e9f23956ea0b` |

### Routes.java Entry

```
PARTICIPATE_REACTIONS_DASH("voyagerSocialDashReactions")
```

## GraphQL Mutation Protocol

GraphQL mutations differ from queries in how they're sent. Discovered from
`BaseGraphQLClient.generateRequestBuilder()` and
`GraphQLMutationRequestBuilder.fillInQueryParams()`:

**Queries** (GET): Variables are Rest.li-encoded in URL query string.
```
GET /voyager/api/graphql?variables=(...)&queryId=...&queryName=...
```

**Mutations** (POST): Variables are JSON in request body, with `action=execute` in URL.
```
POST /voyager/api/graphql?action=execute&queryId=...&queryName=...
Content-Type: application/json
x-li-graphql-pegasus-client: true

{
  "queryId": "...",
  "queryName": "...",
  "variables": { ... }
}
```

The `action=execute` URL parameter is added by `GraphQLMutationRequestBuilder.fillInQueryParams()`.

## Create Reaction

```
POST /voyager/api/graphql?action=execute&queryId=voyagerSocialDashReactions.fd68eadaf15da416b0d839e21399b763&queryName=CreateSocialDashReactions
```

### Variables

The CREATE mutation expects both top-level variables AND an `entity` wrapper:

```json
{
  "queryId": "voyagerSocialDashReactions.fd68eadaf15da416b0d839e21399b763",
  "queryName": "CreateSocialDashReactions",
  "variables": {
    "threadUrn": "urn:li:activity:7442172982820524035",
    "reactionType": "LIKE",
    "entity": {
      "threadUrn": "urn:li:activity:7442172982820524035",
      "reactionType": "LIKE"
    }
  }
}
```

### Response (HTTP 200)

```json
{
  "$metadata": { "isGraphQLActionResponse": true },
  "value": {
    "data": {
      "createSocialDashReactions": {
        "resourceKey": "urn:li:fsd_reaction:(urn:li:fsd_profile:ACoAAA...,urn:li:activity:7442...,0)"
      }
    }
  }
}
```

## Delete Reaction

```
POST /voyager/api/graphql?action=execute&queryId=voyagerSocialDashReactions.315cef4773de8e3a0ddad7655cc1685f&queryName=DoDeleteReactionSocialDashReactions
```

### Variables

Only top-level variables needed (no `entity` wrapper):

```json
{
  "queryId": "voyagerSocialDashReactions.315cef4773de8e3a0ddad7655cc1685f",
  "queryName": "DoDeleteReactionSocialDashReactions",
  "variables": {
    "threadUrn": "urn:li:activity:7442172982820524035",
    "reactionType": "LIKE"
  }
}
```

### Response (HTTP 200)

```json
{
  "$metadata": { "isGraphQLActionResponse": true },
  "value": {
    "data": {
      "doDeleteReactionSocialDashReactions": {
        "result": { "__typename": "restli_common_EmptyRecord" }
      }
    }
  }
}
```

## Reaction Types

From `ReactionType.java` in `com.linkedin.android.pegasus.dash.gen.voyager.dash.feed.social`:

| Enum Value | Ordinal | User-Facing Name |
|------------|---------|------------------|
| `LIKE` | 0 | Like |
| `PRAISE` | 1 | Celebrate |
| `EMPATHY` | 4 | Love / Support |
| `INTEREST` | 5 | Insightful |
| `APPRECIATION` | 6 | Appreciate |
| `ENTERTAINMENT` | 8 | Funny |
| `CELEBRATION` | 9 | Celebrate (alt) |

Note: The `MAYBE` type (ordinal 3) exists in the enum but is removed from
the reaction menu in `ReactionMenuView.java` (`linkedList.remove(ReactionType.MAYBE)`).

## Thread URN Format

The `threadUrn` is the activity URN of the post being reacted to:
- `urn:li:activity:7442172982820524035`

This is extracted from the feed response's `entityUrn` field. The feed returns
URNs like `urn:li:fs_feedUpdate:(V2&FOLLOW_FEED,urn:li:activity:XXXXX)` -- the
inner `urn:li:activity:XXXXX` is the threadUrn.

## Actor Model

The `Reaction` model in `com.linkedin.android.pegasus.dash.gen.voyager.dash.social`
has an `actorUnion` field of type `ReactionActorForCreate`, which is a union of:
- `profileUrn` (for personal reactions)
- `companyUrn` (for company page reactions)

For the GraphQL mutation, the server infers the actor from the authenticated
session -- the actor URN does not need to be passed explicitly.

## Web Client (SDUI)

The LinkedIn web client uses SDUI (Server-Driven UI) which wraps the reaction
in a different protocol:
```
POST /flagship-web/rsc-action/actions/server-request?sduiid=com.linkedin.sdui.reactions.create
```

The SDUI payload uses a protobuf-like structure with `ReactionType_LIKE`
(prefixed) and nested `threadUrnActivityThreadUrn`. This is the web-specific
transport; the Android app uses the Voyager GraphQL endpoint directly.

## CLI Usage

```bash
# Like a post (default reaction)
linkedin-cli feed react urn:li:activity:7442172982820524035

# Celebrate a post
linkedin-cli feed react urn:li:activity:7442172982820524035 --type CELEBRATION

# Remove a reaction
linkedin-cli feed unreact urn:li:activity:7442172982820524035

# Remove a specific reaction type
linkedin-cli feed unreact urn:li:activity:7442172982820524035 --type CELEBRATION
```
