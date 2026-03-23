---
id: TASK-0008
title: Document OAuth2/OIDC auth flow in detail
status: To Do
assignee: []
created_date: '2026-03-23 23:00'
labels:
  - phase3
  - static-analysis
  - auth
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deep-dive into the authentication flow from LiAuthImpl, LiAuth, AuthHttpStackWrapper, and related classes. Document the complete cookie-based auth flow, CSRF token generation, session lifecycle, token refresh, third-party OAuth PKCE flow, Google/Apple/Flash sign-in variants, and challenge/CAPTCHA handling. This is critical for implementing auth in the Rust client.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Complete auth flow documented in re/auth_flow.md
- [ ] #2 Cookie lifecycle documented (li_at, JSESSIONID, others)
- [ ] #3 CSRF token generation and validation documented
- [ ] #4 Third-party OAuth PKCE flow documented with all parameters
- [ ] #5 Challenge/CAPTCHA handling flow documented
<!-- AC:END -->
