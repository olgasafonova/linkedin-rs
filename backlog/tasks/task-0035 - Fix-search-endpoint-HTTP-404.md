---
id: TASK-0035
title: Fix search endpoint (HTTP 404)
status: To Do
assignee: []
created_date: '2026-03-24 09:22'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GET /voyager/api/search/hits?q=guided&guides=List(v->people)&keywords=... returns 404. The endpoint path or query format is wrong. Check decompiled/jadx_intl/ for the correct search endpoint — may need voyager/api/graphql, or voyager/api/search/dash/*, or different query parameters. The international build has 219 Dash routes that may replace legacy search.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Search endpoint returns 200 with results
- [ ] #2 CLI search people shows results correctly
- [ ] #3 Response saved as fixture in secrets/
<!-- AC:END -->
