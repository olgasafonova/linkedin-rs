---
id: TASK-0036
title: Fix profile view endpoint (HTTP 400)
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 09:22'
updated_date: '2026-03-24 10:05'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GET /voyager/api/identity/profiles/{id}?decorationId=com.linkedin.voyager.deco.identity.FullProfile returns 400. The decoration recipe name is likely wrong or the endpoint has moved. Check decompiled/jadx_intl/ for the correct profile endpoint and decoration recipe — may need a versioned recipe suffix or a Dash endpoint.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Profile view endpoint returns 200 with profile data
- [x] #2 CLI profile view shows profile correctly
- [x] #3 Response saved as fixture in secrets/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Replace legacy REST profile endpoint with GraphQL Dash endpoint
2. Use identityDashProfilesByMemberIdentity finder (voyagerIdentityDashProfiles.5f50f83f76a1e270603613bdd0fb0252) from ProfileGraphQLClient
3. Variable: memberIdentity = public_id (vanity URL slug)
4. Test against live API with --json
5. Update CLI display for new response shape
6. Save response fixture to secrets/
7. Run e2e tests
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Confirmed legacy REST endpoint identity/profiles/{id}?decorationId=... returns HTTP 400
- Decompiled ProfileGraphQLClient.java reveals profile fetching moved to GraphQL Dash endpoint
- identityDashProfilesByMemberIdentity finder with queryId voyagerIdentityDashProfiles.5f50f83f76a1e270603613bdd0fb0252
- Variable: memberIdentity = public vanity URL slug
- Updated get_profile() in client.rs to use GraphQL endpoint with envelope unwrapping
- Updated print_profile_summary() in main.rs for Dash response shape:
  - profilePositionGroups.elements[].profilePositionInPositionGroup.elements[] for positions
  - profileEducations.elements[] for education
  - geoLocation.geo.defaultLocalizedName for location
  - industry.name for industry
  - dateRange with start/end instead of timePeriod with startDate/endDate
- All e2e tests pass (35 unit + 2 smoke + clippy + fmt)
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed profile view endpoint that was returning HTTP 400.

Root cause: The legacy REST endpoint identity/profiles/{id}?decorationId=com.linkedin.voyager.deco.identity.FullProfile has been deprecated server-side. Profile fetching has migrated to the Dash/GraphQL surface in the international LinkedIn Android build.

Changes:
- linkedin-api/src/client.rs: Replaced get_profile() REST call with GraphQL identityDashProfilesByMemberIdentity finder query (queryId from decompiled ProfileGraphQLClient.java). Unwraps the GraphQL envelope to return the profile object directly.
- linkedin-cli/src/main.rs: Updated print_profile_summary() for the Dash response shape (profilePositionGroups instead of positions, profileEducations instead of educations, dateRange instead of timePeriod, geoLocation.geo.defaultLocalizedName for location). Renamed format_time_period to format_date_range with backwards-compatible support for both start/end and startDate/endDate keys.

Tests:
- Live API test: profile view ruvald returns 200 with full profile data
- Response saved to secrets/profile_response.json (3081 lines)
- All e2e tests pass (35 unit + 2 smoke + clippy + fmt)
<!-- SECTION:FINAL_SUMMARY:END -->
