# React Native JS Bundle Analysis

Analysis of `extracted/assets/index.android.bundle`. Date: 2026-03-24.

---

## 1. File Format

**NOT Hermes bytecode.** The bundle is plain minified JavaScript.

- `file` output: `React Native minified JavaScript, ASCII text, with very long lines (8273)`
- Magic bytes: `var __BUNDLE_START_TIME__=this.nativePerformance...` (standard RN bundle preamble)
- Size: 2.0 MB (2,075,754 bytes)
- Lines: 1,000 (minified, very long lines)
- Module count: 994 modules (Metro bundler `__d()` registrations)

This means no Hermes decompilation is needed. The JS is directly searchable and analyzable.

---

## 2. Bundle Structure

This is a standard Metro-bundled React Native application. The bundle uses Metro's module system (`__d(function(...), moduleId, [dependencies])`) with 994 registered modules.

### 2.1 Key Dependencies Identified

| Library | Purpose |
|---------|---------|
| React Navigation 5.x | Navigation framework (`@react-navigation`) |
| react-native-reanimated | Animation library |
| react-native-gesture-handler | Gesture handling |
| react-native-svg | SVG rendering |
| CodePush (Microsoft) | Over-the-air JS bundle updates |

### 2.2 Native Bridge Modules

The RN layer communicates with native Java via these `NativeModules`:

| Module | Purpose |
|--------|---------|
| `ZephyrReactInfraModule` | **Primary bridge** -- LinkedIn's custom RN-to-native infrastructure module |
| `CodePush` | OTA update management |
| `CodePushDialog` | OTA update UI dialogs |
| `I18nManager` | Internationalization (RTL support) |
| `PlatformConstants` | Platform info |
| `SettingsManager` | App settings |
| `UIManager` | React Native UI management |
| `ReanimatedModule` | Animation bridge |
| `RNGestureHandlerModule` | Gesture bridge |
| `RNSVGRenderableManager` | SVG rendering bridge |
| `RNSVGSvgViewManager` | SVG view bridge |

`ZephyrReactInfraModule` is the critical one -- it is LinkedIn's custom bridge for the RN layer to access native services (auth, networking, etc). The RN code references it but appears to use it primarily for infrastructure rather than making direct API calls through it.

---

## 3. API Endpoints from RN Layer

### 3.1 API_ENDPOINTS Constant

The bundle defines a central `API_ENDPOINTS` object with hardcoded full URLs:

```javascript
API_ENDPOINTS = {
  me:                          "https://www.linkedin.com/zephyr/api/zephyrMe",
  miniJobs:                    "https://www.linkedin.com/zephyr/api/zephyrMiniJobs",
  socialHiringReferrer:        "https://www.linkedin.com/zephyr/api/zephyrJobSocialHiringReferrerCards",
  socialHiringReferrerState:   "https://www.linkedin.com/zephyr/api/zephyrJobSocialHiringReferrers",
  postTextFeed:                "https://www.linkedin.com/voyager/api/contentcreation/normShares",
  feedUrlPreview:              "https://www.linkedin.com/zephyr/api/voyagerContentcreationUrlPreview",
  socialHiringReferrerAwardTasks: "https://www.linkedin.com/zephyr/api/zephyrJobSocialHiringReferrerAwardTasks"
}
```

### 3.2 All LinkedIn API URLs Found

| URL | API Prefix | Purpose |
|-----|-----------|---------|
| `/zephyr/api/zephyrMe` | Zephyr | Current user info (RN equivalent of native `me` route) |
| `/zephyr/api/zephyrMiniJobs` | Zephyr | Job recommendations (lightweight) |
| `/zephyr/api/zephyrJobSocialHiringReferrerCards` | Zephyr | Social hiring referral cards |
| `/zephyr/api/zephyrJobSocialHiringReferrers` | Zephyr | Social hiring referrer state |
| `/zephyr/api/zephyrJobSocialHiringReferrerAwardTasks` | Zephyr | Social hiring award/task tracking |
| `/zephyr/api/voyagerContentcreationUrlPreview` | Zephyr | URL preview for content creation |
| `/zephyr/api/zephyrCoachCampaignFilterTypeahead` | Zephyr | Company typeahead for coaching/campaigns |
| `/voyager/api/contentcreation/normShares` | Voyager | **Create posts** (note: uses Voyager prefix, not Zephyr) |

### 3.3 Web/Non-API URLs

| URL | Purpose |
|-----|---------|
| `/wukong-web/socialHiring/intro` | Social hiring intro page (WebView) |
| `/wukong-web/socialHiring/referrer` | Social hiring referrer page (WebView) |
| `/wukong-web/socialHiring/referrer/signUp` | Social hiring sign-up (WebView) |
| `/psettings/meet-the-team` | Meet the team settings page (WebView) |

### 3.4 CDN / Static Asset URLs

| URL | Purpose |
|-----|---------|
| `https://media-exp1.licdn.com/dms/image/...` | Company logo image (CDN) |
| `https://static-exp1.licdn.com/sc/h/...` | Static assets (icons, etc.) |

---

## 4. Fetch Call Patterns

The RN layer makes API calls using the standard `fetch()` API (NOT via `ZephyrReactInfraModule` bridge). All fetch calls include a `csrf-token` header obtained from the native layer.

### Pattern 1: GET with CSRF

```javascript
fetch(API_ENDPOINTS.socialHiringReferrer + "?q=viewer", {
  method: 'GET',
  headers: { 'csrf-token': t }
})
```

### Pattern 2: POST with JSON body

```javascript
fetch(API_ENDPOINTS.postTextFeed, {
  method: 'POST',
  headers: {
    'csrf-token': t,
    'Content-Type': 'application/json'
  },
  body: JSON.stringify(payload)
})
```

### Pattern 3: GET with query parameters

```javascript
fetch(API_ENDPOINTS.miniJobs + "?count=" + pageCount + "&q=findRecommendedJobs&start=" + offset, ...)

fetch("https://www.linkedin.com/zephyr/api/zephyrCoachCampaignFilterTypeahead?q=filter&type=COMPANY&query=" + query, {
  method: 'GET',
  headers: { 'csrf-token': t }
})
```

### Key Observations

1. **CSRF token is passed to every request** -- obtained from native layer, same `csrf-token` header as native code uses.
2. **No `X-RestLi-Protocol-Version` header** in RN fetch calls -- the RN layer does not set Rest.li protocol headers. This suggests the Zephyr API endpoints may be a simpler wrapper around the Rest.li backend.
3. **Hardcoded base URL**: `https://www.linkedin.com` is baked into the JS bundle, unlike the native layer which uses configurable SharedPreferences.
4. **Pagination uses `start` + `count`** with `q=` query parameter -- consistent with native layer pattern.

---

## 5. RN Feature Surfaces

Based on the API endpoints and bundle structure, the React Native layer implements these features for the **Zephyr (China/LinkedIn China)** variant:

| Feature | Evidence |
|---------|----------|
| **Social Hiring / Referrals** | Multiple endpoints for referrer cards, award tasks, state tracking; WebView pages for intro/signup |
| **Job Recommendations** | `zephyrMiniJobs` endpoint with pagination |
| **Content Creation (Post)** | Uses `/voyager/api/contentcreation/normShares` -- shares the Voyager endpoint |
| **URL Preview** | `voyagerContentcreationUrlPreview` for link previews in post composer |
| **Company Typeahead** | `zephyrCoachCampaignFilterTypeahead` for company search |
| **Current User** | `zephyrMe` for fetching current user info |

The RN layer is clearly focused on **Social Hiring** (referral programs) and **lightweight job browsing** -- features likely specific to the China market (Zephyr variant).

---

## 6. Other Config/Asset Files in `extracted/assets/`

### 6.1 Huawei Mobile Services (HMS) Configs

The presence of HMS configs confirms this APK targets the China market (where Google Play Services are unavailable):

| File | Purpose |
|------|---------|
| `grs_sdk_server_config.json` | GRS (Global Route Service) base URLs at `grs.dbankcloud.com/cn/eu/asia` |
| `grs_sdk_global_route_config_opensdkService.json` | Huawei HiAnalytics endpoints by region (metrics*.data.hicloud.com:6447) |
| `grs_sdk_global_route_config_opendevicesdk.json` | Huawei push notification endpoints (data-*.push.dbankcloud.com) |
| `grs_sdk_global_route_config_updatesdk.json` | Huawei app store update endpoints (store*.hispace.hicloud.com) |
| `grs_sp.bks` | Bouncy Castle keystore (HMS TLS) |
| `hmsincas.bks` | HMS intermediate CA keystore |
| `hmsrootcas.bks` | HMS root CA keystore |
| `updatesdkcas.bks` | Update SDK CA keystore |
| `hianalytics_njjn` | HiAnalytics config blob |

### 6.2 CUCC (China Unicom) Integration

`cucc/host_cucc.properties`:
```
PRODUCE_STATISTICAL=https://daily.m.zzx9.cn     # Analytics
PRODUCE_DZH=https://msv6.wosms.cn               # Phone number verification (SIM-based)
```

This is China Unicom carrier integration for SIM-based phone number verification.

### 6.3 Guest Experience

`guest_experience.json`: Pre-loaded company showcase data (Microsoft, Tencent, Alibaba, Apple, Mobike, Toutiao, Kuaishou, Baidu, Xiaomi) with Chinese-language content. Used for onboarding/guest browsing. Static images hosted on `o1am219ni.qnssl.com` (Qiniu CDN, China).

### 6.4 Other Assets

| File/Directory | Purpose |
|----------------|---------|
| `career/company_black_list.json` | Company blacklist (30KB) |
| `entities/checked_56dp.json` | Lottie animation (checkmark) |
| `feed/double_tap_to_like.json` | Lottie animation (like) |
| `growth/img_success_check_56dp.json` | Lottie animation (success) |
| `growth/supplierconfig.json` | Push notification supplier config (vivo, xiaomi, huawei, oppo) |
| `l2m/l2m_logout_education_bottle_animation.json` | Lottie animation (logout education) |
| `messaging/bing_maps_v8_web_control.html` | Bing Maps WebView (location sharing in messages) |
| `socialhiring/bole_one_to_many_entry_animation.json` | Lottie animation (social hiring) |
| `templates/first_party_article_template.html` | HTML template for article rendering |
| `video/media_overlay_in_market_nux.json` | Lottie animation (video overlay) |
| `zephyr_add_phone_number.js` | Injected JS for phone number settings WebView |
| `zephyr_phone_number_learn_more.html` | Phone number help page (English) |
| `zephyr_phone_number_learn_more_cn.html` | Phone number help page (Chinese) |
| `zephyr_pinyin.txt` | Pinyin lookup table (125KB, for Chinese name search) |
| `zlsioh.dat` | Unknown binary blob |
| `scholarship_*.png` | Scholarship program images (WeChat sharing) |

---

## 7. Key Findings

### 7.1 This is the Zephyr (China) Variant

This APK is definitively the **LinkedIn China (Zephyr)** variant, not the international Voyager version:
- API prefix is `/zephyr/api/` throughout
- Huawei Mobile Services integration (no Google Play Services)
- China Unicom carrier integration
- Chinese-language guest content
- Pinyin search support
- WeChat sharing assets
- Push notification integration with Chinese OEMs (Xiaomi, Huawei, Oppo, Vivo)

### 7.2 RN-Only Endpoints (Not Found in Native Routes)

These endpoints appear to be **RN-layer-only** (not present in the native `Routes.java` catalog from `architecture_overview.md`):

| Endpoint | Notes |
|----------|-------|
| `/zephyr/api/zephyrMe` | Zephyr-specific version of `/me` |
| `/zephyr/api/zephyrMiniJobs` | Lightweight job recommendations |
| `/zephyr/api/zephyrJobSocialHiringReferrerCards` | Social hiring cards |
| `/zephyr/api/zephyrJobSocialHiringReferrers` | Social hiring referrer info |
| `/zephyr/api/zephyrJobSocialHiringReferrerAwardTasks` | Social hiring awards |
| `/zephyr/api/voyagerContentcreationUrlPreview` | URL preview (Zephyr-wrapped) |
| `/zephyr/api/zephyrCoachCampaignFilterTypeahead` | Company typeahead for coaching |

The only endpoint shared with the native Voyager layer is `/voyager/api/contentcreation/normShares` (post creation).

### 7.3 CodePush OTA Updates

The bundle includes Microsoft CodePush, meaning the RN layer can be updated over-the-air without an APK update. This is typical for RN apps but means the JS bundle in the APK may not reflect the latest deployed version.

### 7.4 Missing Rest.li Headers

The RN fetch calls do NOT include `X-RestLi-Protocol-Version: 2.0.0`. This suggests the Zephyr API endpoints are either:
- A REST wrapper around the Rest.li backend (translating internally), or
- A separate API surface that doesn't use Rest.li protocol conventions

This is a notable difference from the native networking layer.

---

## 8. Limitations

1. **Minified code**: Variable names are mangled. Function logic is readable but requires effort to trace.
2. **CodePush**: The APK-bundled JS may be outdated -- the app likely downloads newer bundles at runtime.
3. **Zephyr-specific**: Findings are specific to the China variant. The international Voyager app may have a different RN bundle or none at all.
4. **No request/response models**: The minified JS does not contain Pegasus model definitions. Data models are handled by the native layer.
