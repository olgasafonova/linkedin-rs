# LinkedIn Android App Architecture Overview

Reverse-engineered from `com.linkedin.android` APK, decompiled via jadx. Analysis date: 2026-03-23.

---

## 1. High-Level Architecture

The LinkedIn Android app (internally called **Voyager/Zephyr**) is a hybrid native + React Native application:

- **Native layer**: Java/Kotlin -- core networking, auth, navigation, data management
- **React Native layer**: Embedded for select UI surfaces (Fabric + TurboModules, Hermes engine)
- **Internal codename**: "Voyager" (international) / "Zephyr" (China/LinkedIn China variant)

### Key Package Map

| Package | Purpose |
|---------|---------|
| `com.linkedin.android.infra` | Core infrastructure (networking config, data management, routes, UI) |
| `com.linkedin.android.networking` | HTTP client, cookie management, Cronet engine |
| `com.linkedin.android.liauthlib` | Auth library (login, registration, SSO, OAuth) |
| `com.linkedin.android.datamanager` | Data caching and request orchestration |
| `com.linkedin.android.pegasus.gen` | **Generated models** (Rest.li Pegasus codegen -- DTOs, builders) |
| `com.linkedin.android.identity` | Profile viewing/editing |
| `com.linkedin.android.feed` | Feed/posts |
| `com.linkedin.android.messaging` | Messaging/conversations |
| `com.linkedin.android.jobs` | Job search, applications |
| `com.linkedin.android.mynetwork` | Connections, PYMK |
| `com.linkedin.android.search` | Search |
| `com.linkedin.android.realtime` | Real-time subscriptions (long polling/SSE) |
| `com.linkedin.android.lixclient` | Feature flags (LinkedIn Internal eXperiments) |

---

## 2. Networking Architecture

### 2.1 Network Stack

LinkedIn does NOT use Retrofit. The networking stack is entirely custom:

```
NetworkClient (public API)
  -> LinkedInNetwork (request orchestration, headers, retries)
     -> BaseLinkedInNetwork (custom headers, status code handlers, cookies)
        -> CronetNetworkEngine (Chromium Cronet v83 -- actual HTTP)
           -> org.chromium.net.UrlRequest (HTTP/2, TLS)
```

**Key insight**: LinkedIn uses Chromium's Cronet library (v83.0.4103.83) as the HTTP transport, NOT OkHttp. OkHttp is bundled but only used by React Native / Fresco. The main app uses Cronet, which means TLS fingerprinting matches Chrome, not standard Android.

### 2.2 Base URLs

From `FlagshipSharedPreferences`:

| URL | Purpose |
|-----|---------|
| `https://www.linkedin.com` | Production base URL (API + web) |
| `https://www.linkedin-ei.com` | Engineering/staging environment |
| `https://slideshare.www.linkedin.com/upload` | Media upload endpoint |

The base URL is configurable via SharedPreferences (key: `baseUrl`). Auth URL defaults to same as base URL (key: `authUrl`).

**Important**: There is NO separate `api.linkedin.com` host. All API calls go to `www.linkedin.com` with path-based routing.

### 2.3 API Path Prefix

From `Routes.java`:

```java
public static String STR_ROOT = "/zephyr/api/";
public static String STR_ROOT_ZEPHYR = "/zephyr/api/";
```

Routes are constructed as: `{baseUrl}/zephyr/api/{route}?{params}`

However, there is also evidence of `/voyager/api/` prefix:
- `/voyager/api/feed/updates` (mock feed)
- Cookie-based host override uses `/voyager/api:` prefix

The app likely uses `/voyager/api/` as the API prefix, with `/zephyr/api/` being the China variant. The `STR_ROOT` value may be overridden at runtime via feature flags.

### 2.4 Request Headers

Every request includes these headers (set by `BaseLinkedInNetwork.getRequestHeaders` and `HeaderUtil`):

| Header | Value | Notes |
|--------|-------|-------|
| `X-RestLi-Protocol-Version` | `2.0.0` | **Always set** by `NetworkClientConfigurator.configure()` |
| `Csrf-Token` | `{JSESSIONID cookie value}` | CSRF protection (set by cookie manager) |
| `X-UDID` | `{device installation ID}` | Unique device identifier |
| `X-LI-Track` | `{JSON blob}` | Device telemetry (see below) |
| `X-LI-Lang` | `{locale}` | Language preference (e.g. `en_US`) |
| `Accept-Language` | `{locale with dash}` | e.g. `en-US` |
| `X-LI-Retry-Context` | `{JSON}` | On retries: `{"attempt": N, "errorCode": E}` |

**X-LI-Track header** is a JSON object containing:

```json
{
  "osName": "Android OS",
  "osVersion": "30",
  "clientVersion": "4.x.x",
  "clientMinorVersion": 12345,
  "carrier": "T-Mobile",
  "model": "Google_Pixel 6",
  "displayDensity": 2.625,
  "displayWidth": 1080,
  "displayHeight": 2400,
  "dpi": "xxhdpi",
  "deviceType": "android",
  "appId": "com.linkedin.android",
  "deviceId": "{uuid}",
  "timezoneOffset": -8,
  "timezone": "America/Los_Angeles",
  "storeId": "{play store id}",
  "advertiserId": "{gaid}",
  "isAdTrackingLimited": false,
  "mpName": "{mp name}",
  "mpVersion": "{mp version}"
}
```

### 2.5 Cookie Management

Managed by `LinkedInHttpCookieManager` (custom, NOT Android's CookieManager).

Key cookies:

| Cookie | Purpose |
|--------|---------|
| `JSESSIONID` | CSRF token -- value echoed as `Csrf-Token` header |
| `li_at` | **Primary auth session cookie** -- presence checked on 401 responses |
| `X-LinkedIn-traceDataContext` | Debug tracing (only for debugging) |
| `X-LinkedIn-country_override` | Country override (30-day expiry) |

JSESSIONID format: `ajax:{19-digit random number}` (generated client-side by `CsrfCookieHelper.generateJsessionId()`).

### 2.6 Query Tunneling

`QueryTunnelUtil` implements Rest.li query tunneling -- when query strings exceed URL length limits, the query parameters are moved into the POST body with the actual HTTP method overridden. This is a standard Rest.li protocol feature.

---

## 3. Rest.li Protocol Details

LinkedIn's API uses Rest.li 2.0, NOT standard REST. Key differences:

### 3.1 Protocol Version

Header `X-RestLi-Protocol-Version: 2.0.0` is sent on every request.

### 3.2 Response Headers

- `X-LinkedIn-Id` / `X-RestLi-Id` -- entity ID from creation responses (checked by `RestliUtils.getIdFromListHeader`)

### 3.3 URL Encoding

Rest.li uses a custom encoding scheme for query parameters. Special characters `( ) , ' :` are encoded using `AsciiHexEncoding` with `%` as the escape character. This is handled by `RestliUtils.ValueCodec`.

### 3.4 Data Types

Parameters are typed using `com.linkedin.data.lite.DataType`:
- `STRING`, `ARRAY`, `RECORD` -- determines encoding format
- Records implement `RecordTemplate` interface (Pegasus codegen)

### 3.5 Pagination

From `Routes.addPagingParameters`:

```
?start={offset}&count={pageSize}&paginationToken={token}
```

Standard Rest.li collection pagination with 0-based offset.

### 3.6 Decoration (Field Projection)

The `decorationId` query parameter controls field projection (which fields/sub-objects to include in the response). This is analogous to GraphQL field selection but Rest.li-specific.

```java
RestliUtils.appendRecipeParameter(uri, "com.linkedin.voyager.deco.identity.FullProfile")
```

---

## 4. Authentication Flow

### 4.1 Primary Auth: Cookie-Based Login

The main auth flow uses `POST /uas/authenticate` with form-encoded credentials:

```
POST {baseUrl}/uas/authenticate
Content-Type: application/x-www-form-urlencoded

session_key={email}&session_password={password}
```

Supported credential types (from `LiAuthImpl.authenticate`):
- `session_key` + `session_password` -- email/password login
- `session_midToken` -- mid-token login (fastrack)
- `googleIdToken` -- Google Sign-In
- `flashIdToken` + `flashAuthInfoId` -- Flash ID token
- `appleIdToken` + `appleAuthCode` -- Apple Sign-In
- `Challenge_id` -- challenge response (CAPTCHA)
- `rememberMeOptIn` -- remember me preference
- `client_enabled_features` -- e.g. `ANDROID_NATIVE_CAPTCHA`

Response on success (200): `{"login_result": "PASS"}`

After successful auth:
1. Session cookies are set by the server (including `li_at`)
2. `POST /uas/issueLoginCookie` is called to get additional cookies
3. Profile data is fetched

### 4.2 CSRF Flow

1. Client generates `JSESSIONID` cookie: `ajax:{19-digit random}`
2. On every request, the `JSESSIONID` value is echoed as `Csrf-Token` header
3. If no `JSESSIONID` exists, `GET /uas/authenticate` is called to obtain one
4. The CSRF token is required for all mutating operations

### 4.3 Third-Party OAuth (Mobile SDK)

For third-party apps using LinkedIn SSO:

```
GET {baseUrl}/oauth/mobilesdk/authorization
  ?response_type={code}
  &client_id={packageName}
  &packageHash={hash}
  &scope={scopes}
  &redirect_uri={uri}
  &code_challenge={pkce_challenge}
  &code_challenge_method={S256}
  &state={state}
  &_l={locale}
```

This is OAuth 2.0 with PKCE. The authorization endpoint is `/oauth/mobilesdk/authorization`.

Authorization grant: `POST /uas/mobilesdk/authorize` with form-encoded parameters including `scope`, `csrfToken`, `packageName`, `packageHash`, `userAuthorized`.

### 4.4 Google Client ID

Found in `RegistrationHelper`:
```
audience:server:client_id:789113911969.apps.googleusercontent.com
```

### 4.5 Logout

```
POST {baseUrl}/uas/directLogout
```

### 4.6 Session Validation

On HTTP 401, `UnauthorizedStatusCodeHandler`:
1. Checks if `li_at` cookie exists
2. If missing: immediately logs user out
3. If present but 401: either reports non-fatal (feature flag) or logs out
4. Background: calls `Auth.signOut()`
5. Foreground: navigates to `LoginActivity`

---

## 5. API Routes Catalog

The `Routes` enum (`com.linkedin.android.infra.shared.Routes`) defines ~200 API routes. Routes are relative paths appended to the API root (`/zephyr/api/` or `/voyager/api/`).

### 5.1 Core Domain Routes

#### Identity / Profile

| Route Constant | Path | Purpose |
|---------------|------|---------|
| `ME` | `me` | Current user info |
| `SETTINGS` | `me/settings` | User settings |
| `PROFILE` | `identity/profiles` | View profiles |
| `NORMALIZED_PROFILE` | `identity/normProfiles` | Normalized profiles |
| `MINIPROFILE` | `identity/miniprofiles` | Mini profile (lightweight) |
| `PROFILE_COMPLETENESS` | `identity/profileCompleteness` | Profile completeness score |
| `DASHBOARD` | `identity/profile/dashboard` | Profile dashboard |
| `IDENTITY_PROFILES` | `voyagerIdentityProfiles` | Profile data (v2) |
| `SEARCH_APPEARANCES` | `voyagerIdentitySearchAppearances` | "Who searched for you" |
| `MEMBER_PHONE_NUMBER` | `identity/phoneNumbers` | Phone numbers |

#### Feed / Content

| Route Constant | Path | Purpose |
|---------------|------|---------|
| `FEED` | `feed/updates` | Feed posts |
| `FEED_BADGING` | `feed/badge` | Feed badge count |
| `FEED_LIKES` | `feed/likes` | Post likes |
| `FEED_COMMENTS` | `feed/comments` | Comments |
| `FEED_URL_PREVIEW` | `feed/urlpreview` | URL preview |
| `FEED_SOCIAL` | `feed/social` | Social interactions |
| `FEED_TOPICS` | `feed/topics` | Content topics |
| `FOLLOWS` | `feed/follows` | Follow entities |
| `CONTENT_CREATION` | `contentcreation/normShares` | Create posts |
| `PUBLISHING_ARTICLES` | `publishing/firstPartyArticles` | Articles |
| `PUBLISHING_CONTENT` | `publishing/firstPartyContent` | First-party content |

#### Messaging

| Route Constant | Path | Purpose |
|---------------|------|---------|
| `MESSAGING_ROOT` | `messaging` | Messaging root |
| `MESSAGING_CONVERSATIONS` | `messaging/conversations` | Conversations list |
| `MESSAGING_BADGING` | `messaging/badge` | Unread count |
| `MESSAGING_INMAIL_CREDITS` | `messaging/credits` | InMail credits |
| `MESSAGING_TYPEAHEAD` | `messaging/typeahead/hits` | Recipient search |
| `MESSAGING_PRESENCE_STATUSES` | `messaging/presenceStatuses` | Online status |
| `MESSAGING_SYNC_CONVERSATIONS` | `messaging/sync/conversations` | Sync conversations |
| `MESSAGING_CONVERSATION_ID` | `messaging/conversationId` | Get conversation by members |

#### Jobs

| Route Constant | Path | Purpose |
|---------------|------|---------|
| `JOB_POSTINGS` | `jobs/jobPostings` | Job listings |
| `JOB_APPLICATIONS` | `jobs/jobApplications` | Applied jobs |
| `JOB_RECOMMENDATIONS` | `jobs/jobPostingRecommendations` | Recommended jobs |
| `JOB_SEARCH` | `jobs/search` | Job search |
| `JOB_SEEKER_PREFERENCES` | `jobs/jobSeekerPreferences` | Job preferences |
| `JOB_SALARY_STATUS` | `zephyrSalarySubmissionStatus` | Salary info |
| `JOBS_HOME_TEMPLATES` | `jobs/jobsHomeTemplates` | Jobs home page |
| `JOB_MEMBER_RESUME` | `jobs/resumes` | Resume data |

#### Network / Connections

| Route Constant | Path | Purpose |
|---------------|------|---------|
| `CONNECTIONS` | `relationships/connections` | Connection list |
| `CONNECTIONS_SUMMARY` | `relationships/connectionsSummary` | Connection stats |
| `RELATIONSHIPS_PYMKS` | `relationships/pymks` | People You May Know |
| `RELATIONSHIPS_INVITATIONS` | `relationships/invitations` | Pending invitations |
| `RELATIONSHIPS_BADGING` | `relationships/badge` | Network badge |

#### Search

| Route Constant | Path | Purpose |
|---------------|------|---------|
| `SEARCH` | `search/hits` | Search results |
| `SEARCH_BLENDED` | `search/blended` | Blended search |
| `SEARCH_HISTORY` | `search/history` | Search history |
| `TYPEAHEAD` | `typeahead/hits` | Typeahead suggestions |

#### Notifications

| Route Constant | Path | Purpose |
|---------------|------|---------|
| `NOTIFICATION_CARDS` | `identity/notificationCards` | Notification list |
| `NOTIFICATION_SETTINGS` | `identity/notificationSettings` | Notification prefs |

### 5.2 Other Notable Endpoints

| Endpoint | Purpose |
|----------|---------|
| `configuration` | App configuration / feature flags |
| `lixTreatments` | A/B test treatments (LinkedIn Internal Experiments) |
| `mux` / `mux/secure` | Request multiplexing (batch multiple requests) |
| `pushRegistration` | Push notification registration |
| `/realtime/realtimeFrontendSubscriptions` | Real-time event subscriptions |
| `/realtime/realtimeFrontendTimestamp` | Real-time server timestamp |
| `mupld/upload` | File upload |

---

## 6. Data Layer

### 6.1 Pegasus Generated Models

The `com.linkedin.android.pegasus.gen` package contains **~1458 model classes** (excluding builders) generated by LinkedIn's Pegasus code generator from Rest.li IDL schemas.

Key model namespaces under `pegasus.gen.voyager`:

| Namespace | Contents |
|-----------|----------|
| `common` | Shared types: `Me`, `Urn`, `Locale`, `CollectionMetadata`, `GraphDistance`, etc. |
| `identity.profile` | `Profile`, `ProfileView`, `Education`, `Certification`, `Skill`, `Position`, etc. |
| `identity.shared` | `MiniProfile` (lightweight profile) |
| `feed` | `Update`, `Comment`, `ShareUpdateContent`, `SocialDetail`, etc. |
| `feed.render` | `UpdateV2`, `UpdateSummary` (v2 feed rendering) |
| `messaging` | `Conversation`, `Event`, `MessageEvent`, `Credits`, etc. |
| `messaging.event` | `MessageEvent`, `CustomContent`, etc. |
| `jobs` | `JobPosting`, `JobApplication`, `JobSeekerPreferences`, etc. |
| `relationships` | Connection/invitation types |
| `search` | `SearchProfile`, `SearchHistory`, etc. |
| `organization` | `Company`, etc. |
| `growth` | Onboarding, invitations |
| `typeahead` | Typeahead suggestions |
| `video` | Video content |

### 6.2 Data Manager

`FlagshipDataManager` extends `DataManager` which handles:
- Network requests via `NetworkDataStore`
- Local caching via `LocalDataStore` (backed by LMDB)
- Consistency management via `ConsistencyManager` (strategy: `UPDATE_VOYAGER_LEGACY`)
- Response parsing via `DataResponseParserFactory` (deserializes Rest.li JSON into Pegasus models)

### 6.3 Data Request Pattern

From `ProfileDataProvider` and similar classes, the typical data fetch pattern is:

```java
DataManager dataManager = ...;
DataRequest request = new DataRequest(Routes.PROFILE.buildUponRoot()
    .buildUpon()
    .appendPath(memberId)
    .build());
dataManager.fetch(request, listener);
```

---

## 7. Real-Time Communication

### 7.1 Subscriptions

`com.linkedin.android.realtime.internal.HttpUtils` reveals the real-time system:

- Subscription endpoint: `/realtime/realtimeFrontendSubscriptions`
- Timestamp endpoint: `/realtime/realtimeFrontendTimestamp`
- Topic format: `(topic:{urn},clientConnectionId:{connectionId})`
- Topics are URN-based (e.g., `urn:li:messaging:...`)
- Uses long-polling via `LongPollStreamNetworkClient`

### 7.2 Content Provider

`com.linkedin.android.RealTimeSystemSubscriptions` content provider manages subscription state internally.

---

## 8. Obfuscation Notes

- **Meituan Robust**: The app uses Meituan Robust hotfix framework (`com.meituan.robust.PatchProxy`). Nearly every method has a PatchProxy check at the start. This is NOT obfuscation -- it's a hotfix/patching mechanism. Every method can be replaced at runtime without an app update.
- **ProGuard/R8**: Resources are obfuscated (single-letter directory names in `r/`). Class names in the networking and infra packages appear relatively well-preserved.
- **Decompilation quality**: jadx output is quite readable. Most important classes have meaningful names.

---

## 9. Key Insights for API Replication

1. **Base URL**: `https://www.linkedin.com` (no separate API subdomain)
2. **API prefix**: `/voyager/api/` (or `/zephyr/api/` for China)
3. **Protocol**: Rest.li 2.0 with custom encoding, NOT standard REST
4. **Transport**: Cronet (Chromium network stack) -- TLS fingerprint matters
5. **Auth**: Cookie-based (`li_at` + `JSESSIONID`), NOT bearer tokens
6. **CSRF**: `JSESSIONID` cookie value echoed as `Csrf-Token` header
7. **Required headers**: `X-RestLi-Protocol-Version`, `Csrf-Token`, `X-LI-Track`, `X-UDID`
8. **Pagination**: `start` + `count` + optional `paginationToken`
9. **Field projection**: `decorationId` parameter controls response fields
10. **Query tunneling**: Long queries moved to POST body (standard Rest.li)
11. **Batch requests**: `mux` endpoint for request multiplexing
12. **Real-time**: Long-polling to `/realtime/realtimeFrontendSubscriptions`

---

## 10. Auth Endpoints Summary

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/uas/authenticate` | GET | Obtain CSRF token (JSESSIONID cookie) |
| `/uas/authenticate` | POST | Login with credentials |
| `/uas/issueLoginCookie` | POST | Get additional session cookies |
| `/uas/directLogout` | POST | Logout |
| `/oauth/mobilesdk/authorization` | GET | Third-party OAuth authorization |
| `/uas/mobilesdk/authorize` | POST | Third-party OAuth token grant |
| `/checkpoint/login/fastrackProfileV2` | POST | Fast-track profile fetch |
| `/checkpoint/lg/login` | -- | Login checkpoint redirect |
