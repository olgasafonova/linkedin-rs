# Device Fingerprinting & Header Requirements

Reverse-engineered from `com.linkedin.android` APK (jadx decompilation). Analysis date: 2026-03-24.

---

## 1. X-LI-Track Header

The `X-LI-Track` header is a **JSON string** sent on every API request. It contains device telemetry data used for analytics and likely for bot detection. The header is constructed once per app session (cached after first build) and only invalidated on timezone changes.

### 1.1 Source Classes

There are **two independent implementations** of X-LI-Track generation in the codebase:

| Class | Used By | Notes |
|-------|---------|-------|
| `com.linkedin.android.networking.util.XLiTrackHeader` | Main app (`BaseLinkedInNetwork` via `HeaderUtil.setDefaultHeaders`) | Full-featured, includes AppConfig fields |
| `com.linkedin.android.liauthlib.network.NetworkUtils` | Auth library (`LoginHelper`, `RegistrationHelper`) | Simplified variant used during login/registration |

The main-app version is the canonical one used for all authenticated API calls. The auth-library version is used only during pre-auth flows (login, registration, fastrack profile).

### 1.2 Main App JSON Structure (XLiTrackHeader.initJson)

Source: `com.linkedin.android.networking.util.XLiTrackHeader.initJson()`

```json
{
  "osName": "Android OS",
  "osVersion": "34",
  "clientVersion": "4.2.1058",
  "clientMinorVersion": 562100,
  "carrier": "T-Mobile",
  "model": "Google_Pixel 8",
  "displayDensity": 2.625,
  "displayWidth": 1080,
  "displayHeight": 2400,
  "dpi": "xxhdpi",
  "deviceType": "android",
  "appId": "com.linkedin.android",
  "deviceId": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "timezoneOffset": -8.0,
  "timezone": "America/Los_Angeles",
  "storeId": "com.linkedin.android",
  "advertiserId": "00000000-0000-0000-0000-000000000000",
  "isAdTrackingLimited": true,
  "mpName": "voyager-android",
  "mpVersion": "4.2.1058"
}
```

### 1.3 Field Reference

| Field | Type | Source | Required | Description |
|-------|------|--------|----------|-------------|
| `osName` | string | Hardcoded | Yes | Always `"Android OS"` |
| `osVersion` | string | `Build.VERSION.SDK_INT` | Yes | Android API level as string (e.g. `"34"` for Android 14) |
| `clientVersion` | string | `PackageInfo.versionName` | Yes | App version string (e.g. `"4.2.1058"`) |
| `clientMinorVersion` | int | `PackageInfo.versionCode` | Yes | Numeric build/version code (e.g. `562100`) |
| `carrier` | string/null | `TelephonyManager.getSimOperatorName()` | No | URI-encoded carrier name; `null` if no phone permission |
| `model` | string | `Build.MANUFACTURER + "_" + Build.MODEL` | Yes | Format: `"Manufacturer_Model"` (e.g. `"Google_Pixel 8"`) |
| `displayDensity` | float | `DisplayMetrics.density` | Yes | Screen density ratio (e.g. `2.625`) |
| `displayWidth` | int | `DisplayMetrics.widthPixels` | Yes | Screen width in pixels |
| `displayHeight` | int | `DisplayMetrics.heightPixels` | Yes | Screen height in pixels |
| `dpi` | string | Calculated from `DisplayMetrics.densityDpi` | Yes | DPI bucket name (see table below) |
| `deviceType` | string | Hardcoded | Yes | Always `"android"` |
| `appId` | string | `Context.getPackageName()` | Yes | Always `"com.linkedin.android"` |
| `deviceId` | string | `Installation.id()` | Yes | UUID v4, persisted to file (see section 2) |
| `timezoneOffset` | float | `TimeZone.getDefault().getOffset() / 3600000.0` | Yes | Hours offset from UTC (e.g. `-8.0`, `5.5`) |
| `timezone` | string/null | `TimeZone.getDefault().getID()` | No | IANA timezone ID (e.g. `"America/Los_Angeles"`); omitted if null |
| `storeId` | string | `AppConfig.getStoreId()` | Conditional | Only if AppConfig is non-null |
| `advertiserId` | string | `AppConfig.getAdvertiserId()` | Conditional | Google Advertising ID (GAID); zeroed if opt-out |
| `isAdTrackingLimited` | boolean | `AppConfig.isAdTrackingLimited()` | Conditional | Ad tracking opt-out flag |
| `mpName` | string | `AppConfig.getMpName()` | Conditional | Internal app name (likely `"voyager-android"`) |
| `mpVersion` | string | `AppConfig.getMpVersion()` | Conditional | Internal version string |

### 1.4 DPI Bucket Mapping

From `XLiTrackHeader.populateDisplayParameters()`:

| densityDpi | dpi string |
|------------|------------|
| 120 | `"ldpi"` |
| 160 | `"mdpi"` |
| 213 | `"tvdpi"` |
| 240 | `"hdpi"` |
| 320 | `"xhdpi"` |
| 480 | `"xxhdpi"` |
| 640 | `"xxxhdpi"` |
| (other) | `"xhdpi"` (default) |

### 1.5 Auth Library Variant (NetworkUtils.getXLiTrackHeader)

The auth library builds a **different** X-LI-Track during login/registration:

```json
{
  "appId": "com.linkedin.android",
  "carrier": "Google",
  "clientVersion": "4.2.1058",
  "clientMinorVersion": "562100",
  "deviceType": "android",
  "dpi": "xxhdpi",
  "language": "en_US",
  "model": "Google_Pixel 8",
  "osName": "ANDROID OS/14",
  "osVersion": "14",
  "timezone": "-8"
}
```

Key differences from main app variant:

| Aspect | Main App (`XLiTrackHeader`) | Auth Library (`NetworkUtils`) |
|--------|---------------------------|-------------------------------|
| `osName` | `"Android OS"` | `"ANDROID OS/{Build.VERSION.RELEASE}"` |
| `osVersion` | `SDK_INT` as string (e.g. `"34"`) | `Build.VERSION.RELEASE` (e.g. `"14"`) |
| `carrier` | SIM operator name (via TelephonyManager) | `Build.BRAND` (device brand, not carrier) |
| `clientMinorVersion` | int | string |
| `timezone` | IANA ID (e.g. `"America/Los_Angeles"`) | Hour offset as string (e.g. `"-8"`) |
| `language` | Not included | Locale string (e.g. `"en_US"`) |
| `displayDensity/Width/Height` | Included | Not included |
| `deviceId` | Included | Not included |
| `timezoneOffset` | Float hours | Not included (timezone field holds the offset) |
| AppConfig fields | Included if AppConfig present | Not included |

**Implication for Rust client**: During authentication (login), use the auth-library format. After authentication, switch to the main-app format for all API calls.

---

## 2. Device ID (X-UDID / Installation ID)

### 2.1 Generation

Source: `com.linkedin.android.networking.util.Installation`

The device ID is a **UUID v4** (random):
1. On first run, `UUID.randomUUID().toString()` is generated
2. Written to `{app_data}/files/INSTALLATION` as plain text
3. Cached in memory as `Installation.sID`
4. Persists across app restarts (but NOT across reinstalls)

Format: standard UUID v4 string, e.g. `"a1b2c3d4-e5f6-7890-abcd-ef1234567890"`

### 2.2 Usage

The same Installation ID is used in two places:
- **`X-UDID` header**: Set by `HeaderUtil.setDefaultHeaders()` on every main-app request
- **`deviceId` field in X-LI-Track**: Set by `XLiTrackHeader.initJson()`

Both call `Installation.id(context)`, so `X-UDID == X-LI-Track.deviceId`. The server can correlate these.

### 2.3 Rust Client Recommendation

Generate a UUID v4 once and persist it. Use the same value for both `X-UDID` header and `deviceId` in X-LI-Track. Do NOT regenerate per-session -- the real app persists this across restarts.

---

## 3. JSESSIONID / CSRF Token

Source: `com.linkedin.android.networking.cookies.CsrfCookieHelper`

### 3.1 Generation

```
"ajax:" + format("%019d", abs(SecureRandom.nextLong()))
```

- Uses `SecureRandom` (cryptographic PRNG)
- Format: `ajax:` prefix + 19-digit zero-padded positive long
- Example: `"ajax:0123456789012345678"`
- Maximum value: `ajax:9223372036854775807` (Long.MAX_VALUE)
- Special case: if `nextLong()` returns `Long.MIN_VALUE`, uses `Long.MAX_VALUE` instead

### 3.2 Usage Flow

1. Client creates JSESSIONID cookie with generated value
2. Cookie is set on the linkedin.com domain
3. On every request, `Csrf-Token` header is set to the JSESSIONID cookie value
4. If no JSESSIONID exists, `GET /uas/authenticate` is called to obtain one from the server

### 3.3 Rust Client Recommendation

Generate a JSESSIONID value before the first request. Set it as a cookie (`JSESSIONID=ajax:{19-digit-number}`) and echo it as the `Csrf-Token` header on every request.

---

## 4. User-Agent Headers

There are **two** User-Agent-like headers:

### 4.1 Standard User-Agent

Source: `NetworkUtils.getUserAgent()`

```
ANDROID OS
```

This is the literal string `"ANDROID OS"` -- not a typical browser user-agent. Used in the auth library's default headers.

Note: The main app likely relies on Cronet's built-in User-Agent (which mimics Chrome), since the main networking stack uses Cronet (Chromium v83) as the HTTP transport. The custom `User-Agent` value above is only for the auth library's direct HTTP calls.

### 4.2 X-LI-User-Agent (Auth Library Only)

Source: `NetworkUtils.getXliUserAgent()`

Format:
```
LIAuthLibrary:0.0.3 com.linkedin.android:4.2.1058 Google_Pixel 8:android_14
```

Structure: `LIAuthLibrary:{lib_version} {package}:{app_version} {manufacturer}_{model}:android_{os_release}`

This header is set by `NetworkUtils.getDefaultHeaders()` during auth flows. The main app does NOT appear to set `X-LI-User-Agent`.

---

## 5. Complete Header Set for API Requests

### 5.1 Main App (Authenticated Requests)

Set by `BaseLinkedInNetwork.getRequestHeaders()` -> `HeaderUtil.setDefaultHeaders()` + `NetworkClientConfigurator.configure()`:

| Header | Value | Set By |
|--------|-------|--------|
| `X-RestLi-Protocol-Version` | `2.0.0` | `NetworkClientConfigurator.configure()` |
| `X-UDID` | `{Installation UUID}` | `HeaderUtil.setDefaultHeaders()` |
| `X-LI-Track` | `{JSON blob}` | `HeaderUtil.setDefaultHeaders()` via `XLiTrackHeader` |
| `X-LI-Lang` | `{locale}` (e.g. `en_US`) | `HeaderUtil.setLangHeader()` |
| `Accept-Language` | `{locale with dash}` (e.g. `en-US`) | `HeaderUtil.setLangHeader()` |
| `Csrf-Token` | `{JSESSIONID value}` | Cookie manager |

On retries, additionally:

| Header | Value |
|--------|-------|
| `X-LI-Retry-Context` | `{"attempt": N, "errorCode": E}` |

### 5.2 Auth Library (Login/Registration)

Set by `LoginHelper.getRequestHeaders()` / `RegistrationHelper.addRequestHeaders()`:

| Header | Value |
|--------|-------|
| `content-type` | `application/json` |
| `Csrf-Token` | `{JSESSIONID value}` |
| `X-LI-Track` | `{auth-library JSON variant}` |

Additional headers from `NetworkUtils.getDefaultHeaders()` (used for initial CSRF fetch):

| Header | Value |
|--------|-------|
| `X-LI-User-Agent` | `LIAuthLibrary:0.0.3 {package}:{version} {model}:android_{release}` |
| `User-Agent` | `ANDROID OS` |
| `Accept-Language` | `{locale with dash}` (e.g. `en-US`) |

### 5.3 Tracking Requests

Set by `DataUtils` for analytics/tracking endpoints:

| Header | Value |
|--------|-------|
| `Content-Type` | `application/x-protobuf2` or `application/json` |
| `X-RestLi-Protocol-Version` | `2.0.0` |

---

## 6. Other Fingerprinting Mechanisms

### 6.1 TLS Fingerprinting via Cronet

The app uses Chromium's Cronet library (v83.0.4103.83) as its HTTP transport. This means:
- TLS Client Hello will match Chrome 83's fingerprint (JA3/JA4)
- This is NOT standard Android TLS -- a standard Rust HTTP client will have a different TLS fingerprint
- **Mitigation for Rust client**: Use a library that can mimic Chrome TLS fingerprints (e.g., `reqwest` with `boring-ssl` and custom cipher configuration, or use `curl-impersonate`)

### 6.2 Cronet HTTP/2 Fingerprinting

Beyond TLS, HTTP/2 settings frames (SETTINGS_HEADER_TABLE_SIZE, SETTINGS_MAX_CONCURRENT_STREAMS, etc.) can fingerprint the client. Cronet's HTTP/2 settings match Chrome.

### 6.3 AppConfig Fields (Store/Advertiser)

The `AppConfig` object provides:
- `storeId`: Likely the Play Store package identifier
- `advertiserId`: Google Advertising ID (GAID) -- a per-device advertising tracker
- `isAdTrackingLimited`: Whether the user has opted out of ad tracking
- `mpName`: Internal metric/app name (e.g. `"voyager-android"`)
- `mpVersion`: App version for internal metrics

For the Rust client, these can be plausible static values. Setting `isAdTrackingLimited: true` with a zeroed GAID is a safe choice (matches privacy-conscious users).

### 6.4 No Certificate Pinning Detected (in these classes)

The networking classes examined do not implement certificate pinning. However, Cronet may have its own certificate verification. This needs further investigation in the Cronet configuration classes.

---

## 7. Rust Client Implementation Recommendations

### 7.1 Recommended Static Values

For a Rust client impersonating the Android app, use these plausible values:

```rust
// Device identity -- generate once, persist
let device_id = Uuid::new_v4().to_string();  // e.g. "a1b2c3d4-..."

// X-LI-Track for authenticated requests (main app format)
let x_li_track = json!({
    "osName": "Android OS",
    "osVersion": "34",                       // Android 14 API level
    "clientVersion": "4.2.1058",             // Recent app version
    "clientMinorVersion": 562100,            // Corresponding version code
    "carrier": "T-Mobile",                   // Or null for Wi-Fi only
    "model": "Google_Pixel 8",               // Popular device
    "displayDensity": 2.625,
    "displayWidth": 1080,
    "displayHeight": 2400,
    "dpi": "xxhdpi",
    "deviceType": "android",
    "appId": "com.linkedin.android",
    "deviceId": device_id,
    "timezoneOffset": -8.0,                  // Adjust to actual timezone
    "timezone": "America/Los_Angeles",       // Adjust to actual timezone
    "storeId": "com.linkedin.android",
    "advertiserId": "00000000-0000-0000-0000-000000000000",
    "isAdTrackingLimited": true,
    "mpName": "voyager-android",
    "mpVersion": "4.2.1058"
}).to_string();

// X-LI-Track for auth requests (auth library format)
let x_li_track_auth = json!({
    "appId": "com.linkedin.android",
    "carrier": "Google",                     // Build.BRAND, not carrier
    "clientVersion": "4.2.1058",
    "clientMinorVersion": "562100",          // Note: string, not int
    "deviceType": "android",
    "dpi": "xxhdpi",
    "language": "en_US",
    "model": "Google_Pixel 8",
    "osName": "ANDROID OS/14",
    "osVersion": "14",                       // Build.VERSION.RELEASE
    "timezone": "-8"                         // Offset as string
}).to_string();

// JSESSIONID
let jsessionid = format!("ajax:{:019}", rand::thread_rng().gen_range(0..=i64::MAX));
```

### 7.2 Header Application Order

1. **Before first request**: Generate JSESSIONID, device_id
2. **CSRF fetch** (`GET /uas/authenticate`): Include `User-Agent: ANDROID OS`, `X-LI-User-Agent`, `Accept-Language`
3. **Login** (`POST /uas/authenticate`): Include `Csrf-Token`, `X-LI-Track` (auth variant), `content-type: application/json`
4. **All subsequent API calls**: Include `X-RestLi-Protocol-Version: 2.0.0`, `X-UDID`, `X-LI-Track` (main variant), `X-LI-Lang`, `Accept-Language`, `Csrf-Token`

### 7.3 Consistency Requirements

- `X-UDID` header value MUST equal `deviceId` in X-LI-Track JSON
- `Csrf-Token` header MUST equal `JSESSIONID` cookie value
- `clientVersion` should be consistent across X-LI-Track and X-LI-User-Agent
- `model` format must be `Manufacturer_Model` (with underscore separator)
- `timezoneOffset` updates on timezone change in real app; for Rust client, set once to actual timezone

### 7.4 Risk Assessment

| Mechanism | Risk Level | Mitigation |
|-----------|-----------|------------|
| X-LI-Track validation | Medium | Use realistic values from a popular device |
| X-UDID correlation | Low | Persist UUID; don't regenerate |
| TLS fingerprinting | High | Use Cronet/Chrome-mimicking TLS library |
| HTTP/2 fingerprinting | Medium | Match Chrome HTTP/2 settings |
| JSESSIONID format | Low | Follow exact format: `ajax:{019d}` |
| Rate limiting | High | Rate-limit requests; use realistic timing |

---

## 8. Source File Index

| File | Purpose |
|------|---------|
| `decompiled/jadx/sources/com/linkedin/android/networking/util/XLiTrackHeader.java` | Main X-LI-Track builder |
| `decompiled/jadx/sources/com/linkedin/android/networking/util/HeaderUtil.java` | Default header setter (X-UDID, X-LI-Track, lang) |
| `decompiled/jadx/sources/com/linkedin/android/networking/util/Installation.java` | Device ID (UUID) generation and persistence |
| `decompiled/jadx/sources/com/linkedin/android/networking/util/Util.java` | Timezone offset/ID helpers |
| `decompiled/jadx/sources/com/linkedin/android/networking/AppConfig.java` | Store/advertiser/mp config |
| `decompiled/jadx/sources/com/linkedin/android/networking/BaseLinkedInNetwork.java` | Request header assembly |
| `decompiled/jadx/sources/com/linkedin/android/networking/cookies/CsrfCookieHelper.java` | JSESSIONID generation |
| `decompiled/jadx/sources/com/linkedin/android/infra/network/NetworkClientConfigurator.java` | X-RestLi-Protocol-Version header |
| `decompiled/jadx/sources/com/linkedin/android/liauthlib/network/NetworkUtils.java` | Auth library X-LI-Track + User-Agent |
| `decompiled/jadx/sources/com/linkedin/android/liauthlib/login/LoginHelper.java` | Login request headers |
| `decompiled/jadx/sources/com/linkedin/android/liauthlib/registration/RegistrationHelper.java` | Registration request headers |
