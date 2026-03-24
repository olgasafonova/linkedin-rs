---
id: TASK-0037
title: Fix notifications endpoint (HTTP 404)
status: To Do
assignee: []
created_date: '2026-03-24 09:22'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GET /voyager/api/identity/notificationCards returns 404 (HTML error page, not JSON). The endpoint path is wrong. Check decompiled/jadx_intl/ for the correct notifications endpoint — may need a different path or query parameters. The international build likely uses a Dash endpoint.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Notifications endpoint returns 200 with notification data
- [ ] #2 CLI notifications list shows notifications correctly
- [ ] #3 Response saved as fixture in secrets/
<!-- AC:END -->
