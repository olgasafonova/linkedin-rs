# Invitations (Connection Requests)

## Overview

LinkedIn's invitation system handles connection requests between members.
The international Android app uses the Dash/GraphQL surface for listing
invitations and the Dash REST endpoint for accepting/ignoring them.

## Listing Pending Invitations

### Endpoint

**Dash GraphQL finder**: `voyagerRelationshipsDashInvitationViews`

```
GET /voyager/api/graphql?variables=(start:{N},count:{N},includeInsights:true)&queryId=voyagerRelationshipsDashInvitationViews.48949225027e0a85d063176777f08e7f&queryName=ReceivedInvitationViews
```

### Source

Discovered in `MynetworkGraphQLClient.receivedInvitationViews()` in the
decompiled international APK (`com.linkedin.android.mynetwork.graphql`).

```java
Query m = FieldSet$$ExternalSyntheticOutline0.m(
    "voyagerRelationshipsDashInvitationViews.48949225027e0a85d063176777f08e7f",
    "ReceivedInvitationViews"
);
m.operationType = "FINDER";
m.setVariable(num, "count");
m.setVariable(Boolean.TRUE, "includeInsights");
m.setVariable(list, "invitationTypes");      // optional filter
m.setVariable(str, "paginationToken");       // optional cursor
m.setVariable(num2, "start");
```

### Variables

| Variable | Type | Required | Description |
|----------|------|----------|-------------|
| `start` | Integer | Yes | Pagination offset (0-based) |
| `count` | Integer | Yes | Page size |
| `includeInsights` | Boolean | Yes | Always `true` to get connection insights |
| `invitationTypes` | List | No | Filter by invitation type (CONNECTION, etc.) |
| `paginationToken` | String | No | Cursor-based pagination token |

### Response Shape

```
data.relationshipsDashInvitationViewsByReceived: {
  elements: [InvitationView],
  paging: { start, count, total }
}
```

Each `InvitationView` contains:
- `title.text` -- inviter's display name
- `subtitle.text` -- inviter's headline/occupation
- `sentTimeLabel` -- human-readable time string (e.g. "2 days ago")
- `typeLabel` -- invitation type label
- `invitation` -- nested `Invitation` object with:
  - `entityUrn` -- invitation URN (`urn:li:fsd_invitation:{id}`)
  - `sharedSecret` -- CSRF token for accept/ignore actions
  - `genericInvitationType` -- enum: `CONNECTION`, `EVENT`, `ORGANIZATION`, etc.
  - `inviter` -- profile of the person who sent the invitation
  - `message` -- optional custom message from inviter
  - `sentTime` -- epoch milliseconds timestamp
  - `invitationId` -- numeric invitation ID
  - `trackingId` -- base64-encoded tracking ID

### Models

From decompiled `InvitationView.java` and `Invitation.java`:
- Package: `com.linkedin.android.pegasus.dash.gen.voyager.dash.relationships.invitation`
- `InvitationView` wraps display-ready fields plus nested `Invitation`
- `Invitation` has full metadata including `sharedSecret` required for actions

## Accepting an Invitation

### Endpoint

**Dash REST action**: `voyagerRelationshipsDashInvitations`

```
POST /voyager/api/voyagerRelationshipsDashInvitations/{invitation_urn}?action=accept
```

### Source

Discovered in `InvitationActionsRepository.Companion.buildInvitationActionRoute()`:

```java
public static String buildInvitationActionRoute(Urn urn, String str) {
    return RestliUtils.appendRecipeParameter(
        Routes.GROWTH_DASH_INVITATIONS.buildUponRoot()
            .buildUpon()
            .appendEncodedPath(urn.toString())
            .appendQueryParameter("action", str)
            .build(),
        "com.linkedin.voyager.dash.deco.relationships.InvitationInviterInfo-4"
    ).toString();
}
```

The `action` parameter can be: `accept`, `ignore`, `withdraw`, `reportSpam`.

### Request Body

```json
{
  "invitationUrn": "urn:li:fsd_invitation:7312345678901234567",
  "sharedSecret": "abcdef123456"
}
```

The `sharedSecret` is obtained from the invitation list response and is
required as a CSRF protection measure.

### Action Types

From `InvitationActionManager.ActionType` enum:
- `ACCEPT` -- accept the invitation
- `IGNORE` -- decline without notifying
- `REJECT` -- decline (may notify)
- `WITHDRAW` -- cancel a sent invitation
- `SEND` -- send a new invitation
- `BATCH_SEND` -- send multiple invitations

## Legacy Endpoints

The older REST endpoints are documented in `api_endpoint_catalog.md`:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `relationships/invitations?start=N&count=N` | GET | List invitations |
| `relationships/invitations/{id}?action=accept` | POST | Accept (legacy) |
| `relationships/invitationViews?q=receivedInvitation` | GET | Invitation views |

These may still work but the international build has migrated to Dash/GraphQL.

## CLI Usage

```bash
# List pending invitations
linkedin-cli connections invitations
linkedin-cli connections invitations --count 20 --start 0 --json

# Accept an invitation (id and secret from the list output)
linkedin-cli connections accept 7312345678901234567 --secret "abcdef123456"
```
