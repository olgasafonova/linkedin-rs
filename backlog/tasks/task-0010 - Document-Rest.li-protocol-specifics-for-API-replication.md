---
id: TASK-0010
title: Document Rest.li protocol specifics for API replication
status: Done
assignee:
  - '@claude'
created_date: '2026-03-23 23:00'
updated_date: '2026-03-24 05:47'
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

## Notes
- Output: re/restli_protocol.md (12 sections, covers all wire-level protocol details)
- Traced AsciiHexEncoding -> DataEncoder -> ValueCodec encoding pipeline with exact char sets
- Query tunneling threshold is 4096 chars (query or path), converts GET to POST with X-HTTP-Method-Override and multipart/mixed body
- Discovered the app uses protobuf by default (not JSON) -- Accept header is application/vnd.linkedin.deduped+x-protobuf. For Rust client, override with application/json.
- Documented 18+ decoration recipe IDs with their domain and versioned names
- CollectionTemplate has 6 top-level JSON keys: elements, metadata, paging, entityUrn, isError, error
- ErrorResponse has 9 fields: status, serviceErrorCode, code, message, docUrl, requestId, exceptionClass, stackTrace, errorDetailType
- Mux endpoint exists but MultiplexRequest is client-side orchestration (deprecated server-side URL), not a server wire protocol -- low priority for Rust client
- X-RestLi-Method header only needed for batch operations (BATCH_UPDATE, BATCH_DELETE)
- Symbol table compression: x-restli-symbol-table-name header sent on all API requests for protobuf field ID mapping
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rest.li URL encoding rules documented with examples
- [x] #2 Query tunneling mechanism documented
- [x] #3 Batch/mux request format documented
- [x] #4 Decoration/recipe parameter system documented
- [x] #5 Collection response and error response formats documented
<!-- AC:END -->
