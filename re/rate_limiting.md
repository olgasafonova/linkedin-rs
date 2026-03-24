# Rate Limiting, Retry, and Timeout Behavior

Reverse-engineered from `com.linkedin.android` APK, decompiled via jadx. Analysis date: 2026-03-24.

---

## 1. HTTP Status Code Handlers

LinkedIn's custom networking stack uses a `StatusCodeRegistry` (`com.linkedin.android.networking.util.StatusCodeRegistry`) -- a `ConcurrentHashMap<Integer, StatusCodeHandler>` that dispatches registered handlers after every response.

### 1.1 Registered Handlers

Only **two** status code handlers are registered via `NetworkClientConfigurator.configure()`:

| Status Code | Handler Class | Behavior |
|-------------|--------------|----------|
| **401** | `UnauthorizedStatusCodeHandler` | See section 1.2 |
| **403** | `ForbiddenStatusCodeHandler` | See section 1.3 |

There is **no registered handler for 429** (Too Many Requests) or any other status code. The registry is open for extension but only 401 and 403 are wired up at app initialization.

### 1.2 UnauthorizedStatusCodeHandler (401)

Location: `com.linkedin.android.app.UnauthorizedStatusCodeHandler`

Behavior:
1. Guard: only fires once per session (uses `HANDLE_UNAUTHORIZED_STATUS_CODE` static flag, set to `false` after first invocation).
2. Guard: only fires if the user is currently authenticated and the request URL starts with the configured base URL.
3. Checks whether the `li_at` cookie is present for the request URL.
4. **If `li_at` is missing**: logs user out immediately.
5. **If `li_at` is present but got 401**: behavior depends on feature flag `Lix.UNAUTHORIZED_LOGOUT_ONLY_WHEN_NO_LIAT`:
   - If flag enabled: reports non-fatal crash only, does NOT log out.
   - If flag disabled: logs user out.
6. Logout in background: calls `Auth.signOut()`.
7. Logout in foreground: navigates to `LoginActivity`.

Key detail: the one-shot flag (`HANDLE_UNAUTHORIZED_STATUS_CODE`) means only the first 401 per app session triggers this logic. Subsequent 401s are silently ignored by the handler (though they still propagate as error responses to the caller).

### 1.3 ForbiddenStatusCodeHandler (403)

Location: `com.linkedin.android.app.ForbiddenStatusCodeHandler`

Behavior:
1. Guard: only fires if the user is authenticated and the request URL starts with the base URL.
2. Reports a non-fatal crash with the URL, `X-LI-UUID` tree ID, and cookie state.
3. Does **not** log the user out.
4. Does **not** retry the request.

This is purely diagnostic -- it logs 403s to the crash reporting system for server-side investigation.

---

## 2. Retry Strategies

### 2.1 Cronet Transport-Level Retry (Main API)

The primary retry mechanism lives in the Cronet network engine: `CronetNetworkEngine.performBlockingRequest()`.

**Retry eligibility** (`CronetNetworkEngineWithoutExecution.canRetry()`):
- `automaticRetriesEnabled` must be `true` on the request (controlled per-request via `AbstractRequest.setShouldEnableAutomaticRetries()`, defaults to `false`).
- HTTP method must be **GET** (0) or **HEAD** (4) -- idempotent methods only.
- Current `retryCount < maxRetryCount` (a static volatile int, settable at runtime; default is 0 meaning no retries unless explicitly configured).

**Retry trigger** (`shouldRetryRequest()`):
- Only retries on `NetworkException` with error code `3` (which maps to Cronet's `ERROR_CONNECTION_REFUSED` or similar network-level failure).
- The retry is **immediate** (no backoff delay).
- The response must not have started yet (`!responseStarted`) -- once response headers arrive, no retry.

**Retry flow**:
1. On failure in `Callback.onFailed()`, if eligible, sets `shouldImmediatelyRetry = true`.
2. `performBlockingRequest()` sees the flag, increments `retryCount`, adds `X-LI-Retry-Context` header, and recurses.
3. `RetryEventListener` callbacks are invoked for monitoring (`onRetryAttempt`, `onRetrySucceeded`, `onRetryFailed`).

**What this means**: The main API has **no application-level retry for HTTP errors** (4xx, 5xx). Retries only happen for transport-level connection failures on GET/HEAD requests, and only if the request explicitly opts in.

### 2.2 X-LI-Retry-Context Header

Set by `HeaderUtil.setRetryAttemptHeader()` on retry attempts:

```json
{
  "attempt": 1,
  "errorCode": -1
}
```

- `attempt`: 1-indexed retry count.
- `errorCode`: HTTP status code from the failed attempt, or -1 if no response was received.

This header tells the server that the request is a retry, enabling server-side analytics and potentially different handling.

### 2.3 Retry-After Header Handling

`HeaderUtil.getDelayFromRetryAfterHeader()` parses the standard `Retry-After` header in two formats:

1. **Numeric (seconds)**: parses as `Double`, rounds to nearest second. Value must be > 0.
2. **HTTP-date**: parses `"EEE, dd MMM yyyy HH:mm:ss zzz"` format (e.g., `"Thu, 01 Dec 2025 16:00:00 GMT"`), computes delta from current time.

**Where it is used**: Only in `CronetNetworkEngineWithoutExecution.followRedirect()`. When following a redirect (3xx), if the redirect response includes a `Retry-After` header, the client **sleeps for that duration** (in seconds * 1000 ms) before following the redirect.

The `Retry-After` header is **not** checked or honored for 429 or 503 responses in LinkedIn's custom stack. It is only used during redirect processing.

### 2.4 OkHttp RetryAndFollowUpInterceptor (React Native Only)

OkHttp is bundled but only used by React Native / Fresco image loading, NOT by the main LinkedIn API. For completeness:

- **408 (Request Timeout)**: retries once if `retryOnConnectionFailure` is enabled, no `Retry-After` delay, and the request body is replayable. Won't retry if prior response was also 408.
- **503 (Service Unavailable)**: retries once if `Retry-After` header is `0` (or absent) and prior response was not 503.
- **401**: delegates to configured `Authenticator`.
- **407**: delegates to proxy authenticator.
- **421 (Misdirected Request)**: retries on a different connection if the request used connection coalescing.
- **Max follow-ups**: 20 (covers both redirects and retries).

This interceptor does NOT handle 429.

### 2.5 Real-Time System Retry

The real-time long-polling system (`RealTimeOnlineManager`) uses a pluggable `ConnectionRetryStrategy`:

**BackoffComputer** (exponential, used for real-time connections):
```
delay = 2^retryCount * 100 ms
```
- retryCount 0: 100ms
- retryCount 1: 200ms
- retryCount 2: 400ms
- retryCount 3: 800ms
- ... (exponential growth)

**LinearBackoffStrategy** (used for messaging real-time):
```
delay = (random(0..2000) + 5000) * failureCount ms
```
- failureCount 1: ~5000-7000ms
- failureCount 2: ~10000-14000ms
- Stops retrying after failureCount > 2 (max 2 retries).

Both reset their failure count on successful connection.

### 2.6 Media Upload Retry

`VectorService` / `VectorMediaUploader` supports retry for file uploads via `UploadRetryEvent` events. The retry count is passed as an intent extra (`fileUploadRetryCount`). The exact retry policy is managed by the upload manager and not directly visible in the decompiled code, but the infrastructure supports it.

### 2.7 Payments Retry

`PaymentsClientV2.RetryHttpPostOperationListenerV2` implements aggressive retry for payment-related POST operations:
- **Max retries**: 60
- **Strategy**: retry on any failure by re-posting with incremented `retryCount`.
- No backoff delay visible in the decompiled code.

### 2.8 Tracking/Analytics Retry

`RequestData` in `com.linkedin.android.litrackingcomponents.network` carries `retryAttemptNumber` and `lastStatusErrorCode`. When `retryAttemptNumber > 0`, the `X-LI-Retry-Context` header is set. Tracking events use WorkManager (`TrackingEventWorker`, `SendTrackingEventWorker`) which provides built-in retry via Android's work scheduling.

---

## 3. Rate Limiting

### 3.1 Server-Side Rate Limiting (429 Handling)

There is **no centralized 429 handler** in the client. The app handles 429 on a case-by-case basis in specific UI components:

1. **Company Contact (`ContactCompanyDialogFragment`)**: checks `rawResponse.code() == 429` and shows "already contacted" error message. This suggests LinkedIn uses 429 to mean "action already performed / rate limited" for this specific endpoint.

2. **Premium Insights (`CompanyPremiumInsightsCardsTransformer`)**: checks for error message `"Received error status code for request:429"` and displays a specific error card. Handled alongside 404 as a data-unavailable condition.

3. **PEM Availability Tracking (`PemAvailabilityTrackingMetadata`)**: 429 is explicitly **excluded** from client error degradation tracking. The logic treats 4xx (400-499) as non-degraded (client errors) EXCEPT 429, which IS treated as degraded. This confirms LinkedIn considers 429 a server-side capacity issue, not a client error.

### 3.2 No Client-Side Request Throttling

There is **no evidence** of client-side request throttling, rate limiting, or request queuing in the decompiled code:
- No token bucket, sliding window, or leaky bucket implementations found.
- No `throttle`, `requestLimit`, `concurrentRequest`, or `maxRequest` patterns in the networking package.
- No request queue with concurrency limits beyond standard `ExecutorService` thread pool sizing.

The client sends requests as fast as the caller asks. Rate limiting is entirely server-side.

### 3.3 No Rate-Limit Response Headers

No evidence of the client parsing rate-limit-specific response headers such as:
- `X-RateLimit-Limit`
- `X-RateLimit-Remaining`
- `X-RateLimit-Reset`
- `RateLimit-*` (IETF draft)

The only rate-related header parsed is `Retry-After`, and only during redirect processing (see section 2.3).

---

## 4. Circuit Breaker Patterns

### 4.1 No Circuit Breaker Implementation

There is **no circuit breaker pattern** in the networking stack. Searched for `circuitBreak`, `CircuitBreaker`, and related patterns -- no results in LinkedIn's own code.

The only reference to "circuit breaker" is in `SymbolTableHolder` (tracking symbol table) which contains the string as a tracking event name, suggesting circuit breakers may exist server-side but the client has no awareness of them.

### 4.2 PEM (Performance and Error Monitoring) as a Proxy

`PemAvailabilityTrackingMetadata` tracks degraded response codes per feature. While not a circuit breaker, this system could influence client behavior via feature flags (LIX) -- the server could disable features experiencing degradation. This is a server-driven degradation mechanism, not a client-side circuit breaker.

---

## 5. Timeout Configuration

### 5.1 Default Timeouts

From `AbstractRequest`:

| Timeout | Default | Source |
|---------|---------|--------|
| **Connect timeout** | 10 seconds | `DEFAULT_CONNECT_TIMEOUT_MILLIS = TimeUnit.SECONDS.toMillis(10)` |
| **Read (socket) timeout** | 10 seconds | `DEFAULT_READ_TIMEOUT_MILLIS = TimeUnit.SECONDS.toMillis(10)` |
| **Write timeout** | 10 seconds | `DEFAULT_WRITE_TIMEOUT_MILLIS = TimeUnit.SECONDS.toMillis(10)` |

All three default to 10 seconds. Each can be overridden per-request.

### 5.2 Timeout Exception Handling

`BaseLinkedInNetwork.getStatusCode()` maps exceptions to pseudo status codes:
- `SocketException` -> 998
- `SocketTimeoutException` -> 408
- Other exceptions -> 999

When a 408 (timeout) occurs, `LinkedInNetwork.handleFailureResponse()` reports to the perf listener:
- `connectionDidTimeout(url, connectTimeoutMillis)`
- `requestTimedOut(url)`

### 5.3 Long-Poll Timeouts

Long-poll connections (`LongPollStreamNetworkClient.connect()`) accept a custom socket timeout parameter, and set write timeout to 0 (infinite). The actual timeout value is passed by the real-time system at connection time.

### 5.4 Cronet Engine Init

Cronet engine initialization has no explicit timeout but runs on a single-thread executor. If init is slow, requests block on `initFuture.get()` (which will block indefinitely).

### 5.5 Browser ID Cookie Seeding

`seedBrowserIdCookie()` uses `HttpURLConnection` (not Cronet) with hardcoded timeouts:
- Connect timeout: `Constants.ASSEMBLE_PUSH_RETRY_INTERVAL` (3000ms based on Xiaomi SDK constant)
- Read timeout: Same value (3000ms)

---

## 6. Summary: Implications for API Replication

### What LinkedIn Does
1. **Transport-level retry** for GET/HEAD on connection failures (immediate, no backoff, opt-in per request).
2. **Retry-After header** honored only on redirects.
3. **X-LI-Retry-Context header** sent on retries for server-side analytics.
4. **401 handling**: one-shot logout mechanism with feature-flag-controlled grace period.
5. **403 handling**: diagnostic logging only.
6. **429 handling**: ad-hoc in specific UI components, no centralized strategy.

### What LinkedIn Does NOT Do
1. No retry on 429 or 503 at the networking layer.
2. No client-side rate limiting or request throttling.
3. No backoff for main API requests (only for real-time connections).
4. No circuit breaker pattern.
5. No parsing of rate-limit response headers.

### What Our Rust Client Should Do
1. **Implement Retry-After handling for 429 and 503** -- the server will send these; the Android app just doesn't handle them well.
2. **Add exponential backoff with jitter** for retries (the Android app's immediate retry is fragile).
3. **Consider client-side rate limiting** to avoid triggering server-side restrictions -- LinkedIn clearly enforces limits server-side (429 responses exist) but the app relies on being a "known good" client.
4. **Send X-LI-Retry-Context header** on retries to match app behavior.
5. **Use 10-second default timeouts** to match the app.
6. **Handle 401 by re-authenticating** rather than logging out.
7. **Monitor for 429 responses** as a signal to back off -- the PEM system confirms LinkedIn treats these as server capacity issues.

---

## 7. Key Source Files

| File | Relevance |
|------|-----------|
| `com/linkedin/android/networking/interfaces/StatusCodeHandler.java` | Handler interface |
| `com/linkedin/android/networking/util/StatusCodeRegistry.java` | Handler dispatch registry |
| `com/linkedin/android/app/UnauthorizedStatusCodeHandler.java` | 401 handling |
| `com/linkedin/android/app/ForbiddenStatusCodeHandler.java` | 403 handling |
| `com/linkedin/android/infra/network/NetworkClientConfigurator.java` | Wires up 401/403 handlers |
| `com/linkedin/android/networking/engines/cronet/CronetNetworkEngine.java` | Transport retry loop |
| `com/linkedin/android/networking/engines/cronet/CronetNetworkEngineWithoutExecution.java` | Retry eligibility logic, Retry-After on redirect |
| `com/linkedin/android/networking/util/HeaderUtil.java` | X-LI-Retry-Context, Retry-After parsing |
| `com/linkedin/android/networking/request/AbstractRequest.java` | Timeout defaults, retry flag |
| `com/linkedin/android/realtime/internal/BackoffComputer.java` | Exponential backoff for real-time |
| `com/linkedin/android/messaging/realtime/backoff/LinearBackoffStrategy.java` | Linear backoff for messaging real-time |
| `com/linkedin/android/realtime/api/ConnectionRetryStrategy.java` | Abstract retry strategy |
| `com/linkedin/android/networking/BaseLinkedInNetwork.java` | Timeout exception mapping |
| `com/linkedin/android/health/pem/PemAvailabilityTrackingMetadata.java` | 429 treated as degraded |
| `okhttp3/internal/http/RetryAndFollowUpInterceptor.java` | OkHttp retry (React Native only) |
