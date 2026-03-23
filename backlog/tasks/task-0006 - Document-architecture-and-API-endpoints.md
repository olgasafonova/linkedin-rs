---
id: TASK-0006
title: Document architecture and API endpoints
status: Done
assignee:
  - '@claude'
created_date: '2026-03-23 22:32'
updated_date: '2026-03-23 23:01'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After extraction and decompilation, document the app architecture, identify API endpoint patterns, and create a preliminary API catalog. This is a meta-task to add more Phase 3 backlog tasks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 App architecture overview documented in re/
- [x] #2 Phase 3 backlog tasks created based on findings
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Search for base URLs and API host constants
2. Identify REST client framework (Retrofit/OkHttp/custom)
3. Search for Rest.li protocol headers and interceptors
4. Map auth flow from liauthlib package
5. Find endpoint patterns and URL builders
6. Write findings to re/architecture_overview.md
7. Create Phase 3 backlog tasks
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Completed architecture_overview.md with full networking stack, auth flow, API routes, Rest.li protocol details
- Key findings: Cronet (not OkHttp) as transport, cookie-based auth (not bearer tokens), /voyager/api/ prefix, ~200 routes in Routes enum, ~1458 Pegasus model classes

- Created 7 Phase 3 backlog tasks:
  - TASK-0007: Map complete API endpoint catalog
  - TASK-0008: Document OAuth2/OIDC auth flow in detail
  - TASK-0009: Extract data models/DTOs from Pegasus layer
  - TASK-0010: Document Rest.li protocol specifics
  - TASK-0011: Analyze React Native JS bundle
  - TASK-0012: Document X-LI-Track header and fingerprinting
  - TASK-0013: Map real-time messaging and notification system
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Documented LinkedIn Android app architecture through targeted analysis of the 362MB jadx decompiled output.

Key deliverables:
- Created re/architecture_overview.md with comprehensive documentation of:
  - Networking stack: custom NetworkClient -> LinkedInNetwork -> CronetNetworkEngine (Chromium Cronet v83, NOT OkHttp)
  - Base URL: https://www.linkedin.com with /voyager/api/ (or /zephyr/api/) prefix
  - Rest.li 2.0 protocol: custom URL encoding, query tunneling, decoration-based field projection
  - Auth flow: cookie-based (li_at + JSESSIONID CSRF), NOT bearer tokens. POST /uas/authenticate with form-encoded credentials
  - ~200 API routes enumerated from Routes.java covering all in-scope domains
  - ~1458 Pegasus-generated model classes identified in the voyager namespace
  - X-LI-Track header format (device telemetry JSON blob)
  - Real-time subscription system via long-polling

Created 7 Phase 3 tasks (TASK-0007 through TASK-0013) covering:
  - Complete API endpoint catalog extraction
  - Detailed auth flow documentation
  - Pegasus data model extraction
  - Rest.li protocol specifics
  - React Native JS bundle analysis
  - Device fingerprinting requirements
  - Real-time messaging system mapping
<!-- SECTION:FINAL_SUMMARY:END -->
