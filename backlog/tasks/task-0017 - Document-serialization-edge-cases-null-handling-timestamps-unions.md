---
id: TASK-0017
title: 'Document serialization edge cases (null handling, timestamps, unions)'
status: To Do
assignee: []
created_date: '2026-03-24 06:29'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Examine Pegasus RecordTemplate/UnionTemplate serialization to determine: null vs absent field distinction, empty collection handling, timestamp format (epoch seconds vs ms), deep nested union discriminator format, and custom serializers. Package: com.linkedin.data.lite, com.linkedin.android.pegasus
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Null vs absent field behavior documented
- [ ] #2 Timestamp format confirmed (seconds vs milliseconds)
- [ ] #3 Nested union discriminator format documented
- [ ] #4 Findings written to re/serialization_edge_cases.md
<!-- AC:END -->
