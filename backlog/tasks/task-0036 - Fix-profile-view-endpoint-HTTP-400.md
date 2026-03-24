---
id: TASK-0036
title: Fix profile view endpoint (HTTP 400)
status: To Do
assignee: []
created_date: '2026-03-24 09:22'
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
- [ ] #1 Profile view endpoint returns 200 with profile data
- [ ] #2 CLI profile view shows profile correctly
- [ ] #3 Response saved as fixture in secrets/
<!-- AC:END -->
