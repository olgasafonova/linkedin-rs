---
id: TASK-0042
title: React to a post (like/celebrate/etc)
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 13:21'
updated_date: '2026-03-24 13:54'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement liking/reacting to feed posts. Need to find the reaction endpoint — likely a POST to a socialActions or reactions Dash endpoint. Check decompiled code for reaction types (LIKE, CELEBRATE, LOVE, INSIGHTFUL, FUNNY, SUPPORT).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Reaction endpoint identified
- [x] #2 CLI 'feed react <post_urn> [--type like]' command works
- [x] #3 Documented in re/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add react_to_post() method to client.rs using REST Dash endpoint POST voyagerSocialDashReactions?action=create
2. Add unreact_from_post() method for removing reactions (DELETE endpoint)
3. Add FeedAction::React subcommand to CLI
4. Document findings in re/reactions.md
5. Run just e2e to verify build/lint/test
6. Test against live API
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Discovered reaction endpoints in FeedFrameworkGraphQLClient.java: CREATE, DELETE, UPDATE query IDs
- ReactionType enum has 11 values (LIKE, PRAISE, EMPATHY, INTEREST, APPRECIATION, ENTERTAINMENT, CELEBRATION, etc.)
- Captured live web traffic via Chrome DevTools MCP: web uses SDUI proxy, not direct Voyager API
- Key discovery: GraphQL mutations use POST to /voyager/api/graphql?action=execute (from GraphQLMutationRequestBuilder.fillInQueryParams())
- CREATE mutation needs both top-level variables AND entity wrapper; DELETE only needs top-level variables
- Tested all reaction types against live API: create LIKE, create CELEBRATION, delete both - all working
- Added graphql_post() method to client.rs for mutation support (reusable for future mutations)
- Also added unreact command as bonus feature
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented feed post reactions (like/celebrate/etc.) for the LinkedIn CLI.

Changes:
- Added `graphql_post()` to `linkedin-api/src/client.rs` -- general-purpose GraphQL mutation method using POST to `/voyager/api/graphql?action=execute`. Discovered that mutations differ from queries: they POST JSON body with queryId/queryName/variables instead of URL-encoding variables.
- Added `react_to_post()` and `unreact_from_post()` methods using the `voyagerSocialDashReactions` GraphQL mutations (query IDs from decompiled FeedFrameworkGraphQLClient.java).
- Added `feed react <post_urn> [--type LIKE]` and `feed unreact <post_urn> [--type LIKE]` CLI subcommands with validation of reaction types.
- Documented full endpoint analysis in `re/reactions.md` including mutation protocol, variable formats, reaction type enum, and SDUI vs Voyager differences.

Tested against live API:
- Created LIKE and CELEBRATION reactions -- verified via feed list that like count incremented
- Removed reactions -- verified count decremented
- All 7 reaction types validated: LIKE, PRAISE, EMPATHY, INTEREST, APPRECIATION, ENTERTAINMENT, CELEBRATION

E2E gate: build + test + clippy + fmt all pass.
<!-- SECTION:FINAL_SUMMARY:END -->
