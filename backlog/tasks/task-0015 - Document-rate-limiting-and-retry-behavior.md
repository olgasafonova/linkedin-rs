---
id: TASK-0015
title: Document rate limiting and retry behavior
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 06:29'
updated_date: '2026-03-24 06:35'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Search decompiled code for HTTP 429 handling, Retry-After header parsing, backoff strategies, request throttling, and circuit breaker patterns. Check StatusCodeHandler classes beyond 401/403. Package paths: com.linkedin.android.app, com.linkedin.android.networking

Findings summary:
- Only 2 StatusCodeHandlers registered: 401 (UnauthorizedStatusCodeHandler) and 403 (ForbiddenStatusCodeHandler). No 429 handler.
- Main API retry: transport-level only (Cronet), GET/HEAD only, on connection failures (error code 3), immediate (no backoff), opt-in per request.
- Retry-After header parsed but only honored during redirect processing, not for 429/503.
- X-LI-Retry-Context header sent on retries with attempt number and error code.
- No client-side rate limiting, throttling, or circuit breaker patterns.
- 429 handled ad-hoc in 2-3 UI components (company contact, premium insights).
- PEM system treats 429 as server degradation (not client error).
- Default timeouts: 10s connect, 10s read, 10s write.
- Real-time has separate backoff: exponential (2^n * 100ms) and linear (random 5-7s * attempt, max 2 retries).
- Full findings in re/rate_limiting.md
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All StatusCodeHandler classes cataloged
- [x] #2 Retry/backoff strategies documented for main API (not just real-time)
- [x] #3 Rate limit response headers identified if present
- [x] #4 Findings written to re/rate_limiting.md
<!-- AC:END -->
