# Rest.li 2.0 Protocol Specification (LinkedIn Android Client)

Reverse-engineered from `com.linkedin.android` APK decompilation. This document covers the
exact wire-level protocol details needed to build a compatible HTTP client in Rust.

---

## 1. Protocol Version

Every API request MUST include:

```
X-RestLi-Protocol-Version: 2.0.0
```

Set unconditionally by `NetworkClientConfigurator.configure()`. There is no version negotiation;
the app hardcodes `2.0.0`.

Source: `com.linkedin.android.infra.network.NetworkClientConfigurator`

---

## 2. URL Encoding (Rest.li Custom Encoding)

Rest.li does NOT use standard percent-encoding for query parameter values. It uses a
two-layer encoding scheme.

### 2.1 Layer 1: AsciiHexEncoding

The innermost layer encodes Rest.li reserved characters using `%` as the escape character,
converting each reserved char to `%XX` (uppercase hex).

**Reserved characters** (from `DataCodecConstants.RESERVED_CHARS`):

```
( ) : , '
```

Encoding rules:
- The `%` character itself is also encoded (as `%25`)
- Each reserved char becomes `%` + two uppercase hex digits of its ASCII code
- Non-ASCII characters are NOT handled by this layer

**Encoding table:**

| Character | Encoded |
|-----------|---------|
| `(`       | `%28`   |
| `)`       | `%29`   |
| `:`       | `%3A`   |
| `,`       | `%2C`   |
| `'`       | `%27`   |
| `%`       | `%25`   |

Source: `com.linkedin.data.lite.restli.AsciiHexEncoding`

### 2.2 Layer 2: URI-Context-Aware Encoding (ValueCodec)

After AsciiHexEncoding, the `ValueCodec` applies additional encoding depending on whether
the value appears in a URI path or query parameter:

- **URI_QUERY context**: characters ` `, `#`, `&` are percent-encoded using standard URI encoding
  (e.g., ` ` -> `%20`, `#` -> `%23`, `&` -> `%26`)
- **URI_PATH context**: characters ` `, `/`, `?`, `#` are percent-encoded
- Non-ASCII characters (outside printable ASCII range `0x20`-`0x7E`) are encoded as UTF-8 bytes,
  each byte becoming `%XX`

The `%` characters introduced by Layer 1 (AsciiHexEncoding) are preserved -- they are NOT
double-encoded. The `ValueCodec.encode()` calls `Uri.encode(str, "%")`, which treats `%` as
a safe character that should not be re-encoded.

Source: `com.linkedin.android.infra.shared.RestliUtils.ValueCodec`

### 2.3 Combined Encoding Pipeline

For a query parameter value:
1. Apply `AsciiHexEncoding('%', RESERVED_CHARS)` -- encode `( ) : , ' %`
2. Apply `ValueCodec.encode(result, URI_QUERY)` -- additionally encode ` # &` and non-ASCII

**Example**: Encoding `urn:li:member:123` as a query parameter value:
1. AsciiHexEncoding: `urn%3Ali%3Amember%3A123`
2. ValueCodec (URI_QUERY): unchanged (no `#`, `&`, or spaces)
3. Final: `urn%3Ali%3Amember%3A123`

**Example**: Encoding `John's (profile)`:
1. AsciiHexEncoding: `John%27s %28profile%29`
2. ValueCodec (URI_QUERY): `John%27s%20%28profile%29`

### 2.4 Separate Encoding for `appendEncodedQueryParameter`

`RestliUtils.appendEncodedQueryParameter()` uses a slightly different path:
1. Apply `AsciiHexEncoding('%', RESTLI_ENCODING_CHARACTERS)` -- same five reserved chars
2. Apply `Uri.encode(result, "%")` -- standard Android URI encoding, preserving `%`
3. Manually build the query string (not using Android's `appendQueryParameter`)

The `RESTLI_ENCODING_CHARACTERS` set matches `DataCodecConstants.RESERVED_CHARS`:
`( ) , ' :`

Source: `com.linkedin.android.infra.shared.RestliUtils`

---

## 3. Rest.li Data Encoding (Structured Values in URLs)

Rest.li encodes complex data types (records, arrays, maps) into URL-safe string representations
using `DataEncoder`. This is how structured query parameters are serialized.

### 3.1 Primitive Types

| Type    | Encoding              | Example          |
|---------|-----------------------|------------------|
| String  | AsciiHex + URI encode | `hello` -> `hello` |
| String (empty) | `''`           | `''`             |
| Integer | Decimal string        | `42`             |
| Long    | Decimal string        | `123456789`      |
| Float   | Decimal string        | `3.14`           |
| Double  | Decimal string        | `2.718`          |
| Boolean | `true` or `false`     | `true`           |
| Enum    | Enum name string      | `ACTIVE`         |

### 3.2 Records

Records are encoded as parenthesized, comma-separated `field:value` pairs:

```
(field1:value1,field2:value2,field3:value3)
```

### 3.3 Arrays (Lists)

Arrays are encoded with the `List()` wrapper:

```
List(item1,item2,item3)
```

### 3.4 Maps

Maps use the same parenthesized format as records:

```
(key1:value1,key2:value2)
```

### 3.5 Unions

Unions also use the parenthesized format with the member type as the key:

```
(memberType:value)
```

### 3.6 Nested Example

A query parameter `keywords` with value `List(software,engineer)` combined with a record
parameter `origin:(SEARCH_HOME)` would produce a query string like:

```
?keywords=List(software,engineer)&origin=(SEARCH_HOME)
```

After encoding the Rest.li reserved characters through AsciiHexEncoding + URI encoding:

```
?keywords=List%28software%2Cengineer%29&origin=%28SEARCH_HOME%29
```

Source: `com.linkedin.data.lite.restli.DataEncoder`

---

## 4. Query Tunneling

When a request URL exceeds length limits, Rest.li "tunnels" the query through a POST body.

### 4.1 Trigger Conditions

From `LiNetworkDataStore.shouldForceQueryTunnel()`:

1. `DataRequest.forceQueryTunnel == true` (explicit opt-in per request), OR
2. Encoded query string length > **4096** characters, OR
3. Encoded path length > **4096** characters (logged as error)

### 4.2 Tunneling Mechanism

When triggered, `QueryTunnelUtil.getTunnelRequest()`:

1. **Changes HTTP method to POST** (regardless of original method)
2. **Adds header**: `X-HTTP-Method-Override: {original_method}` (e.g., `GET`)
3. **Moves query parameters to POST body** as `multipart/mixed`:
   - Part 1: `Content-Type: application/x-www-form-urlencoded` containing the query string
     (key=value pairs joined by `&`)
   - Part 2: Original request body (if any)
4. **Strips query from URL**: The URL becomes just the path (with scheme/authority)
5. **Sticky parameters**: Some parameters can be kept in the URL via `stickyParamKeysDuringQueryTunnel`
   (these stay in the URL and are removed from the POST body)

### 4.3 Wire Format Example

Original request:
```
GET /voyager/api/search/hits?keywords=...&decorationId=...&count=25&start=0
```

After tunneling:
```
POST /voyager/api/search/hits
X-HTTP-Method-Override: GET
Content-Type: multipart/mixed; boundary=...

--boundary
Content-Type: application/x-www-form-urlencoded

keywords=...&decorationId=...&count=25&start=0
--boundary--
```

Source: `com.linkedin.android.networking.util.QueryTunnelUtil`,
`com.linkedin.android.datamanager.impl.LiNetworkDataStore`

---

## 5. Content Negotiation

### 5.1 Accept Header

The `Accept` header varies based on request type (from `BaseHttpRequest.getAdditionalHeaders()`):

| Condition | Accept Header |
|-----------|---------------|
| `decorationId` present in URL | `application/vnd.linkedin.deduped+x-protobuf+2.0` |
| API request without `decorationId` | `application/vnd.linkedin.deduped+x-protobuf` |
| Debug/hierarchical JSON forced | `application/vnd.linkedin.mobile.debug+json` |
| Non-API request | No custom Accept header |

**Key insight**: The production app uses a custom **protobuf** wire format by default, NOT JSON.
The `deduped` qualifier means the response uses deduplication (shared entities referenced by ID
rather than inlined). The `+2.0` suffix on decorated requests indicates protobuf format version 2.

### 5.2 Symbol Table

API requests include:
```
x-restli-symbol-table-name: zephyr-{N}
```

Where `{N}` is the size of the client's symbol table. This is used for protobuf field name
compression -- field names are replaced with numeric IDs from a shared symbol table.

### 5.3 Content-Type for Request Bodies

- Protobuf: `application/x-protobuf2 ;symbol-table=zephyr-{N}`
- JSON: `application/json`

### 5.4 Response Parsing

`ContentTypeProvider` determines the parser based on response Content-Type:

| Content-Type contains | Parser |
|-----------------------|--------|
| `application/json` | JSON parser |
| `application/vnd.linkedin.mobile.debug+json` | JSON parser (debug only) |
| `application/vnd.linkedin.mobile.deduped+json` | JSON parser (deduped) |
| `application/x-protobuf2` | Protobuf parser |
| `application/vnd.linkedin.deduped+x-protobuf` | Protobuf parser (deduped) |

**For our Rust client**: Request `application/json` via Accept header to avoid needing to
reverse-engineer the protobuf schema. Use `application/vnd.linkedin.mobile.debug+json` to force
hierarchical (non-deduped) JSON responses for simpler parsing. If that triggers detection,
fall back to `application/json`.

Source: `com.linkedin.android.infra.network.BaseHttpRequest`,
`com.linkedin.android.datamanager.ContentTypeProvider`

---

## 6. Decoration / Recipe System (Field Projection)

### 6.1 Overview

Rest.li's "decoration" system controls which fields are included in API responses. It is
analogous to GraphQL's field selection. Each "recipe" (decoration) is a named, versioned
projection specification stored server-side.

### 6.2 Usage

The `decorationId` query parameter specifies the recipe:

```
?decorationId=com.linkedin.voyager.deco.jobs.shared.FullJobPosting-71
```

`RestliUtils.appendRecipeParameter(uri, recipeId)` simply adds the `decorationId` query
parameter to the URI.

### 6.3 Recipe ID Format

Recipe IDs follow the pattern:

```
com.linkedin.voyager.deco.{domain}.{name}-{version}
```

**Known recipes** (from decompiled code):

| Recipe ID | Used For |
|-----------|----------|
| `com.linkedin.voyager.deco.jobs.shared.FullJobPosting-71` | Full job posting details |
| `com.linkedin.voyager.deco.jobs.shared.ListedJobPosting-43` | Job listing in lists |
| `com.linkedin.voyager.deco.jobs.search.ListedJobSearchHit-50` | Job search results |
| `com.linkedin.voyager.deco.jobs.ListedJobPostingRecommendation-45` | Recommended jobs |
| `com.linkedin.voyager.deco.identity.normalizedprofile.shared.ApplicantProfile-13` | Profile in job context |
| `com.linkedin.voyager.deco.identity.normalizedprofile.shared.ListedProfile-6` | Profile in lists |
| `com.linkedin.voyager.deco.organization.shared.FullCompany-40` | Full company page |
| `com.linkedin.voyager.deco.organization.shared.CompactCompany-5` | Compact company |
| `com.linkedin.voyager.deco.jobs.FullJobSeekerPreferences-32` | Job seeker preferences |
| `com.linkedin.voyager.deco.jobs.premiuminsights.ListedTopApplicantJobs-39` | Premium job insights |
| `com.linkedin.voyager.deco.jobs.premiuminsights.FullApplicantInsights-6` | Full applicant insights |
| `com.linkedin.voyager.deco.jobs.premiuminsights.ApplicantRankInsights-5` | Applicant rank |
| `com.linkedin.voyager.deco.jobs.premiuminsights.FullCompanyInsights-11` | Company insights |
| `com.linkedin.voyager.deco.organization.shared.FullSchoolV2-19` | School page |
| `com.linkedin.voyager.deco.organization.shared.EmployeeCulturalInsights-18` | Cultural insights |
| `com.linkedin.voyager.deco.organization.landingpage.FullLandingPageContents-12` | Company landing page |
| `com.linkedin.voyager.deco.organization.shared.FullTargetedContent-16` | Targeted content |
| `com.linkedin.voyager.deco.jobs.FullJobsHomePreferencesTemplate-9` | Jobs home page |

### 6.4 Effect on Accept Header

When `decorationId` is present in the query string, the Accept header changes:
- Without decoration: `application/vnd.linkedin.deduped+x-protobuf`
- With decoration: `application/vnd.linkedin.deduped+x-protobuf+2.0`

### 6.5 Real-Time Recipe Headers

For real-time (SSE) connections, recipes are passed differently:
- `x-li-recipe-map`: JSON object mapping topic types to recipe IDs
- `x-li-recipe-accept`: Accept type for recipe responses
- `x-li-accept`: Alternative accept header

Source: `com.linkedin.android.infra.shared.RestliUtils.appendRecipeParameter()`,
`com.linkedin.android.infra.network.BaseHttpRequest.isDecoRequest()`,
`com.linkedin.android.realtime.internal.HttpUtils`

---

## 7. Collection Response Format

All list/collection API endpoints return responses following the `CollectionTemplate` schema.

### 7.1 Top-Level Structure

```json
{
  "elements": [ ... ],
  "paging": {
    "start": 0,
    "count": 10,
    "total": 42,
    "links": [...]
  },
  "metadata": { ... },
  "entityUrn": "urn:li:...",
  "isError": false,
  "error": null
}
```

JSON key ordinals (from `CollectionTemplate.JSON_KEY_STORE`):

| Key         | Ordinal | Description |
|-------------|---------|-------------|
| `elements`  | 0       | Array of result items |
| `metadata`  | 1       | Collection-specific metadata (type varies by endpoint) |
| `paging`    | 2       | Pagination info (`CollectionMetadata`) |
| `entityUrn` | 3      | URN identifying this collection |
| `isError`   | 4       | Boolean error flag |
| `error`     | 5       | Error details (`ErrorResponse`) |

### 7.2 Paging Object (`CollectionMetadata`)

```json
{
  "start": 0,
  "count": 10,
  "total": 42,
  "links": [
    { "rel": "next", "href": "/voyager/api/..." }
  ]
}
```

| Field   | Type  | Required | Description |
|---------|-------|----------|-------------|
| `start` | int   | Yes      | 0-based offset of current page |
| `count` | int   | Yes      | Number of items requested (page size) |
| `total` | int   | No       | Total number of items (may be absent) |
| `links` | array | No       | HATEOAS-style links (rarely used by mobile client) |

### 7.3 Pagination Request Parameters

From `Routes.addPagingParameters()`:

```
?start={offset}&count={pageSize}&paginationToken={token}
```

| Parameter         | Type   | Required | Description |
|-------------------|--------|----------|-------------|
| `start`           | int    | Yes      | 0-based offset |
| `count`           | int    | Yes      | Page size |
| `paginationToken` | string | No       | Opaque cursor for cursor-based pagination |

The client determines "has more data" by checking:
```
!paging.hasTotal || (fetchedStart + fetchedSize < paging.total)
```

### 7.4 Domain-Specific Collection Models

Some endpoints have their own collection models that embed `elements` and `paging` directly
(e.g., `Comments`, which adds `metadata` and `relevanceSortingSupported` alongside the
standard `elements`/`paging` fields). These follow the same pattern but may have additional
fields.

Source: `com.linkedin.android.pegasus.gen.collection.CollectionTemplate`,
`com.linkedin.android.pegasus.gen.restli.common.CollectionMetadata`,
`com.linkedin.android.infra.collections.CollectionTemplateHelper`

---

## 8. Error Response Format

### 8.1 Structure (`ErrorResponse`)

```json
{
  "status": 404,
  "serviceErrorCode": 100,
  "code": "RESOURCE_NOT_FOUND",
  "message": "Could not find entity",
  "docUrl": "https://...",
  "requestId": "abc-123-def",
  "exceptionClass": "com.linkedin.restli.server.RestLiServiceException",
  "stackTrace": "...",
  "errorDetailType": "..."
}
```

| Field              | Type   | Description |
|--------------------|--------|-------------|
| `status`           | int    | HTTP status code |
| `serviceErrorCode` | int    | LinkedIn-specific error code |
| `code`             | string | Machine-readable error code |
| `message`          | string | Human-readable error message |
| `docUrl`           | string | Documentation URL for the error |
| `requestId`        | string | Server-assigned request ID for debugging |
| `exceptionClass`   | string | Java exception class (debug info) |
| `stackTrace`       | string | Stack trace (debug info, usually stripped in prod) |
| `errorDetailType`  | string | Type of additional error details |

All fields are optional. In production, `exceptionClass` and `stackTrace` are typically absent.

### 8.2 Error in Collection Context

Collections can contain errors inline:

```json
{
  "elements": null,
  "paging": null,
  "isError": true,
  "error": { "status": 500, "message": "..." }
}
```

### 8.3 HTTP Status Code Handling

From `NetworkClientConfigurator`:
- **401**: Triggers `UnauthorizedStatusCodeHandler` -- checks for `li_at` cookie, may force logout
- **403**: Triggers a separate status code handler (likely rate-limit or permission denied)
- **408**: Treated as request timeout (used by real-time connections)

Source: `com.linkedin.android.pegasus.gen.restli.common.ErrorResponse`,
`com.linkedin.android.infra.network.NetworkClientConfigurator`

---

## 9. Rest.li Method Headers

For certain operations, the `X-RestLi-Method` header specifies the Rest.li operation type:

| Header Value    | Meaning |
|-----------------|---------|
| `BATCH_UPDATE`  | Update multiple entities |
| `BATCH_DELETE`  | Delete multiple entities |

This is used by the real-time subscription system:
- PUT requests to `/realtime/realtimeFrontendSubscriptions` use `X-RestLi-Method: BATCH_UPDATE`
- DELETE requests use `X-RestLi-Method: BATCH_DELETE`

Standard CRUD operations (GET, CREATE, UPDATE, DELETE) do NOT require this header -- it is
only needed for batch and action operations.

Source: `com.linkedin.android.realtime.internal.HttpUtils.getSubscribeHeaders()`

---

## 10. Response ID Headers

After entity creation, the server returns the new entity's ID in response headers:

| Header          | Purpose |
|-----------------|---------|
| `X-LinkedIn-Id` | Primary: entity ID from creation response |
| `X-RestLi-Id`   | Fallback: entity ID (Rest.li standard) |

`RestliUtils.getIdFromListHeader()` checks `X-LinkedIn-Id` first, falls back to `X-RestLi-Id`.

Source: `com.linkedin.android.infra.shared.RestliUtils.getIdFromListHeader()`

---

## 11. Batch/Mux Request Format

### 11.1 Overview

The app supports request multiplexing via Routes `MUX` (`mux`) and `SECURE_MUX` (`mux/secure`).

### 11.2 Client-Side Implementation

From the decompiled code, `MultiplexRequest` is a client-side orchestration mechanism:
- **Parallel mode**: All sub-requests are dispatched independently and responses collected
- **Sequential mode**: Sub-requests are dispatched one at a time; if a `required` request
  fails, subsequent requests are cancelled with an error

The `multiplexerUrl` field exists but is `@Deprecated`, suggesting the server-side mux endpoint
may no longer be the primary batching mechanism. The app dispatches individual HTTP requests
and manages ordering/dependency client-side.

### 11.3 Practical Implication

For the Rust client, mux is **low priority**. Individual requests are the standard path.
If batching is needed later, implement client-side parallel dispatch with response collection,
matching the app's parallel mode.

Source: `com.linkedin.android.datamanager.MultiplexRequest`

---

## 12. Summary: Minimum Viable Request

To make a valid Rest.li API call to LinkedIn, the Rust client must:

### Required Headers

```http
GET /voyager/api/{route}?{params} HTTP/2
Host: www.linkedin.com
X-RestLi-Protocol-Version: 2.0.0
Csrf-Token: ajax:{19-digit-random}
Cookie: JSESSIONID="ajax:{19-digit-random}"; li_at={session-token}
Accept: application/json
```

Notes:
- Use `application/json` for Accept to get JSON responses (simpler than protobuf)
- The `Csrf-Token` header value MUST match the `JSESSIONID` cookie value
- The `li_at` cookie is the auth session token obtained from login

### Optional but Recommended Headers

```http
X-LI-Track: {"osName":"...", "clientVersion":"...", ...}
X-UDID: {device-id}
X-LI-Lang: en_US
Accept-Language: en-US
```

### Query Parameter Encoding

Use the two-layer encoding (AsciiHexEncoding + URI encoding) for all parameter values
that may contain Rest.li reserved characters `( ) : , '`.

### Query Tunneling

If the full URL exceeds ~4096 characters in the query string, convert to POST with
`X-HTTP-Method-Override` header and move query parameters to a multipart body.

### Pagination

Use `?start=0&count=25` for paginated endpoints. Check `paging.total` in the response
(if present) to determine if more pages exist.

### Field Projection

Add `?decorationId=com.linkedin.voyager.deco.{domain}.{RecipeName}-{version}` to control
which fields are returned. Without a decoration, you may get a minimal or default response.
