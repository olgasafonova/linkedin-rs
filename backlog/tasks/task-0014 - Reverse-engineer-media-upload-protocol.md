---
id: TASK-0014
title: Reverse-engineer media upload protocol
status: To Do
assignee: []
created_date: '2026-03-24 06:29'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Trace VectorUploadManager and related classes to document the upload endpoint, request format (multipart? signed URLs?), chunking, resumability, and media preprocessing. Package: com.linkedin.android.publishing.shared.mediaupload
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Upload endpoint URL pattern documented
- [ ] #2 Request format documented (headers, body, content-type)
- [ ] #3 Chunk/resume support documented
- [ ] #4 Media preprocessing pipeline documented
- [ ] #5 Findings written to re/media_upload.md
<!-- AC:END -->
