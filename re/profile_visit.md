# Profile Visit (Register View) Mechanism

## Summary

When a LinkedIn user views another person's profile, the visit is registered
server-side so the target sees the viewer in "Who Viewed My Profile". There is
**no separate POST/PUT endpoint** for registering profile views. The view is
recorded as a side effect of fetching the profile via specific GraphQL query
decorations.

## Discovery Method

Captured network traffic via Chrome DevTools MCP (2026-03-24) by navigating to
`https://www.linkedin.com/in/satyanadella/` while authenticated.

## Web Client Behavior

When loading a profile page, the web client fires multiple
`voyagerIdentityDashProfiles` GraphQL queries in parallel, each with a
different `queryId` (decoration) and `x-li-pem-metadata` tag:

| queryId hash | x-li-pem-metadata | Purpose |
|---|---|---|
| `a3de77c32c473719f1c58fae6bff43a5` | `Voyager - Profile=profile-top-card-supplementary` | Top card with actions, positions, education; **registers the view** |
| `2ca312bdbe80fac72fd663a3e06a83e7` | `Voyager - Profile=profile-tab` | Tab/locale data (lightweight) |

Both use `vanityName` as the variable (the public URL slug), not
`memberIdentity` (the opaque member ID used by the mobile app).

### Key Request Details

```
GET /voyager/api/graphql?includeWebMetadata=true
    &variables=(vanityName:satyanadella)
    &queryId=voyagerIdentityDashProfiles.a3de77c32c473719f1c58fae6bff43a5

Headers:
  x-li-page-instance: urn:li:page:d_flagship3_profile_view_base;{random_base64}==
  x-li-pem-metadata: Voyager - Profile=profile-top-card-supplementary
  x-restli-protocol-version: 2.0.0
  csrf-token: ajax:{jsessionid}
  accept: application/vnd.linkedin.normalized+json+2.1
```

### Response

Returns a normalized JSON response with `included` entities containing:
- `com.linkedin.voyager.dash.identity.profile.Profile` (full profile data)
- `com.linkedin.voyager.dash.identity.profile.Position` (positions)
- `com.linkedin.voyager.dash.identity.profile.Education` (education)
- `com.linkedin.voyager.dash.relationships.MemberRelationship` (connection status)
- `com.linkedin.voyager.dash.feed.FollowingState`

## Mobile App Behavior

Our existing `get_profile()` uses the mobile app's query ID
(`5f50f83f76a1e270603613bdd0fb0252`) with `memberIdentity` variable. This
fetches profile data but may not register the view since it uses a different
decoration.

## DiscloseAsProfileViewerInfo Enum

Found in the decompiled APK at:
`com.linkedin.android.pegasus.dash.gen.voyager.dash.identity.profile.DiscloseAsProfileViewerInfo`

Values:
- `DISCLOSE_FULL` -- Show full viewer identity (default for most users)
- `DISCLOSE_ANONYMOUS` -- Show as anonymous viewer (Premium feature)
- `HIDE` -- Don't appear at all (Premium feature)

This enum is part of `PrivacySettings` and controls the **viewer's own
disclosure preference**, not the view registration mechanism. The view is
registered regardless of this setting; the setting controls what the target
sees about the viewer.

## Implementation

`visit_profile(public_id)` uses the web query ID
(`a3de77c32c473719f1c58fae6bff43a5`) with `vanityName` variable to trigger
server-side view registration.

CLI: `linkedin-cli profile visit <public_id>`

## Comparison: get_profile vs visit_profile

| | get_profile | visit_profile |
|---|---|---|
| Variable | `memberIdentity` | `vanityName` |
| Query ID | `5f50f83f76a1e270603613bdd0fb0252` (mobile) | `a3de77c32c473719f1c58fae6bff43a5` (web) |
| Registers view | Uncertain (mobile decoration) | Yes (web profile-top-card decoration) |
| Returns | Full profile data | Full profile data with actions |
