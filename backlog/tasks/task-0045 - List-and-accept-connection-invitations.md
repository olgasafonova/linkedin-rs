---
id: TASK-0045
title: List and accept connection invitations
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 13:21'
updated_date: '2026-03-24 14:25'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement listing pending connection invitations and accepting them. Check relationships/invitations or mynetwork endpoints.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 List invitations endpoint works
- [x] #2 Accept invitation endpoint works
- [x] #3 CLI 'connections invitations [--accept id]' works
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add get_invitations() to client.rs using legacy REST endpoint relationships/invitationViews?q=receivedInvitation
2. Add accept_invitation() to client.rs using Dash REST endpoint voyagerRelationshipsDashInvitations/{urn}?action=accept
3. Add Invitations and Accept subcommands to ConnectionsAction in main.rs
4. Add cmd_connections_invitations() and cmd_connections_accept() handler functions
5. Add human-readable print_invitation() display function
6. Document in re/invitations.md
7. Run just e2e to verify
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Discovered Dash GraphQL query for listing invitations from MynetworkGraphQLClient.java
  queryId: voyagerRelationshipsDashInvitationViews.48949225027e0a85d063176777f08e7f
- Discovered Dash REST accept endpoint from InvitationActionsRepository.buildInvitationActionRoute()
  POST voyagerRelationshipsDashInvitations/{urn}?action=accept
- The Invitation model has sharedSecret field required for accept CSRF protection
- Added get_invitations() and accept_invitation() to linkedin-api client.rs
- Added CLI subcommands: connections invitations and connections accept
- Human-readable output shows inviter name, headline, sent time, message, and accept command hint
- Documented endpoint analysis in re/invitations.md
- All e2e checks pass (build, test, lint, fmt-check)
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added invitation listing and acceptance to the LinkedIn CLI and API library.

API changes (linkedin-api/src/client.rs):
- get_invitations(start, count) -- fetches pending received invitations via Dash GraphQL
  query voyagerRelationshipsDashInvitationViews (ReceivedInvitationViews finder)
- accept_invitation(invitation_urn, shared_secret) -- accepts an invitation via Dash REST
  POST voyagerRelationshipsDashInvitations/{urn}?action=accept

CLI changes (linkedin-cli/src/main.rs):
- connections invitations [--count N] [--start N] [--json] -- list pending invitations
  Human-readable output shows inviter name, headline, sent time, custom message, and a
  ready-to-copy accept command with id and secret
- connections accept <id> --secret <secret> [--json] -- accept a specific invitation

Documentation:
- re/invitations.md -- endpoint analysis, request/response shapes, source references

Tests:
- nix-shell --run "just e2e" passes (build, test, lint, fmt-check)
<!-- SECTION:FINAL_SUMMARY:END -->
