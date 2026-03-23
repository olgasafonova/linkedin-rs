---
id: TASK-0007
title: Map complete API endpoint catalog from decompiled code
status: To Do
assignee: []
created_date: '2026-03-23 23:00'
labels:
  - phase3
  - static-analysis
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract all API endpoints from the decompiled code beyond what Routes.java provides. Search for URL construction patterns in DataProvider classes, direct URL strings in feature packages (feed, messaging, jobs, identity, search, etc.), and any dynamically constructed endpoints. Produce a comprehensive endpoint catalog in re/api_endpoints.md with HTTP methods, parameters, request/response models, and decoration IDs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Complete endpoint catalog documented in re/api_endpoints.md
- [ ] #2 Endpoints organized by domain (identity, feed, messaging, jobs, search, etc.)
- [ ] #3 Each endpoint includes HTTP method, path, query parameters, and linked Pegasus model
<!-- AC:END -->
