---
id: TASK-0049
title: Extract hardcoded GraphQL query IDs to config
status: To Do
assignee: []
created_date: '2026-03-24 17:42'
labels: []
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
15+ GraphQL query ID hashes are hardcoded in client.rs and RE docs. These are version-locked to the decompiled APK and will silently break when LinkedIn ships a new version. Extract to a config file or constants module for easier updates. Categories: comments (7), reactions (3), create post (4), profile (1), invitations (1), messaging (~5), search (~2).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Query IDs extracted to a single constants module or config file
- [ ] #2 client.rs reads from the constants module
- [ ] #3 RE docs reference the constants rather than hardcoding
<!-- AC:END -->
