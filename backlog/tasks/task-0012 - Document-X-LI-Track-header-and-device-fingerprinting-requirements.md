---
id: TASK-0012
title: Document X-LI-Track header and device fingerprinting requirements
status: Done
assignee:
  - '@claude'
created_date: '2026-03-23 23:01'
updated_date: '2026-03-24 05:53'
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
- [x] #1 X-LI-Track header format fully documented with all fields
- [x] #2 Device ID generation mechanism documented (Installation.id())
- [x] #3 Recommendations for Rust client header generation documented
- [x] #4 Other fingerprinting mechanisms identified
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Traced X-LI-Track through two independent code paths:
- Main app: XLiTrackHeader.java (18 fields, used for all authenticated requests)
- Auth library: NetworkUtils.java (11 fields, different format, used during login/registration)

Key findings:
- Two distinct X-LI-Track JSON formats (main vs auth) with significant differences in field names, types, and values
- Device ID is UUID v4 persisted to file, same value used for both X-UDID header and deviceId in track JSON
- JSESSIONID generated with SecureRandom, format ajax:{019d}
- TLS fingerprinting via Cronet (Chrome 83) is the highest-risk detection vector
- AppConfig fields (storeId, advertiserId, mpName, mpVersion) are conditional on AppConfig presence
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Documented device fingerprinting and header requirements in re/device_fingerprinting.md.

Key deliverables:
- Complete X-LI-Track JSON schema with all 18 fields, types, sources, and required/optional status
- Documented the two distinct X-LI-Track variants (main app vs auth library) with a comparison table
- Installation.id() device ID generation documented (UUID v4, file-persisted)
- JSESSIONID/CSRF generation format: ajax:{019-digit SecureRandom long}
- User-Agent and X-LI-User-Agent header formats documented
- Complete header matrix for authenticated requests, auth requests, and tracking requests
- TLS/HTTP2 fingerprinting risks identified (Cronet/Chrome 83)
- Rust client implementation recommendations with code examples and consistency requirements
- Risk assessment table for each fingerprinting mechanism
<!-- SECTION:FINAL_SUMMARY:END -->
