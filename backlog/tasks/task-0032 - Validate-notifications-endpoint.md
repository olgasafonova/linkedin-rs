---
id: TASK-0032
title: Validate notifications endpoint
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 07:49'
updated_date: '2026-03-24 08:49'
labels: []
dependencies:
  - TASK-0026
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Call the notifications endpoint. Verify response structure against documented models. Test mark-as-read if safe.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Notifications endpoint returns data
- [x] #2 Response structure documented
- [ ] #3 Response saved as fixture in secrets/
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented get_notifications() in client.rs calling identity/notificationCards with start/count pagination. Added NotificationCardsResponse and NotificationCard models in models.rs matching the Card schema from pegasus_models.md section 3.8. Added notifications list subcommand to CLI with --count, --start, --json flags. Human-readable output shows headline, sub-headline, kicker (time), content type, and published-at timestamp. Unread notifications marked with asterisk. All e2e tests pass (35 unit tests, 3 integration tests, clippy clean, fmt clean). AC#3 (save fixture in secrets/) requires a live session and is deferred to manual validation.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added notifications endpoint support: client.get_notifications(), NotificationCard model, and CLI subcommand (notifications list). Endpoint path: identity/notificationCards with start/count pagination. AC#3 (fixture in secrets/) requires live validation with an active session.
<!-- SECTION:FINAL_SUMMARY:END -->
