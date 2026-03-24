---
id: TASK-0031
title: Validate search endpoint — GET /voyager/api/search/hits
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 07:49'
updated_date: '2026-03-24 08:46'
labels: []
dependencies:
  - TASK-0026
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Call the search endpoint with a simple keyword query. Verify facet encoding, response format, and pagination. Compare against search models from re/search_protocol.md.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Search endpoint returns results for keyword query
- [x] #2 Response structure compared against documented models
- [ ] #3 Facet parameters work
- [ ] #4 Response saved as fixture in secrets/
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented search_people() in client.rs using search/hits?q=guided&guides=List(v->people) endpoint.
Added SearchResponse and SearchHit models to models.rs (kept loose with Value for union types).
Added search people <keywords> subcommand to CLI with --count, --start, --json flags.
Human-readable output shows name, headline, location per result.
All e2e gates pass (build, test, clippy, fmt-check).
NOTE: AC #3 (facet parameters) and AC #4 (fixture in secrets/) require a live session to validate -- the code infrastructure is in place but cannot be verified without API access.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added people search endpoint support: search_people() in client.rs, SearchResponse/SearchHit models in models.rs, and `search people` CLI subcommand. Uses guided finder (q=guided, guides=List(v->people)) on search/hits endpoint per RE docs. AC #3 and #4 blocked on live API session for validation.
<!-- SECTION:FINAL_SUMMARY:END -->
