---
id: TASK-0041
title: Register profile view (visit a profile as viewer)
status: To Do
assignee: []
created_date: '2026-03-24 13:21'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When the real LinkedIn app views a profile, it registers the visit so the target sees you in their 'who viewed my profile'. Our profile view command fetches data but likely doesn't register the visit. Investigate DiscloseAsProfileViewerInfo enum (DISCLOSE_FULL, DISCLOSE_ANONYMOUS, HIDE) and find the mechanism — may be automatic with the right decoration, or a separate POST endpoint. Use Chrome DevTools MCP to capture what the web app sends when visiting a profile.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Mechanism for registering profile views identified
- [ ] #2 CLI 'profile visit <id>' command triggers a real profile view
- [ ] #3 Documented in re/
<!-- AC:END -->
