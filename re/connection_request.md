# Connection Request (Invitation) Endpoint Analysis

## Summary

LinkedIn's Android app sends connection invitations via a REST POST to the
`normInvitations` endpoint. The international (Dash) variant uses a newer
`voyagerRelationshipsDashInvitations` endpoint with `?action=create`.

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

We use the legacy `voyagerGrowthNormInvitations` endpoint because:
1. It is simpler (no Dash decoration/recipe parameters needed)
2. It is confirmed to still work on production
3. The `NormInvitation` model structure is fully visible in the decompiled code
4. The Dash endpoint requires additional recipe parameters and uses a different
   model structure that is harder to reconstruct from obfuscated code
