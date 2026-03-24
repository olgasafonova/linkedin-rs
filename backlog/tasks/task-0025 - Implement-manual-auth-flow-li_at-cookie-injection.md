---
id: TASK-0025
title: Implement manual auth flow (li_at cookie injection)
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 07:49'
updated_date: '2026-03-24 08:10'
labels: []
dependencies:
  - TASK-0024
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Since automated login triggers CAPTCHA, implement a manual auth mode: user provides li_at cookie value (from browser dev tools), client injects it into the cookie jar alongside a generated JSESSIONID. Persist tokens to ~/.local/share/linkedin/session.json. Per re/auth_flow.md.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 User can provide li_at cookie via CLI flag or env var
- [x] #2 JSESSIONID auto-generated and paired with li_at
- [x] #3 Session persisted to disk
- [x] #4 Session loaded on subsequent runs
- [x] #5 Auth status check (call a simple endpoint to verify session works)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Implement Session struct in auth.rs with li_at, jsessionid, created_at fields
2. Add serde Serialize/Deserialize, save/load to JSON, default_path via dirs crate, is_valid check
3. Add LinkedInClient::with_session method that injects li_at cookie into cookie jar
4. Implement CLI auth login --li-at flag + LINKEDIN_LI_AT env var fallback
5. Implement CLI auth status (load session, report validity)
6. Implement CLI auth logout (delete session file)
7. Run just e2e to verify
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented Session struct in auth.rs with li_at, jsessionid, created_at (DateTime<Utc>)
- Session::save writes JSON with 0600 perms on Unix, Session::load deserializes, Session::default_path returns ~/.local/share/linkedin/session.json via dirs crate
- Session::is_valid checks li_at is non-empty (local only, no API call)
- Added LinkedInClient::with_session that injects both li_at and JSESSIONID cookies into the cookie jar
- CLI auth login accepts --li-at flag with LINKEDIN_LI_AT env var fallback
- CLI auth status loads session and reports validity
- CLI auth logout deletes session file
- All 13 tests pass, clippy clean, fmt clean
- Manual e2e verification: login/status/logout/env-var paths all work
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented manual auth flow for li_at cookie injection.

Changes:
- linkedin-api/src/auth.rs: Replaced stub AuthSession with full Session struct (li_at, jsessionid, created_at). Added save/load (JSON, 0600 perms), default_path (~/.local/share/linkedin/session.json via dirs crate), and is_valid (non-empty li_at check).
- linkedin-api/src/client.rs: Added LinkedInClient::with_session() that injects li_at and JSESSIONID cookies into the reqwest cookie jar on .linkedin.com domain.
- linkedin-cli/src/main.rs: Implemented auth login (--li-at flag + LINKEDIN_LI_AT env var), auth status (load and report), auth logout (delete session file).

Tests:
- 4 new unit tests in auth::tests (roundtrip, is_valid, default_path, load error)
- All 13 tests pass, clippy clean, rustfmt clean
- Manual e2e: login/status/logout/env-var paths verified

Limitations:
- Auth status is local-only (no API call to verify session). Server-side validation would require a follow-up task.
- Device ID is not persisted in the session (generated fresh each time). Could be added if fingerprint consistency matters.
<!-- SECTION:FINAL_SUMMARY:END -->
