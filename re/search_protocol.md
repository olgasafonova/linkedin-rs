# LinkedIn Search Query and Facet Protocol

Reverse-engineered from `com.linkedin.android` APK (jadx decompilation). Analysis date: 2026-03-24.

Source packages:
- `com.linkedin.android.search` (especially `SearchRoutes`, `FacetParameterMap`, `SearchDataProvider`)
- `com.linkedin.android.pegasus.gen.voyager.search` (Pegasus-generated search models)
- `com.linkedin.android.pegasus.gen.voyager.typeahead` (typeahead models)

---

## 1. Search Endpoints

All endpoints are relative to the API root (`/voyager/api/` or `/zephyr/api/`).

| Route constant | Path | Purpose |
|----------------|------|---------|
| `SEARCH` | `search/hits` | Primary search results |
| `SEARCH_BLENDED` | `search/blended` | Blended (multi-vertical) search |
| `GUIDED_SEARCH_CLUSTER` | `search/cluster` | Guided/faceted search clusters |
| `SEARCH_FACET` | `search/facets` | Facet metadata and values |
| `SEARCH_FILTERS` | `search/filters` | Filter metadata (v2) |
| `SEARCH_HISTORY` | `search/history` | Search history (view/clear/update) |
| `SEARCH_TOPICS` | `search/topics` | Trending / suggested topics |
| `SEARCH_QUERIES` | `search/queries` | Suggested queries |
| `SEARCH_DEGREES` | `search/degrees` | Popular degrees |
| `SEARCH_ADS` | `search/ads` | Search ads |
| `SEARCH_JYMBII_ADS` | `search/wwuAds` | "Who's Who" ads |
| `SEARCH_SAVED_SEARCH` | `search/savedSearches` | Saved search CRUD |
| `TYPEAHEAD` | `typeahead/hits` | Typeahead suggestions (v1) |
| `TYPEAHEADV2` | `typeahead/hitsV2` | Typeahead suggestions (v2) |
| `TYPEAHEAD_HITS` | `voyagerTypeaheadHits` | Typeahead hits (alternate) |
| `JOB_SEARCH` | `jobs/search` | Job-specific search |

---

## 2. Search Query Parameter Format

### 2.1 Core Parameters

Every search request uses Rest.li finder semantics. The `q` parameter selects the finder method:

| Finder (`q=`) | Used By | Notes |
|---------------|---------|-------|
| `guided` | Blended SRP, faceted search | Main search mode |
| `jserpAll` | Job SERP | Job search results page |
| `jserpDeepLinkUrl` | Job SERP deep links | When navigating from external URL |
| `suggested` | Topics, queries | Suggested content |
| `trending` | Topics | Trending topics |
| `quelp` | Query suggestions | Auto-suggest |
| `popularDegreesInCurrentCountry` | Degrees | Degree suggestions |
| `wwuAds` | JYMBII ads | Profile-targeted ads |

### 2.2 Keyword Encoding

Keywords are passed via the `keywords` query parameter, encoded using Rest.li's `AsciiHexEncoding` with `%` as the escape character. Special characters `( ) , ' :` are hex-encoded per Rest.li convention.

From `SearchRoutes.buildJobSerpRoute`:
```
keywords=<AsciiHexEncoded(keyword_string)>
```

When no keywords are provided, the app sends an empty quoted string:
```
keywords=''
```

The keywords value is appended via `RestliUtils.appendEncodedQueryParameter`, which applies Rest.li encoding on top of standard URI encoding.

### 2.3 Standard Query Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `keywords` | string | Search terms (Rest.li encoded) |
| `searchId` | string | Unique search session ID |
| `origin` | string | Search origin context (e.g., where the search was initiated) |
| `timestamp` | long | Current time in milliseconds (`System.currentTimeMillis()`) |
| `start` | int | Pagination offset (0-based) |
| `count` | int | Page size |
| `paginationToken` | string | Opaque pagination token |
| `spellCorrectionEnabled` | bool | Set to `"false"` to disable spell correction |
| `blendedSrpEnabled` | bool | `"true"` to enable blended search results page |
| `relatedSearchesEnabled` | bool | `"true"` to include related searches in response |
| `geoUrn` | URN | Geography filter (default: `urn:li:fs_geo:92000000` = worldwide) |
| `decorationId` | string | Rest.li decoration recipe for field projection |

### 2.4 Decoration (Field Projection)

Job search applies a specific decoration recipe:
```
decorationId=com.linkedin.voyager.deco.jobs.search.ListedJobSearchHit-50
```

This controls which fields are included in the response. Other search types use different recipes set elsewhere.

---

## 3. Search Types

The `SearchType` enum defines all search verticals:

| Value | Description |
|-------|-------------|
| `ALL` | All results |
| `TOP` | Top/best results |
| `PEOPLE` | People search |
| `COMPANIES` | Company search |
| `JOBS` | Job search |
| `CONTENT` | Content/posts search |
| `GROUPS` | Group search |
| `SCHOOLS` | School search |
| `ARTICLE` | Article search |
| `FEED_UPDATES` | Feed updates |
| `PROVIDERS` | Service providers |
| `UNIFIED_LEARNING` | LinkedIn Learning |
| `HASHTAG` | Hashtag search |

### 3.1 Search Type in Guided Search

The search type is encoded as a "guide" parameter using the format:

```
v-><type_lowercase>
```

For example:
- `v->people` for people search
- `v->content` for content search
- `v->jobs` for job search

This is passed within the `guides` batch parameter (see Facet Encoding below).

---

## 4. Facet Encoding

LinkedIn search uses two facet systems: the v1 "guided" system (facets as guide parameters) and a v2 filter system.

### 4.1 FacetParameterMap (v1 System)

`FacetParameterMap` is the core data structure for facets. It is a `Map<String, List<String>>` where keys are facet parameter names (prefixed with `facet`) and values are lists of selected facet values.

**Facet detection**: A parameter is a facet if its name starts with `"facet"` (checked by `isFacetParam()`).

**Internal representation**: Facet entries are stored both in the map and as a flat list of strings in the format:
```
<facetName>-><facetValue>
```

For example:
```
facetIndustry->96
facetCurrentCompany->1035
facetGeoRegion->103644
```

### 4.2 Facet Value Parsing

When facet values arrive from the server as `SearchQueryParam` objects, the value format is:
```
["value1","value2","value3"]
```

The `parseFacetParam` method extracts values using regex `\["(.*)"\]` and splits on `","` to produce individual values joined by `|` (pipe separator, from `HiAnalyticsConstant.REPORT_VAL_SEPARATOR`).

### 4.3 Guides Parameter (Wire Format)

Facets are serialized into the `guides` batch query parameter using Rest.li list encoding:

```
guides=List(v->people,facetIndustry->96,facetGeoRegion->103644)
```

The first element is always the vertical guide (`v->type`). Remaining elements are facet guides.

For **content** search, the facet list uses the raw `facetName->value` format. For **people** and other types, `buildStringList()` joins multi-valued facets with `|`:
```
facetIndustry->96|104
```

### 4.4 Location Parameters

Location is handled specially within guide parameters:

| Scenario | Parameters |
|----------|-----------|
| Country + postal code | `countryCode->{code}` and `postalCode->{code}` |
| Location ID | `locationId->{id}` |
| Fallback (worldwide) | `locationId->OTHERS.worldwide` |

### 4.5 Mobile-to-Web Facet Mapping

For saved job searches, mobile facet names are translated to web query parameters:

| Mobile Facet | Web Parameter |
|-------------|---------------|
| `facetApplyWithLinkedIn` | `f_LF` |
| `facetTimePosted` | `f_TP` |
| `facetCompany` | `f_C` |
| `facetExperience` | `f_E` |
| `facetJobType` | `f_JT` |
| `facetIndustry` | `f_I` |
| `facetFunction` | `f_F` |

---

## 5. Facet Types by Search Vertical

### 5.1 People Search Facets (`PeopleSearchFacetType`)

| Facet | Description |
|-------|-------------|
| `CONNECTION_OF` | Connections of a specific person |
| `CURRENT_COMPANY` | Current employer |
| `CURRENT_FUNCTION` | Current job function |
| `FIELD_OF_STUDY` | Field of study |
| `GEO_REGION` | Geographic region (legacy) |
| `GEO` | Geographic area (v2) |
| `GROUPS` | Group membership |
| `INDUSTRY` | Industry |
| `NETWORK` | Connection degree (1st, 2nd, 3rd+) |
| `NONPROFIT_INTEREST` | Nonprofit interests |
| `PAST_COMPANY` | Past employer |
| `PROFILE_LANGUAGE` | Profile language |
| `SCHOOL` | School attended |
| `SENIORITY` | Seniority level |
| `SKILL_EXPLICIT` | Explicit skills |
| `PROFESSIONAL_EVENT` | Professional event attendance |

### 5.2 Job Search Facets (`JobSearchFacetType`)

| Facet | Description |
|-------|-------------|
| `COMPANY` | Hiring company |
| `EXPERIENCE` | Experience level |
| `FUNCTION` | Job function |
| `INDUSTRY` | Industry |
| `JOB_TYPE` | Employment type (full-time, part-time, etc.) |
| `LOCATION` | Job location |
| `GEO_CITY` | City |
| `POPULATED_PLACE` | Populated place |
| `COUNTRY_REGION` | Country/region |
| `DISTANCE` | Distance from location |
| `TIME_POSTED` | When posted |
| `TIME_POSTED_RANGE` | Posted time range |
| `SALARY_BUCKET` / `V2` / `V3` | Salary range |
| `TITLE` | Job title |
| `APPLY_WITH_LINKEDIN` | Easy Apply filter |
| `LINKEDIN_FEATURES` | LinkedIn features |
| `WORK_REMOTE_ALLOWED` | Remote work filter |
| `COMMUTE_FEATURES` | Commute options |
| `EMPLOYEE_TYPES` | Employee types |
| `EMPLOYEE_SCHEDULES` | Work schedules |
| `COMPANY_TYPE` | Company type |
| `BENEFITS` | Benefits offered |
| `FAIR_CHANCE_EMPLOYER` | Fair chance employer |
| `EARLY_APPLICANT` | Early applicant flag |
| `HIRE_FROM_YOUR_COMPANY` | Hiring from your company |
| `HIRE_FROM_YOUR_SCHOOL` | Hiring from your school |
| `JOB_IN_YOUR_NETWORK` | Jobs in your network |
| `DURATION_IN_MONTHS` | Contract duration |
| `START_AND_END_TIME` | Start/end dates |
| `SORT_BY` | Sort order |

### 5.3 Content Search Facets (`ContentSearchFacetType`)

| Facet | Description |
|-------|-------------|
| `CONTENT_TYPE` | Type of content |
| `NETWORK` | Network connection level |
| `RECENCY` | How recent |
| `SORT_BY` | Sort order |
| `TOPIC` | Topic filter |

### 5.4 General Search Facets (`SearchFacetType`)

Shared across multiple verticals:

`CURRENT_COMPANY`, `CURRENT_FUNCTION`, `FIELD_OF_STUDY`, `GEO_REGION`, `INDUSTRY`, `NETWORK`, `NONPROFIT_INTEREST`, `PAST_COMPANY`, `PROFILE_LANGUAGE`, `SCHOOL`, `SKILL_EXPLICIT`, `GEO`

### 5.5 Unified Filter Types (`SearchFilterType`)

The v2 filter system (`SearchFilterType`) is a superset combining people, job, and content filters plus additional fields:

People-specific: `FIRST_NAME`, `LAST_NAME`, `EDUCATION_START_YEAR`, `EDUCATION_END_YEAR`, `CONTACT_INTEREST`, `SERVICE_CATEGORY`

Location: `COUNTRY_CODE`, `POSTAL_CODE`, `LOCATION_ID`, `LOCATION_FALLBACK`, `LATITUDE`, `LONGITUDE`, `GEO_URN`

Meta: `RESULT_TYPE`, `SAVED_SEARCH_ID`, `ANCHOR_TOPIC`, `AUTHOR_COMPANY`, `AUTHOR_INDUSTRY`

---

## 6. Sorting / Ranking Parameters

### 6.1 SearchSortBy Enum

| Value | Likely Meaning |
|-------|---------------|
| `R` | Relevance (default) |
| `DD` | Date descending (newest first) |
| `DA` | Date ascending (oldest first) |
| `DP` | Date posted |
| `DE` | Date ended |
| `FS` | Featured/sponsored |

### 6.2 Time-Posted Filters

Job search has special time-posted encoding using the `f_TPR` parameter:

| Value | Meaning |
|-------|---------|
| `r86400` | Past 24 hours (86400 seconds) |
| `r604800` | Past week (604800 seconds) |
| `r2592000` | Past month (2592000 seconds) |

---

## 7. Response Model Structure

### 7.1 Search Results: Clustered Response

Search results are returned as `SearchCluster` objects, each containing:

```
SearchCluster
  title: string            -- Cluster display title (e.g., "People", "Jobs")
  total: long              -- Total results in this cluster
  type: ClusterType        -- LARGE, MEDIUM, PRIMARY, SMALL
  hitType: SearchType      -- Which vertical (PEOPLE, JOBS, etc.)
  keywords: string         -- Echo of search keywords
  navigateText: string     -- CTA text for "See all" navigation
  elements: [SearchHit]    -- Array of search results
  guides: [Guide]          -- Available facet guides for refinement
```

### 7.2 SearchHit (Individual Result)

Each `SearchHit` contains:

```
SearchHit
  trackingId: string       -- Tracking identifier
  targetPageInstance: string
  hitInfo: HitInfo (union) -- One of:
    - SearchProfile         -- People result
    - SearchJob             -- Job result (legacy)
    - SearchJobJserp        -- Job SERP result
    - SearchCompany         -- Company result
    - SearchSchool          -- School result
    - SearchGroup           -- Group result
    - SearchArticle         -- Article result
    - SecondaryResultContainer -- Secondary/related results
    - FacetSuggestion       -- Suggested facet refinement
    - Paywall               -- Premium upsell
    - Update (feed)         -- Content/feed result
    - Jymbii                -- Job recommendation ad
    - BlurredHit            -- Blurred/premium-gated result
```

The `HitInfo` is a **Rest.li union** -- exactly one member is present per hit, discriminated by type.

### 7.3 SearchProfile (People Result)

```
SearchProfile
  id: string
  backendUrn: Urn
  miniProfile: MiniProfile  -- Lightweight profile data (name, photo, headline)
  distance: MemberDistance   -- Network distance (1st/2nd/3rd)
  location: string
  industry: string
  snippets: [Snippet]       -- Highlighted matching text
  sharedConnectionCount: int
  sharedConnectionsInfo: SharedConnectionsInfo
  educations: [Education]
  followingInfo: FollowingInfo
  memberBadges: MemberBadges
  profileActions: ProfileActions
  headless: bool            -- Profile without photo/details
  nameMatch: bool           -- Whether name matched query
```

### 7.4 SearchMetadata

The metadata object accompanies clustered results:

```
SearchMetadata
  id: string               -- Search result set ID
  keywords: string          -- Echoed keywords
  origin: string            -- Search origin
  type: SearchType          -- Search vertical
  sortBy: SearchSortBy      -- Applied sort order
  totalResultCount: long    -- Total matching results
  filteredEntityCount: int  -- Results after filtering
  facets: [SearchFacet]     -- Available facets with counts
  guides: [Guide]           -- Guide parameters for navigation
  relatedSearches: [RelatedSearch]  -- Related search suggestions
  spellCorrection: SearchSpellingCorrection  -- Spell correction info
  queryExpansion: SearchSpellingCorrection   -- Query expansion info
  locationInfo: LocationInfo
  savedSearchPreview: SavedSearchPreview
  taggedQueryKeyword: string
  taggedQueryType: TaggedQueryType
  taggedQueryUrn: Urn
```

### 7.5 SearchFacet (Facet Metadata in Response)

```
SearchFacet
  displayName: string            -- Human-readable facet name
  facetParameterName: string     -- Parameter name for filtering (e.g., "facetIndustry")
  facetType: SearchFacetType     -- Typed facet enum
  facetTypeV2: SearchFacetTypeV2 -- V2 typed facet
  premiumFacet: bool             -- Whether this requires Premium
  facetValues: [SearchFacetValue]
    value: string                -- Facet value ID
    displayValue: string         -- Human-readable label
    count: int                   -- Result count for this value
    selected: bool               -- Whether currently active
    image: Image                 -- Optional icon/image
```

### 7.6 Guide (Navigation/Refinement)

```
Guide
  urlParameter: string     -- URL parameter value (e.g., "v->people", "facetIndustry->96")
  displayText: string      -- Human-readable label
  selected: bool           -- Currently selected
  type: GuideType          -- FACET or VERTICAL
  guideInfo: GuideInfo     -- Additional guide metadata
```

### 7.7 Pagination

Standard Rest.li collection pagination:

```
CollectionMetadata
  start: int               -- Offset of first result
  count: int               -- Number of results in page
  total: int               -- Total available results
  paginationToken: string  -- Opaque token for next page
```

Request: `?start=0&count=10`
Next page: `?start=10&count=10` or use `paginationToken` if provided.

---

## 8. Typeahead Endpoints

### 8.1 Blended Typeahead (v1)

```
GET typeahead/hits?q=blended&id={trackingId}&query={keywords}
```

### 8.2 Blended Typeahead (v2)

```
GET typeahead/hitsV2?q=blended&id={trackingId}&keywords={keywords}
```

For job-specific typeahead:
```
GET typeahead/hitsV2?q=blendedJobs&keywords={keywords}
```

### 8.3 Hashtag Typeahead

```
GET typeahead/hitsV2?q=hashtags&prefix={prefix}&commentary={text}&urns=List({urn1},{urn2})
```

### 8.4 Geo Typeahead (Bing)

```
GET typeahead/hitsV2?q=bingGeo&id={trackingId}&keywords={query}&types=List(ADDRESS)
```

Facet geo typeahead includes sub-type filtering:
```
GET typeahead/hitsV2?q={type}&keywords={query}&{type}=GEO&queryContext=List(geoVersion->3,bingGeoSubTypes->MARKET_AREA|COUNTRY_REGION|ADMIN_DIVISION_1)
```

### 8.5 Federated Typeahead

General-purpose typeahead with type filtering:

```
GET typeahead/hits?q=federated&types=List(PEOPLE,COMPANY)&origin={origin}&query={keywords}
```

Optional: `shouldUseSchoolParams=true` for school context.

### 8.6 Mentions Typeahead

```
GET typeahead/hits?q=mentions&query={keywords}&origin={origin}
```

### 8.7 TypeaheadType Enum

Available typeahead entity types:

`MY_NETWORK`, `PEOPLE`, `CONNECTIONS`, `AUTO_COMPLETE`, `COMPANY`, `SCHOOL`, `GEO`, `GEO_REGION`, `TITLE`, `FIELD_OF_STUDY`, `REGION`, `DEGREE`, `GROUP`, `SITE_FEATURE`, `SHOWCASE`, `SKILL`, `SUGGESTION`, `INDUSTRY`, `GROUP_MEMBERS`, `JOB_FUNCTION`, `CITY`, `LANGUAGE`, `ESCAPE_HATCH`, `BING_GEO`, `HASHTAG`, `PROFESSIONAL_EVENT`

---

## 9. Search History

### 9.1 View History

```
GET search/history
```

### 9.2 Update History

```
GET search/history?action=update
```

### 9.3 Clear History

```
GET search/history?action=dismiss
```

### 9.4 SearchHistory Model

History entries are typed unions containing one of:
- `SearchHistoryProfile`
- `SearchHistoryCompany`
- `SearchHistoryGroup`
- `SearchHistoryJob`
- `SearchHistorySchool`
- `SearchHistoryArticle`
- `SearchHistoryTrendingResultContainer`

---

## 10. Wire Format Example

### 10.1 People Search with Facets

```
GET /voyager/api/search/cluster
  ?q=guided
  &searchId={uuid}
  &origin=GLOBAL_SEARCH_HEADER
  &timestamp=1711267200000
  &guides=List(v->people,facetIndustry->96,facetGeoRegion->103644)
  &keywords=software%20engineer
  &start=0
  &count=10
```

### 10.2 Job Search

```
GET /voyager/api/search/hits
  ?q=jserpAll
  &searchId={uuid}
  &origin=JOB_SEARCH_PAGE_SEARCH_BUTTON
  &timestamp=1711267200000
  &facetJobType=List(F)
  &facetExperience=List(2)
  &geoUrn=urn:li:fs_geo:92000000
  &keywords=backend%20engineer
  &decorationId=com.linkedin.voyager.deco.jobs.search.ListedJobSearchHit-50
```

### 10.3 Blended SRP (Search Results Page)

```
GET /voyager/api/search/cluster
  ?q=guided
  &searchId={uuid}
  &origin=GLOBAL_SEARCH_HEADER
  &timestamp=1711267200000
  &guides=List(v->people)
  &blendedSrpEnabled=true
  &relatedSearchesEnabled=true
  &keywords=data%20science
```

---

## 11. Key Implementation Notes

1. **Rest.li encoding matters**: All query parameters go through Rest.li's `AsciiHexEncoding`. Characters `( ) , ' :` are percent-encoded with `%` prefix. Standard URL encoding is applied on top.

2. **Facet params vs regular params**: The `FacetParameterMap.isFacetParam()` method simply checks if the parameter name starts with `"facet"`. This is the sole heuristic.

3. **Guides are ordered**: The first guide is always the vertical (`v->type`), followed by facet guides. Order matters for the server-side interpretation.

4. **Content search facets differ**: Content search uses raw facet list format (`facetName->value` per entry), while people search joins multi-valued facets with `|` pipe separator.

5. **Job search uses a different query style**: Job SERP uses `q=jserpAll` with individual facet parameters (via `appendFacetParameters`), not the guides-based system. Job facet parameters use `f_` prefixed web-style names when building saved search URLs.

6. **`SearchHit.HitInfo` is a union**: The response is polymorphic. Client must check which member of the union is present to determine result type.

7. **Spell correction can be disabled**: Pass `spellCorrectionEnabled=false` to suppress automatic query rewriting.

8. **Default geo is worldwide**: If no location is specified, job search defaults to `geoUrn=urn:li:fs_geo:92000000`.

---

## 12. Source File Index

| File | Package | Key Content |
|------|---------|-------------|
| `SearchRoutes.java` | `c.l.a.search.shared` | URL construction for all search/typeahead endpoints |
| `FacetParameterMap.java` | `c.l.a.search.facet` | Facet parameter storage, serialization, parsing |
| `SearchDataProvider.java` | `c.l.a.search` | Data fetching, response parsing orchestration |
| `SearchType.java` | `c.l.a.pegasus.gen.voyager.search` | Search vertical enum |
| `SearchFacetType.java` | `c.l.a.pegasus.gen.voyager.search` | General facet type enum |
| `PeopleSearchFacetType.java` | same | People-specific facets |
| `JobSearchFacetType.java` | same | Job-specific facets |
| `ContentSearchFacetType.java` | same | Content-specific facets |
| `SearchFilterType.java` | same | Unified v2 filter types |
| `SearchHit.java` | same | Result model with HitInfo union |
| `SearchCluster.java` | same | Clustered results container |
| `SearchMetadata.java` | same | Search response metadata |
| `SearchFacet.java` / `SearchFacetValue.java` | same | Facet response model |
| `SearchSortBy.java` | `c.l.a.pegasus.gen.voyager.search.shared` | Sort order enum |
| `Guide.java` / `GuideType.java` | `c.l.a.pegasus.gen.voyager.search` | Navigation guide model |
| `TypeaheadType.java` | `c.l.a.pegasus.gen.voyager.typeahead` | Typeahead entity types |
| `EntityAwareSearchQuery.java` | `c.l.a.pegasus.gen.voyager.search` | Query with entity suggestions |
