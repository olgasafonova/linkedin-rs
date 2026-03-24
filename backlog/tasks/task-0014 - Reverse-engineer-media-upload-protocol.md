---
id: TASK-0014
title: Reverse-engineer media upload protocol
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 06:29'
updated_date: '2026-03-24 06:35'
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
- [x] #1 Upload endpoint URL pattern documented
- [x] #2 Request format documented (headers, body, content-type)
- [x] #3 Chunk/resume support documented
- [x] #4 Media preprocessing pipeline documented
- [x] #5 Findings written to re/media_upload.md
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Analyzed decompiled jadx sources across 5 packages. Traced the full Vector upload protocol: metadata request -> signed URL upload -> multipart completion. Documented both the modern Vector system and legacy SlideShare/MUPLD paths. Found image preprocessing (downscale to 1280px min-axis, max 7680x4320, EXIF rotation, quality 100) and video transcoding config (720p, 5Mbps target -- stubbed in Zephyr build). Three upload modes: SINGLE, MULTIPART, PARTIAL_MULTIPART.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fully documented LinkedIn media upload protocol in re/media_upload.md. Key finding: two-phase signed URL pattern (POST voyagerMediaUploadMetadata?action=upload to get pre-signed URLs, then PUT raw bytes to those URLs). Supports single and multipart (chunked) uploads with per-part byte ranges and expiring URLs. Multipart uploads require a completion call (action=completeMultipartUpload) that sends back part response headers. No true resume support -- only whole-part retries (default 10). Image preprocessing: EXIF rotation + downscale (min-axis 1280px cap, max 7680x4320). Video transcoding: 720p/5Mbps target but no-op in analyzed Zephyr build.
<!-- SECTION:FINAL_SUMMARY:END -->
