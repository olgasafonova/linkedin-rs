---
id: TASK-0041
title: Register profile view (visit a profile as viewer)
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 13:21'
updated_date: '2026-03-24 13:32'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When the real LinkedIn app views a profile, it registers the visit so the target sees you in their 'who viewed my profile'. Our profile view command fetches data but likely doesn't register the visit. Investigate DiscloseAsProfileViewerInfo enum (DISCLOSE_FULL, DISCLOSE_ANONYMOUS, HIDE) and find the mechanism — may be automatic with the right decoration, or a separate POST endpoint. Use Chrome DevTools MCP to capture what the web app sends when visiting a profile.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Mechanism for registering profile views identified
- [x] #2 CLI 'profile visit <id>' command triggers a real profile view
- [x] #3 Documented in re/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze web browser network traffic for profile view registration mechanism
2. Finding: No separate POST endpoint. Profile views are registered server-side when fetching the profile via the web GraphQL query (vanityName-based, queryId voyagerIdentityDashProfiles.a3de77c32c473719f1c58fae6bff43a5). The x-li-page-instance header with d_flagship3_profile_view_base marks it as a page view.
3. Implement visit_profile() in client.rs using the web query ID (a3de77c32c473719f1c58fae6bff43a5) with vanityName variable
4. Add profile visit <public_id> CLI subcommand
5. Document findings in re/profile_visit.md
6. Run e2e tests
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Captured web browser network traffic via Chrome DevTools MCP navigating to /in/satyanadella/
- Found no separate POST endpoint for registering profile views
- View registration is a server-side side effect of the GraphQL profile fetch
- Web uses queryId a3de77c32c473719f1c58fae6bff43a5 with vanityName variable
- Our existing get_profile uses mobile queryId 5f50f83f76a1e270603613bdd0fb0252 with memberIdentity
- Implemented visit_profile() using the web query ID in client.rs
- Added profile visit <public_id> CLI subcommand
- Documented in re/profile_visit.md
- All 41 tests pass, clippy clean, fmt check passes
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added profile visit command that registers the viewer in the target's "Who Viewed My Profile".\n\nMechanism: LinkedIn has no separate POST endpoint for view registration. The profile view is recorded server-side as a side effect of the GraphQL profile fetch when using the web client's query decoration (queryId a3de77c32c473719f1c58fae6bff43a5 with vanityName variable). This was discovered by capturing Chrome DevTools network traffic while navigating to a profile.\n\nChanges:\n- linkedin-api/src/client.rs: Added visit_profile() using the web query ID\n- linkedin-cli/src/main.rs: Added profile visit <public_id> subcommand with --json flag\n- re/profile_visit.md: Documented the mechanism, request/response format, and comparison with get_profile\n\nTests: just e2e passes (41 tests, clippy, fmt)
<!-- SECTION:FINAL_SUMMARY:END -->
