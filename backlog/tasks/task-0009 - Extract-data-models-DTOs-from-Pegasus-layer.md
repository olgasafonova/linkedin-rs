---
id: TASK-0009
title: Extract data models/DTOs from Pegasus layer
status: To Do
assignee: []
created_date: '2026-03-23 23:00'
labels:
  - phase3
  - static-analysis
  - models
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Analyze the ~1458 Pegasus-generated model classes under com.linkedin.android.pegasus.gen.voyager to extract the data schema for each domain. Map field names, types, optional/required status, and URN references. Focus on the models most relevant to our in-scope features: Me, MiniProfile, Profile, ProfileView, Update/UpdateV2, Conversation, Event, MessageEvent, JobPosting, Connection, Invitation, Notification. This will directly inform the Rust struct definitions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Key model schemas documented in re/data_models.md
- [ ] #2 URN format and entity reference patterns documented
- [ ] #3 Collection/pagination metadata model documented
- [ ] #4 At least 10 core models fully mapped (fields, types, optionality)
<!-- AC:END -->
