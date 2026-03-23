---
id: TASK-0011
title: Analyze React Native JS bundle for API calls
status: To Do
assignee: []
created_date: '2026-03-23 23:00'
labels:
  - phase3
  - static-analysis
  - react-native
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decompile and analyze the Hermes bytecode in assets/index.android.bundle to find API calls originating from the React Native layer. The RN layer may use different endpoints or patterns than the native layer. Use hermes-dec or hbcdump to decompile the Hermes bytecode, then search for fetch/XMLHttpRequest calls, API URLs, and data models. Some newer features are built in RN and may not have native Java equivalents.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 JS bundle successfully decompiled from Hermes bytecode
- [ ] #2 API calls from RN layer identified and documented
- [ ] #3 Any RN-only endpoints added to the endpoint catalog
<!-- AC:END -->
