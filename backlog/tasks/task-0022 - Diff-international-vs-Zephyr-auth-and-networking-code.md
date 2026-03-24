---
id: TASK-0022
title: Diff international vs Zephyr auth and networking code
status: To Do
assignee: []
created_date: '2026-03-24 07:02'
labels: []
dependencies:
  - TASK-0021
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Compare the auth flow, Routes.java, networking stack, and required headers between the international Voyager and Zephyr China builds. Document any differences that affect the Rust client implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Routes.java compared — endpoint differences documented
- [ ] #2 Auth flow compared — login endpoint differences documented
- [ ] #3 Header requirements compared
- [ ] #4 Findings written to re/intl_vs_zephyr_diff.md
<!-- AC:END -->
