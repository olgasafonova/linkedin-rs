---
id: TASK-0050
title: Make device fingerprint configurable
status: To Do
assignee: []
created_date: '2026-03-24 17:42'
labels: []
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
client.rs hardcodes Google Pixel 8, T-Mobile carrier, app version 4.2.1058, clientMinorVersion 562100. This is detectable as spoofed activity and goes stale when LinkedIn updates. Make the device profile configurable via a struct or config file.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Device fingerprint loaded from config or constructor parameter
- [ ] #2 Default values still work out of the box
- [ ] #3 Documented how to update when LinkedIn ships new versions
<!-- AC:END -->
