# LinkedIn Android Real-Time Messaging and Notification System

Reverse-engineered from `com.linkedin.android.realtime` package (jadx decompilation). Analysis date: 2026-03-24.

---

## 1. Transport Protocol: Server-Sent Events (SSE) over HTTP Long Poll

The real-time system uses **Server-Sent Events (SSE)** delivered via a long-polling HTTP connection. This is NOT WebSocket, MQTT, or any other pub/sub protocol.

Evidence from `HttpUtils.getConnectHeaders()`:
```java
arrayMap.put("Accept", "text/event-stream");
```

The connection is managed by `LongPollStreamNetworkClient`, which:
- Opens a single HTTP GET request to the SSE endpoint
- Holds the connection open (socket timeout configurable, default 60,000ms)
- Reads the response body as a continuous stream via `processStream(InputStream)`
- Runs on a dedicated single-thread executor: `LongPoll-RequestExecutor`
- Uses the same Cronet/LinkedInNetwork stack as regular API calls (same TLS fingerprint, cookies, headers)

There is no WebSocket or MQTT usage anywhere in the codebase.

---

## 2. Connection Endpoint

### 2.1 SSE Connection Endpoint

```
GET {baseUrl}/realtime/connect
Accept: text/event-stream
```

The path `/realtime/connect` is passed as a relative URL (resolved against the base URL, which is `https://www.linkedin.com`).

Full URL: `https://www.linkedin.com/realtime/connect`

Note: This is NOT under the `/voyager/api/` prefix. The realtime endpoint sits at a different path.

### 2.2 Supporting Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/realtime/connect` | GET | SSE stream connection (long poll) |
| `/realtime/realtimeFrontendSubscriptions?ids=List(...)` | PUT (BATCH_UPDATE) / DELETE (BATCH_DELETE) | Subscribe/unsubscribe to topics |
| `/realtime/realtimeFrontendTimestamp` | GET | Server time synchronization |
| `/realtime/realtimeFrontendClientConnectivityTracking?action=sendHeartbeat` | POST | Client heartbeat for connectivity tracking |

---

## 3. Authentication

The real-time channel uses **the same cookie-based authentication** as the rest of the LinkedIn API. No special auth tokens are needed beyond the standard session.

Authentication flows through `LongPollStreamNetworkClient` -> `LinkedInNetwork`, which attaches:
- `li_at` session cookie (via `LinkedInHttpCookieManager`)
- `JSESSIONID` cookie and corresponding `Csrf-Token` header
- All standard headers (`X-RestLi-Protocol-Version`, `X-LI-Track`, `X-UDID`, etc.)

### Additional Real-Time Specific Headers

Set by `HttpUtils.getConnectHeaders()`:

| Header | Value | Notes |
|--------|-------|-------|
| `Accept` | `text/event-stream` | Declares SSE protocol |
| `x-li-recipe-map` | JSON object | Recipe/decoration configuration (optional) |
| `x-li-recipe-accept` | String | Recipe accept type (optional) |
| `x-li-accept` | String | Additional accept type (optional) |
| `x-li-realtime-session` | UUID string | Session ID from `RealTimeConnectivityTracker` (optional, for heartbeat tracking) |

### Subscription Request Headers

Set by `HttpUtils.getSubscribeHeaders()`:

| Header | Value | Notes |
|--------|-------|-------|
| `X-RestLi-Method` | `BATCH_UPDATE` (subscribe) or `BATCH_DELETE` (unsubscribe) | Rest.li batch method override |

---

## 4. Message Format

### 4.1 SSE Stream Format

The SSE stream follows the standard [Server-Sent Events specification](https://html.spec.whatwg.org/multipage/server-sent-events.html).

The `EventProcessor.Parser` class implements the SSE parser, handling these fields:
- `data:` -- Event payload (accumulated across multiple `data:` lines, joined with `\n`)
- `id:` -- Last event ID (stored for tracking, not used for reconnection in this implementation)
- `event:` -- Event type (read but not acted upon -- all events are dispatched the same way)
- `retry:` -- Reconnection time in milliseconds (passed to `EventHandler.setReconnectTimeMillis()`)
- Lines starting with `:` -- Comments (ignored)
- Empty line -- Dispatches the accumulated event

### 4.2 Event Payload Format (JSON)

The `data:` field contains a JSON object parsed as a `RealtimeEvent` union type. The union has exactly one of three members:

```
RealtimeEvent (union) = one of:
  1. "com.linkedin.realtimefrontend.ClientConnection"  -- Connection established
  2. "com.linkedin.realtimefrontend.Heartbeat"          -- Server heartbeat
  3. "com.linkedin.realtimefrontend.DecoratedEvent"     -- Actual event data
```

#### ClientConnection (connection established)

Sent as the first event after connecting. Contains:

```json
{
  "com.linkedin.realtimefrontend.ClientConnection": {
    "id": "<connection-id-string>",
    "serverProcessingTime": 123,
    "leftServerAt": 1711234567890,
    "personalTopics": ["messagesTopic", "conversationsTopic", "typingIndicatorsTopic", ...]
  }
}
```

Fields:
- `id` -- Unique connection ID, used in all subsequent subscription requests
- `serverProcessingTime` -- Server processing latency (ms)
- `leftServerAt` -- Server timestamp when the event was sent
- `personalTopics` -- List of topic names the user is pre-subscribed to (server-side)

#### Heartbeat

Periodic keepalive from the server:

```json
{
  "com.linkedin.realtimefrontend.Heartbeat": {}
}
```

Empty record. Simply confirms the connection is alive.

#### DecoratedEvent (actual event data)

```json
{
  "com.linkedin.realtimefrontend.DecoratedEvent": {
    "id": "<event-id>",
    "topic": "urn:li-realtime:<topic-name>:urn:li:<entity-type>:<entity-id>",
    "payload": { ... },
    "leftServerAt": 1711234567890,
    "publisherTrackingId": "<optional-tracking-id>",
    "trackingId": "<optional-bytes-as-string>"
  }
}
```

Fields:
- `id` (required) -- Unique event identifier
- `topic` (required) -- URN identifying which subscription topic this event belongs to
- `payload` (required) -- The actual event data, format depends on the topic (see section 5)
- `leftServerAt` (required) -- Server timestamp when the event left the server
- `publisherTrackingId` (optional) -- Publisher tracking ID for analytics
- `trackingId` (optional) -- Tracking ID bytes for analytics

The `payload` is parsed dynamically: the `DecoratedEventBuilder` looks up the topic URN in the `SubscriptionManager`, finds the registered `DataTemplateBuilder` for that topic, and uses it to deserialize the raw JSON bytes into the appropriate Pegasus model class.

---

## 5. Event Types and Topics

Topics are URN-based. The URN format is:

```
urn:li-realtime:<topic-name>:urn:li:member:<member-id>
```

For "personal" topics (belonging to the current user), the embedded URN is `urn:li-realtime:myself` which resolves to the logged-in member:

```
urn:li-realtime:<topic-name>:urn:li-realtime:myself
```

### 5.1 Known Personal Topics

These are subscribed per-user and receive events about the current user's data:

| Topic Name | Pegasus Model | Description |
|------------|--------------|-------------|
| `messagesTopic` | `RealtimeEvent` (`voyager.messaging.realtime`) | New message events. Contains the full `Event` object (message content, sender, conversation URN, etc.) plus optional `participantReceipts`. |
| `conversationsTopic` | `RealtimeConversation` (`voyager.messaging.realtime`) | Conversation metadata updates (title change, participants added/removed, etc.) |
| `typingIndicatorsTopic` | `RealtimeTypingIndicator` (`voyager.messaging.realtime`) | Typing indicators from other participants in conversations |
| `messageSeenReceiptsTopic` | `RealtimeSeenReceipt` (`voyager.messaging.realtime`) | Read receipts / seen status for messages |
| `replySuggestionTopicV2` | `RealtimeQuickReplyRecommendation` (`voyager.messaging.realtime`) | Smart reply suggestions (English locale only) |
| `tabBadgeUpdateTopic` | `RealtimeTabBadgesEvent` (`voyager.growth.communications`) | Tab badge count updates (notifications, messaging, my network counts) |

### 5.2 Non-Personal Topics

| Topic Name | Pegasus Model | Description |
|------------|--------------|-------------|
| `presenceStatusTopic` | `PresenceStatus` (`realtimefrontend.presence`) | Online/offline presence status of specific members. Topic URN includes the target member's URN (not `myself`). |

### 5.3 Pre-Subscribed Topics

When the SSE connection is established, the server returns a `personalTopics` list in the `ClientConnection` event. These topics are **already subscribed server-side** -- the client does not need to send a separate subscription request for them.

The `respectPreSubscribedTopics` config flag controls whether these are treated as already-subscribed (default: true in messaging context). When true, subscribing to a pre-subscribed topic is a no-op that immediately fires `onSubscriptionChangeSuccess`.

---

## 6. Subscription Management Protocol

### 6.1 Subscribe

```
PUT /realtime/realtimeFrontendSubscriptions?ids=List((topic:<url-encoded-topic-urn>,clientConnectionId:<connection-id>),(topic:...,...))
X-RestLi-Method: BATCH_UPDATE
Content-Type: application/json

{
  "entities": {
    "(topic:<url-encoded-topic-urn>,clientConnectionId:<connection-id>)": {},
    ...
  }
}
```

The URL query parameter `ids` is a Rest.li compound key list. Each element is a tuple of `(topic:<urn>,clientConnectionId:<id>)`.

The request body is a `BatchSubscriptionRequest` with `entities` map. Each value is an empty `Subscription` object (no configuration fields).

When the query string would exceed URL length limits, `QueryTunnelUtil` may tunnel the request (move query params into POST body with method override). This is controlled by the `useTunnelRequest` config flag (default: true in messaging context).

### 6.2 Unsubscribe

```
DELETE /realtime/realtimeFrontendSubscriptions?ids=List((topic:<url-encoded-topic-urn>,clientConnectionId:<connection-id>))
X-RestLi-Method: BATCH_DELETE
```

Same URL format as subscribe, but using DELETE method with `BATCH_DELETE` Rest.li method header.

### 6.3 Subscription Response

The response is a `BatchSubscriptionStatus` containing:
- `results` -- Map of successfully subscribed/unsubscribed topic keys
- `errors` -- Map of failed topic keys to `TopicSubscriptionStatus` with a `status` code

Error handling:
- 4xx errors on individual topics: dropped (not retried)
- Non-4xx errors on individual topics: retried
- Overall request failure with non-4xx: retried via `ConnectionRetryStrategy`
- Overall request failure with 4xx: dropped (no retry)
- HTTP 412: triggers full reconnection (`onReconnectRequested`)

### 6.4 Subscription State Machine

Each topic in `SubscriptionManager` has a state:

```
0: NOT_SUBSCRIBED  -- Initial state, or after unsubscribe completes
1: PENDING         -- Subscribe/unsubscribe request in flight
2: SUBSCRIBED      -- Successfully subscribed on server
3: ERROR           -- Subscription failed
```

Transitions:
```
NOT_SUBSCRIBED --(subscribe request sent)--> PENDING
PENDING --(success)--> SUBSCRIBED
PENDING --(error)--> ERROR
SUBSCRIBED --(unsubscribe request sent)--> PENDING
PENDING --(unsubscribe success)--> NOT_SUBSCRIBED
```

### 6.5 Subscription Retry

Individual subscription requests retry up to **3 times** with exponential backoff:
- Delay: `2^retryCount * 100ms` (i.e., 100ms, 200ms, 400ms)
- Computed by `BackoffComputer.getBackoffTimeMillis()`

---

## 7. Connection Lifecycle

### 7.1 Connection States

The `RealTimeOnlineManager` tracks connection state as a string:

```
DISCONNECTED -> CONNECTING -> CONNECTED -> DISCONNECTING -> DISCONNECTED
```

### 7.2 Connection Flow

1. **Pre-flight checks**: Network available, app in foreground, not already connected/connecting, no active underlying connection
2. **Time sync**: `ServerTimeManager.startTimeSync()` sends 4 requests to `/realtime/realtimeFrontendTimestamp` (first is discarded for latency measurement, next 3 are used to calculate server-client time offset)
3. **SSE connect**: `LongPollStreamNetworkClient.connect("/realtime/connect", ...)` opens the HTTP GET with `Accept: text/event-stream`
4. **Stream processing**: `EventProcessor.processStream()` reads the stream character-by-character, parsing SSE frames
5. **First event**: `ClientConnection` event received -- contains connection ID and pre-subscribed topics
6. **State transition**: `CONNECTING -> CONNECTED`, connection retry counter reset
7. **Batch subscribe**: All pending topic subscriptions are sent to the server
8. **Event loop**: `DecoratedEvent` and `Heartbeat` events are delivered to subscribers

### 7.3 Connection Termination

The connection ends when:
- **Client disconnect**: App goes to background, screen off, or explicit stop
- **Server close**: Server closes the connection (graceful)
- **Connection failure**: Network error, timeout, or HTTP error response

### 7.4 Foreground-Only Connection

The real-time connection is **active only when the app is in the foreground**:
- `onActivityResumed` -> `RealTimeOnlineManager.start()` (connect)
- `onTrimMemory(level >= 20)` -> `RealTimeOnlineManager.stop()` (disconnect)
- `SCREEN_OFF` broadcast -> `stop()`
- `SCREEN_ON` broadcast (if foreground) -> `start()`
- Network connectivity change -> `start()` or `stop()` accordingly

This is managed by `SystemSubscriptions` (a `ContentProvider` initialized at app startup).

---

## 8. Reconnection and Retry Logic

### 8.1 Connection Retry Strategy

The messaging module uses `LinearBackoffStrategy`:

```
retryDelay = (random(0, 2000) + 5000) * failureCount  milliseconds
```

- After 1st failure: 5000-7000ms
- After 2nd failure: 10000-14000ms
- **Maximum 2 retries** (gives up after 2 consecutive failures)
- Failure counter resets on successful connection

Only non-4xx errors trigger retries. 4xx errors are considered permanent (not retried).

### 8.2 Server Drop Handling

When the server closes the connection (not client-initiated):
- `shouldAttemptReconnectAfterServerDrop()` checks if two server drops happened within `serverDropReconnectThresholdMillis`
- If this is the first drop, or enough time has passed since the last drop: immediate reconnect
- If two drops happen too quickly: give up (prevent reconnection storm)

### 8.3 HTTP 412 Handling

A 412 (Precondition Failed) response on a subscription request triggers a full reconnect cycle (disconnect then reconnect). This likely indicates a stale connection ID.

---

## 9. Client Heartbeat (Connectivity Tracking)

`RealTimeConnectivityTracker` sends periodic heartbeats **from client to server** (separate from the server's SSE heartbeats):

```
POST /realtime/realtimeFrontendClientConnectivityTracking?action=sendHeartbeat
Content-Type: application/json

{
  "realtimeSessionId": "<uuid>",
  "mpName": "<app-name>",
  "mpVersion": "<app-version>",
  "isLastHeartbeat": false  // true when monitoring stops
}
```

- Frequency: configurable via `heartbeatFrequencyMillis`, minimum 30 seconds
- Session ID: Random UUID, regenerated when monitoring stops
- The `isLastHeartbeat: true` flag is sent when the tracker stops (app goes to background)

---

## 10. Server Time Synchronization

`ServerTimeManager` synchronizes client time with server time:

1. Sends 4 GET requests to `/realtime/realtimeFrontendTimestamp`
2. Response contains `{ "timestamp": <server-time-millis> }`
3. First response is discarded (used only for latency measurement)
4. Subsequent 3 responses are used by `ServerTimeCalculator` to compute the offset
5. `getServerTime()` returns the calculated server time based on the offset

Retry: up to 3 consecutive failures with exponential backoff (`2^failureCount * 100ms`). 4xx errors (except 408 Request Timeout) are not retried.

---

## 11. Architecture Summary

```
                                    +---------------------------+
                                    |  LinkedIn SSE Server      |
                                    |  /realtime/connect        |
                                    +---------------------------+
                                              |
                                        SSE Stream
                                    (text/event-stream)
                                              |
                                              v
                              +-------------------------------+
                              | LongPollStreamNetworkClient   |
                              | (HTTP GET, long-lived)        |
                              | Uses Cronet via LinkedInNetwork|
                              +-------------------------------+
                                              |
                                        InputStream
                                              |
                                              v
                              +-------------------------------+
                              | EventProcessor                |
                              | (SSE frame parser)            |
                              | Reads char-by-char            |
                              | Accumulates data: lines       |
                              | Dispatches on empty line      |
                              +-------------------------------+
                                              |
                                        RealtimeEvent (JSON)
                                              |
                            +-----------------+-----------------+
                            |                 |                 |
                            v                 v                 v
                    ClientConnection      Heartbeat      DecoratedEvent
                    (conn established)   (keepalive)    (topic + payload)
                            |                                  |
                            v                                  v
                    RealTimeOnlineManager             SubscriptionManager
                    - stores connection ID            - routes payload to
                    - triggers batch subscribe          registered subscribers
                    - manages state machine             by topic URN
                                                               |
                                    +-----------+-----------+--+--+-----------+
                                    |           |           |     |           |
                                    v           v           v     v           v
                            messagesTopic  conversations typing  seen   tabBadges
                            (new msgs)     (conv updates) indic. rcpts  (badges)
```

---

## 12. Key Insights for Rust Implementation

1. **Protocol**: Standard SSE (`text/event-stream`), not WebSocket. Any HTTP client that can hold a connection and stream the response body works.

2. **No separate auth**: Same cookies and headers as regular API calls. The SSE endpoint is at `/realtime/connect`, NOT under `/voyager/api/`.

3. **Connection ID is critical**: The `ClientConnection` event must be received and its `id` stored. All subscription requests include this connection ID.

4. **Pre-subscribed topics**: The server may already subscribe you to personal topics. Check the `personalTopics` list in the connection event before sending redundant subscribe requests.

5. **Subscription uses Rest.li BATCH_UPDATE/BATCH_DELETE**: The subscribe endpoint is a standard Rest.li batch resource, not a custom protocol.

6. **JSON payloads**: Everything is JSON (Pegasus serialization format). No protobuf.

7. **Foreground only**: The real-time connection is only maintained while the app is in the foreground. Background push is handled by FCM/GCM (separate system, not covered here).

8. **Reconnection is conservative**: Max 2 retries with linear backoff (5-14s). After that, gives up until next app foreground transition.

9. **Topic URN format**: `urn:li-realtime:<topic-name>:urn:li-realtime:myself` for personal topics. URL-encode the URN when constructing subscription request keys.

10. **Query tunneling**: Long subscription requests may be tunneled (query params moved to POST body). The `useTunnelRequest` flag is enabled by default.
