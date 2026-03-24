---
id: TASK-0034
title: Fix messaging endpoint (HTTP 500)
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 09:22'
updated_date: '2026-03-24 09:48'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GET /voyager/api/messaging/conversations returns 500. The endpoint path or query parameters are wrong. Investigate the decompiled code for the correct messaging endpoint — may need a different path (e.g., messaging/threads, messaging/dash/messengerConversations) or required query parameters (q= finder, decorationId). Check the international Voyager decompiled code at decompiled/jadx_intl/ since that's the build we're targeting.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Messaging endpoint returns 200 with conversation data
- [x] #2 CLI messages list shows conversations correctly
- [x] #3 Response saved as fixture in secrets/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Fix get_conversations() - change from start/count to createdBefore/count cursor pagination (matching Zephyr MessagingRoutes.addConversationsParams)
2. Fix get_conversation_events() - change from start/count to createdBefore/count cursor pagination
3. Test with live API using linkedin-cli
4. Fix CLI display code if response structure changed
5. Save working response to secrets/messaging_response.json
6. Run e2e tests
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Root cause: REST endpoint messaging/conversations returns HTTP 500 (deprecated server-side for international builds)
- Solution: Migrated to Voyager GraphQL endpoint using MessengerGraphQLClient queries from decompiled code
- Key discovery: URN values in Rest.li variables format must be percent-encoded (colons -> %3A)
- Conversations: messengerConversationsByCategory query (queryId: voyagerMessagingDashMessengerConversations.7dc50d3efc3953190125aca9c05f0af6)
- Messages: messengerMessagesByConversation query (queryId: voyagerMessagingDashMessengerMessages.7cde5843a127bbecc3de900d3894a74a)
- Required x-li-graphql-pegasus-client: true header for GraphQL requests
- mailboxUrn uses urn:li:fsd_profile format (dashEntityUrn from /me)
- conversationUrn uses urn:li:msg_conversation format
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed messaging endpoint HTTP 500 by migrating from the deprecated REST messaging/conversations endpoint to LinkedIn's Voyager GraphQL API.\n\nChanges:\n- linkedin-api/src/client.rs: Added graphql_get() method for GraphQL queries with x-li-graphql-pegasus-client header. Rewrote get_conversations() and get_conversation_events() to use MessengerGraphQLClient queries (messengerConversationsByCategory, messengerMessagesByConversation) discovered in the international APK decompilation.\n- linkedin-cli/src/main.rs: Changed pagination from offset-based (--start) to cursor-based (--before) for messages subcommands. Added print_graphql_conversation() and print_graphql_message() display functions for the new GraphQL response format. Removed old REST-specific display functions.\n\nKey technical discoveries:\n- The messaging/conversations REST endpoint is dead (HTTP 500) for international builds\n- LinkedIn GraphQL uses Rest.li variable encoding where URN colons must be percent-encoded\n- mailboxUrn requires urn:li:fsd_profile format from miniProfile.dashEntityUrn\n- conversationUrn requires urn:li:msg_conversation:(profileUrn,threadId) format\n\nTests: just e2e passes (35 unit + 2 smoke + clippy + fmt)
<!-- SECTION:FINAL_SUMMARY:END -->
