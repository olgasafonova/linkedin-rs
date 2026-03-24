---
id: TASK-0047
title: Comment on a post
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 13:21'
updated_date: '2026-03-24 14:56'
labels: []
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement commenting on feed posts. Need to find the comment creation endpoint — likely under socialActions or comments Dash endpoint.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Comment endpoint identified
- [x] #2 CLI 'feed comment <post_urn> <text>' works
- [x] #3 Documented in re/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add comment_on_post() to linkedin-api client.rs (GraphQL mutation using createSocialDashNormComments)
2. Add Comment subcommand to FeedAction enum in linkedin-cli main.rs
3. Add cmd_feed_comment() handler function
4. Create re/comments.md documentation
5. Run just e2e to verify
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Found createSocialDashNormComments mutation in ConversationsGraphQLClient.java
- queryId: voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e
- NormCommentForUpdate model has fields: commentary, threadUrn, origin, organizationActorUrn, mediaUnion, timeOffset, tscpUrl
- Minimum payload: entity.commentary.text + entity.threadUrn + entity.origin=FEED
- Added comment_on_post() to linkedin-api client.rs
- Added Comment variant to FeedAction enum with --yes safety guard
- Created re/comments.md with full endpoint catalog and model docs
- All e2e tests pass (build, test, lint, fmt-check)
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added comment-on-post feature to the LinkedIn API client and CLI.

Changes:
- linkedin-api/src/client.rs: Added comment_on_post() method using the createSocialDashNormComments GraphQL mutation (queryId: voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e) discovered in ConversationsGraphQLClient.java. Sends entity with commentary.text, threadUrn, and origin=FEED.
- linkedin-cli/src/main.rs: Added feed comment <post_urn> <text> [--yes] [--json] subcommand with --yes safety guard matching the existing feed post pattern.
- re/comments.md: Full endpoint documentation covering all 7 comment-related GraphQL operations, NormComment data model fields, CommentOrigin enum, TextViewModelForCreate structure, and example payloads.

Not tested against live API per task instructions. The --yes guard prevents accidental real comments.
<!-- SECTION:FINAL_SUMMARY:END -->
