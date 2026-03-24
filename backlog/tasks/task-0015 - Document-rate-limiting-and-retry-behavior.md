---
id: TASK-0015
title: Document rate limiting and retry behavior
status: To Do
assignee: []
created_date: '2026-03-24 06:29'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Search decompiled code for HTTP 429 handling, Retry-After header parsing, backoff strategies, request throttling, and circuit breaker patterns. Check StatusCodeHandler classes beyond 401/403. Package paths: com.linkedin.android.app, com.linkedin.android.networking
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All StatusCodeHandler classes cataloged
- [ ] #2 Retry/backoff strategies documented for main API (not just real-time)
- [ ] #3 Rate limit response headers identified if present
- [ ] #4 Findings written to re/rate_limiting.md
<!-- AC:END -->
