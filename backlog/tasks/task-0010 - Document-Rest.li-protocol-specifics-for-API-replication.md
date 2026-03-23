---
id: TASK-0010
title: Document Rest.li protocol specifics for API replication
status: To Do
assignee: []
created_date: '2026-03-23 23:00'
labels:
  - phase3
  - static-analysis
  - protocol
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deep-dive into Rest.li 2.0 protocol implementation details needed to correctly replicate API calls. Document: custom URL encoding (AsciiHexEncoding with RestliUtils.ValueCodec), query tunneling mechanism (QueryTunnelUtil), batch/mux request format, decoration/recipe parameter system, collection response format (elements array, paging metadata), error response format (ErrorResponse model), and any Rest.li-specific headers beyond X-RestLi-Protocol-Version.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rest.li URL encoding rules documented with examples
- [ ] #2 Query tunneling mechanism documented
- [ ] #3 Batch/mux request format documented
- [ ] #4 Decoration/recipe parameter system documented
- [ ] #5 Collection response and error response formats documented
<!-- AC:END -->
