---
id: TASK-0048
title: 'Redact PII from RE docs (URNs, names)'
status: To Do
assignee: []
created_date: '2026-03-24 17:42'
labels: []
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
re/send_message.md contains real profile URNs (ACoAABxxxxxx... and ACoAACyyyyyy...) identifying real users. re/profile_viewers.md contains real names with occupation and publicIdentifier. Replace with placeholder values before pushing to any remote.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Real URNs in re/send_message.md replaced with placeholders
- [ ] #2 Real name/profile in re/profile_viewers.md replaced with placeholder
- [ ] #3 PII scan passes
<!-- AC:END -->
