---
id: TASK-0034
title: Fix messaging endpoint (HTTP 500)
status: To Do
assignee: []
created_date: '2026-03-24 09:22'
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
- [ ] #1 Messaging endpoint returns 200 with conversation data
- [ ] #2 CLI messages list shows conversations correctly
- [ ] #3 Response saved as fixture in secrets/
<!-- AC:END -->
