---
id: TASK-0044
title: Send connection request
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 13:21'
updated_date: '2026-03-24 14:14'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement sending connection invitations. Need to find the invitation endpoint in decompiled code — likely under relationships or growth routes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Connection request endpoint identified
- [x] #2 CLI 'connections invite <public_id> [--message text]' works
- [x] #3 Documented in re/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add send_connection_request() to linkedin-api client.rs
  - Endpoint: POST /voyager/api/voyagerGrowthNormInvitations
  - Body: NormInvitation model with inviteeProfile.profileId and optional message
  - Also supports Dash endpoint as fallback: voyagerRelationshipsDashInvitations?action=create
2. Add Invite variant to ConnectionsAction enum in main.rs
  - connections invite <public_id_or_urn> [--message text] [--json]
3. Add cmd_connections_invite handler in main.rs
  - Resolve public_id to profile URN if needed
  - Extract member ID from URN
  - Call client.send_connection_request()
4. Document endpoint in re/connection_request.md
5. Run just e2e to verify compilation
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Traced invitation flow through decompiled code:
  - Old APK: InvitationNetworkUtil.sendInvite() -> POST voyagerGrowthNormInvitations
  - Intl APK: InvitationActionManagerImpl.n() -> POST voyagerRelationshipsDashInvitations?action=create
- Chose legacy normInvitations endpoint (simpler model, no Dash recipe params needed)
- NormInvitation model uses Rest.li union for invitee with FQ Java type key
- Added send_connection_request() to client.rs with base64-encoded trackingId
- Added connections invite CLI subcommand with public_id and URN resolution
- Documented endpoint analysis in re/connection_request.md
- All 43 tests pass, clippy clean, fmt clean
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added connection request (invitation) feature to the LinkedIn CLI and API library.

Endpoint discovery:
- Traced invitation flow through both China and international APK decompilations
- Identified two endpoints: legacy voyagerGrowthNormInvitations and Dash voyagerRelationshipsDashInvitations
- Chose legacy endpoint for simplicity (no Dash decoration/recipe parameters needed)
- Fully documented NormInvitation model structure including Rest.li union typing

API changes (linkedin-api/src/client.rs):
- Added send_connection_request(profile_urn, message) method
- Extracts member ID from URN, generates base64-encoded tracking ID
- Builds NormInvitation payload with Rest.li union-typed invitee field
- Supports optional custom message

CLI changes (linkedin-cli/src/main.rs):
- Added "connections invite <public_id_or_urn> [--message text] [--json]" subcommand
- Resolves public identifiers to profile URNs automatically
- Human-readable and JSON output modes

Documentation:
- Created re/connection_request.md with full endpoint analysis, model structure, routes reference

Tests: 43 unit tests pass, clippy clean, fmt clean. Not tested against live API per task instructions.
<!-- SECTION:FINAL_SUMMARY:END -->
