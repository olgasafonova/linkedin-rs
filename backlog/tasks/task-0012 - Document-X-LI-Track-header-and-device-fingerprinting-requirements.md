---
id: TASK-0012
title: Document X-LI-Track header and device fingerprinting requirements
status: To Do
assignee: []
created_date: '2026-03-23 23:01'
labels:
  - phase3
  - static-analysis
  - fingerprinting
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The X-LI-Track header is a critical device telemetry blob sent on every request. Document the exact format, all fields, how to generate realistic values for the Rust client (deviceId format, version strings, carrier, display metrics, etc.). Also investigate if LinkedIn validates/correlates these values for bot detection. Check for any other fingerprinting mechanisms (TLS fingerprint via Cronet, User-Agent validation, etc.).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 X-LI-Track header format fully documented with all fields
- [ ] #2 Device ID generation mechanism documented (Installation.id())
- [ ] #3 Recommendations for Rust client header generation documented
- [ ] #4 Other fingerprinting mechanisms identified
<!-- AC:END -->
