---
id: TASK-0035
title: Fix search endpoint (HTTP 404)
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 09:22'
updated_date: '2026-03-24 09:57'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GET /voyager/api/search/hits?q=guided&guides=List(v->people)&keywords=... returns 404. The endpoint path or query format is wrong. Check decompiled/jadx_intl/ for the correct search endpoint — may need voyager/api/graphql, or voyager/api/search/dash/*, or different query parameters. The international build has 219 Dash routes that may replace legacy search.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Search endpoint returns 200 with results
- [x] #2 CLI search people shows results correctly
- [x] #3 Response saved as fixture in secrets/
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Replace search/hits REST endpoint with GraphQL searchDashClustersByAll
2. queryId: voyagerSearchDashClusters.fae19421cdd51a7cd735e0b7d7b32e0f
3. Variables: query=(flagshipSearchIntent:SEARCH_SRP,keywords:X,queryParameters:(resultType:List(PEOPLE)))
4. Update search_people() in client.rs to use graphql_get()
5. Update CLI display (print_search_hit) for new response shape
6. Test live, save response to secrets/search_response.json
7. Run e2e tests
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Traced search endpoint migration from REST (search/hits) to GraphQL (searchDashClustersByAll) via decompiled SearchGraphQLClient.java
- queryId: voyagerSearchDashClusters.fae19421cdd51a7cd735e0b7d7b32e0f
- Variables use Rest.li record encoding with flagshipSearchIntent:SEARCH_SRP and queryParameters resultType:List(PEOPLE)
- Response shape changed: clusters -> items -> entityResult (vs old SearchHit.hitInfo.SearchProfile)
- Added restli_encode_string() helper for keyword encoding
- Updated CLI display to extract title/primarySubtitle/secondarySubtitle/navigationUrl from entityResult
- Tested live: returns 200 with correct people search results
- Saved response fixture to secrets/search_response.json
- e2e tests pass (build, test, clippy, fmt-check)
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed search endpoint (HTTP 404) by migrating from deprecated REST endpoint to GraphQL.

Root cause: The international LinkedIn build has migrated search from REST (GET /voyager/api/search/hits?q=guided&guides=List(v->people)) to GraphQL (voyagerSearchDashClusters). The old endpoint returns 404.

Changes:
- linkedin-api/src/client.rs: Rewrote search_people() to use graphql_get() with queryId voyagerSearchDashClusters.fae19421cdd51a7cd735e0b7d7b32e0f. Variables encoded in Rest.li record syntax with flagshipSearchIntent:SEARCH_SRP and queryParameters:(resultType:List(PEOPLE)). Added restli_encode_string() helper. GraphQL envelope (data.searchDashClustersByAll) is unwrapped before returning.
- linkedin-cli/src/main.rs: Rewrote display logic for new response shape. Old format had SearchProfile.miniProfile; new format has clusters[].items[].item.entityResult with title.text, primarySubtitle.text, secondarySubtitle.text, navigationUrl.
- secrets/search_response.json: Saved working response fixture.

Tested: Live API returns 200 with correct results. e2e gate passes (build, test, clippy, fmt-check).
<!-- SECTION:FINAL_SUMMARY:END -->
