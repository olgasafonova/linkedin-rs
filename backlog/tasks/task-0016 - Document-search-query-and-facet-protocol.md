---
id: TASK-0016
title: Document search query and facet protocol
status: Done
assignee:
  - '@claude'
created_date: '2026-03-24 06:29'
updated_date: '2026-03-24 06:42'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Read SearchRoutes, FacetParameterMap, and search-related Pegasus models to document the full search query format, facet encoding, sorting, and response structure. Package: com.linkedin.android.search
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Search query parameter format documented
- [x] #2 Facet encoding format documented
- [x] #3 Search response model structure documented
- [x] #4 Findings written to re/search_protocol.md
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read SearchRoutes.java, FacetParameterMap.java, and search Pegasus models
2. Trace query construction (keyword encoding, facet serialization, guides)
3. Catalog all search types, facet types, filter types, sort enums
4. Analyze response models (SearchCluster, SearchHit, SearchMetadata, SearchFacet)
5. Document typeahead endpoints and types
6. Write findings to re/search_protocol.md
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Analyzed SearchRoutes.java: mapped all URL construction methods for search, faceted search, job SERP, typeahead (v1/v2), geo typeahead, history
- Analyzed FacetParameterMap.java: documented facet detection (startsWith "facet"), internal format (facetName->value), serialization (buildStringList joins with pipe), parsing (regex for JSON array format)
- Cataloged enums: SearchType (13 values), PeopleSearchFacetType (16), JobSearchFacetType (32), ContentSearchFacetType (5), SearchFilterType (48 unified), SearchSortBy (6), TypeaheadType (26)
- Analyzed response models: SearchCluster (clustered results), SearchHit (union with 13 variant types), SearchMetadata (facets, spelling, pagination), SearchFacet/Value (counts, selected state)
- Documented guides system: vertical guide (v->type) + facet guides, different serialization for content vs people
- Found mobile-to-web facet mapping (7 entries) used for saved job searches
- Documented time-posted filter encoding (r86400, r604800, r2592000)
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Documented the full LinkedIn Android search query and facet protocol in re/search_protocol.md.

Key findings:
- Search uses Rest.li guided finder pattern (q=guided) with typed guides (v->people, facetName->value)
- Two facet systems: v1 guides-based (FacetParameterMap) and v2 filters-based (SearchFilterType)
- Job search uses different query style (q=jserpAll with individual facet params) than general search
- Responses are clustered (SearchCluster -> SearchHit with HitInfo union discriminating 13 result types)
- 6 search verticals documented with per-vertical facet catalogs (16 people, 32 job, 5 content facets)
- 8 typeahead endpoint patterns documented (blended, job, hashtag, geo/bing, federated, mentions)
- Rest.li encoding, pagination, sorting, and wire format examples included

File: re/search_protocol.md (12 sections, ~450 lines)
<!-- SECTION:FINAL_SUMMARY:END -->
