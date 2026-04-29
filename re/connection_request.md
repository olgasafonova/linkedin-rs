# Connection Request (Invitation) Endpoint Analysis

## Summary

As of **29-04-2026** LinkedIn's live web client sends connection invitations
through the Dash `MemberRelationships` endpoint with action
`verifyQuotaAndCreateV2`. The legacy `voyagerGrowthNormInvitations`
endpoint (used by older Android builds and the previous version of this
crate) now returns HTTP 301 — treat it as retired.

The Dash *invitations* endpoint anticipated by the international APK
(`voyagerRelationshipsDashInvitations?action=create`) is *not* what the
modern web client uses. The contract below was captured from the live
flow at `linkedin.com/preload/custom-invite/?vanityName=<vanity>` (Send
without a note → POST observed in browser network capture).

## Modern endpoint (live, used by this crate)

```text
POST /voyager/api/voyagerRelationshipsDashMemberRelationships
  ?action=verifyQuotaAndCreateV2
  &decorationId=com.linkedin.voyager.dash.deco.relationships.InvitationCreationResultWithInvitee-2
```

The action name suggests LinkedIn enforces the weekly invitation quota at
the same call (the V2 suffix indicates this is the second iteration of the
combined verify + create flow).

### Request body

```json
{
  "invitee": {
    "inviteeUnion": {
      "memberProfile": "urn:li:fsd_profile:ACoAAA..."
    }
  }
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `invitee.inviteeUnion.memberProfile` | string (full URN) | Yes | Full `urn:li:fsd_profile:` URN — *not* the bare member ID the legacy endpoint required |
| `message` | string | No | Placement on the modern endpoint is **unverified** — the smoke capture was a "Send without a note" flow. The crate currently attaches the field at the top level as a best-effort carry-over from the legacy contract |

No `trackingId` is sent. No fully-qualified Java type key is needed in the
inner union (the legacy endpoint required `com.linkedin.voyager.growth.invitation.InviteeProfile`).

### Response

LinkedIn returns an `InvitationCreationResultWithInvitee-2` decorated
entity (per the `decorationId` query parameter).

## Retired endpoints (historical)

### Legacy (Growth / normInvitations) — retired 29-04-2026

```text
POST /voyager/api/voyagerGrowthNormInvitations
```

## Endpoints

### Legacy (Growth / normInvitations)

```
POST /voyager/api/voyagerGrowthNormInvitations
```

Discovered in the **China APK** decompiled code:
- `InvitationNetworkUtil.sendInvite()` calls
  `MyNetworkRoutesUtil.makeSendGrowthInvitationRoute()` which resolves to
  `Routes.NORM_INVITATIONS` = `"voyagerGrowthNormInvitations"`
- `MyNetworkRequestUtil.buildInvitation()` constructs the `NormInvitation` model

Source files:
- `decompiled/jadx/sources/com/linkedin/android/mynetwork/shared/network/InvitationNetworkUtil.java`
- `decompiled/jadx/sources/com/linkedin/android/mynetwork/shared/network/MyNetworkRoutesUtil.java`
- `decompiled/jadx/sources/com/linkedin/android/mynetwork/shared/network/MyNetworkRequestUtil.java`

### Dash (International APK)

```
POST /voyager/api/voyagerRelationshipsDashInvitations?action=create
```

Discovered in the **international APK** decompiled code:
- `InvitationActionsRepository.Companion.buildInvitationActionRoute()` uses
  `Routes.GROWTH_DASH_INVITATIONS` = `"voyagerRelationshipsDashInvitations"`
  with `?action=create` query parameter
- `InvitationActionManagerImpl.sendInvite()` (obfuscated to `.n()`) performs
  the actual network call

Source files:
- `decompiled/jadx_intl/sources/com/linkedin/android/mynetwork/invitations/InvitationActionsRepository.java`
- `decompiled/jadx_intl/sources/com/linkedin/android/mynetwork/invitations/InvitationActionManagerImpl$$ExternalSyntheticLambda3.java`
- `decompiled/jadx_intl/sources/com/linkedin/android/mynetwork/relationship/RelationshipBuildingActionHandler.java`

### Batch Create

```
POST /voyager/api/voyagerGrowthNormInvitations?action=batchCreate
```

Body: `{ "invitations": [ <NormInvitation>, ... ] }`

## Request Body (NormInvitation)

The `NormInvitation` model is a Rest.li record with a union-typed `invitee` field:

```json
{
  "trackingId": "<base64-encoded-16-random-bytes>",
  "invitee": {
    "com.linkedin.voyager.growth.invitation.InviteeProfile": {
      "profileId": "<member-id>"
    }
  },
  "message": "optional custom message (max ~300 chars)"
}
```

### Field Details

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `trackingId` | string | Yes | Base64-encoded 16 random bytes (`TrackingUtils.generateBase64EncodedTrackingId()`) |
| `invitee` | union | Yes | Rest.li union; key is the fully-qualified Java type name |
| `invitee.profileId` | string | Yes | The member ID (last segment of `urn:li:fsd_profile:ACoAAA...`) |
| `message` | string | No | Custom invitation message; LinkedIn enforces a ~300 char limit server-side |

### Invitee Union Variants

The `invitee` field is a Rest.li union that supports two types:

1. **InviteeProfile** (by member ID):
   ```json
   {
     "com.linkedin.voyager.growth.invitation.InviteeProfile": {
       "profileId": "ACoAAA..."
     }
   }
   ```

2. **InviteeEmail** (by email, used for email-verified invitations):
   ```json
   {
     "com.linkedin.voyager.growth.invitation.InviteeEmail": {
       "email": "user@example.com"
     }
   }
   ```

## Response

On success, LinkedIn returns the created `NormInvitation` entity (or the Dash
`InvitationCreationResult` on the Dash endpoint).

## Related Code

- `InvitationActionManager.ActionType` enum: `SEND`, `SEND_AND_AUTOFOLLOW`,
  `BATCH_SEND`, `WITHDRAW`, `ACCEPT`, `IGNORE`, `REJECT`, `FOLLOW`, `UNFOLLOW`
- `InvitationCreateParams`: simple wrapper holding an `inviteeProfileUrn`
- `MemberRelationship` model tracks the relationship state after invitation

## Routes Reference

From `Routes.java` in the decompiled code:

| Route Constant | Path String |
|---|---|
| `NORM_INVITATIONS` | `voyagerGrowthNormInvitations` |
| `RELATIONSHIPS_NORM_INVITATIONS` | `relationships/normInvitations` |
| `GROWTH_DASH_INVITATIONS` | `voyagerRelationshipsDashInvitations` |
| `RELATIONSHIPS_DASH_INVITATIONS_SUMMARY` | `voyagerRelationshipsDashInvitationsSummary` |

## Implementation

We use `voyagerRelationshipsDashMemberRelationships?action=verifyQuotaAndCreateV2`
(see `linkedin-api/src/client.rs::send_connection_request`). The legacy
`normInvitations` endpoint stopped working on 29-04-2026; the Dash
`Invitations?action=create` route from the international APK was *not* the
replacement LinkedIn picked — the modern web client targets the
`MemberRelationships` resource directly.

### Open follow-ups

1. **With-message capture.** The smoke flow used "Send without a note", so
   the placement of the `message` field on the new endpoint is unverified.
   Capture an "Add a note" flow and either confirm the top-level placement
   or correct the body shape.
2. **Email invitee variant.** The legacy contract supported `InviteeEmail`
   for email-verified invitations. The Dash endpoint may use a different
   union member name (e.g. `emailContact` or similar). Out of scope for the
   current fix.
3. **Withdraw / cancel.** Likely also moved to the `MemberRelationships`
   resource (different action on the same path). Worth a separate capture
   when the withdraw flow is needed.
