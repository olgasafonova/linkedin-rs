# Send Message API

## Endpoint

```
POST /voyager/api/voyagerMessagingDashMessengerMessages?action=createMessage
```

Note: action is `createMessage`, NOT `create`. The legacy `messaging/conversations?action=create` endpoint returns 403 for non-browser clients.

## Request

### Headers

Standard LinkedIn headers are sufficient (same as read endpoints):
- `Csrf-Token: {JSESSIONID value}` (e.g., `ajax:0000000000000000001`)
- `X-RestLi-Protocol-Version: 2.0.0`
- `Content-Type: application/json` (the browser sends `text/plain;charset=UTF-8` but `application/json` works too)
- `Accept: application/json`

No special cookies beyond `li_at` + self-generated `JSESSIONID` are required.

### Payload

```json
{
  "message": {
    "body": {
      "attributes": [],
      "text": "Hello World"
    },
    "originToken": "ee08dae8-bec6-46a5-8e04-c0bb135b6fb6",
    "renderContentUnions": []
  },
  "mailboxUrn": "urn:li:fsd_profile:ACoAABxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "trackingId": "<16 random bytes as raw string>",
  "dedupeByClientGeneratedToken": false,
  "hostRecipientUrns": ["urn:li:fsd_profile:ACoAACyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"]
}
```

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `message.body.text` | string | yes | Message text |
| `message.body.attributes` | array | yes | Empty array for plain text. Used for @mentions, links, etc. |
| `message.originToken` | string | yes | UUID v4, used for deduplication |
| `message.renderContentUnions` | array | yes | Empty for text-only. Used for attachments, cards, etc. |
| `mailboxUrn` | string | yes | Sender's `fsd_profile` URN |
| `trackingId` | string | **yes** | **16 random bytes as raw chars** — without this, server returns 400 |
| `dedupeByClientGeneratedToken` | bool | yes | Set to `false` |
| `hostRecipientUrns` | array | yes | List of recipient `fsd_profile` URNs |

### trackingId Format (Critical)

The `trackingId` is NOT a UUID or base64 string. It is 16 random bytes directly mapped to characters via `byte as char`. In the JSON wire format this produces escape sequences for non-printable bytes (e.g., `\u0004`, `\b`).

**Rust:**
```rust
let tracking_bytes: [u8; 16] = rand::random();
let tracking_id: String = tracking_bytes.iter().map(|&b| b as char).collect();
```

**JavaScript:**
```javascript
const arr = new Uint8Array(16);
crypto.getRandomValues(arr);
const trackingId = String.fromCharCode(...arr);
```

Without this field, or with a UUID/string value, the server returns `{"status": 400}` with no further details.

## Response (HTTP 200)

```json
{
  "value": {
    "renderContentUnions": [],
    "entityUrn": "urn:li:msg_message:(urn:li:fsd_profile:...,2-...)",
    "backendConversationUrn": "urn:li:messagingThread:2-...",
    "senderUrn": "urn:li:msg_messagingParticipant:urn:li:fsd_profile:...",
    "originToken": "ee08dae8-bec6-46a5-8e04-c0bb135b6fb6",
    "body": {
      "attributes": [],
      "text": "Hello World"
    },
    "backendUrn": "urn:li:messagingMessage:2-...",
    "conversationUrn": "urn:li:msg_conversation:(urn:li:fsd_profile:...,2-...)",
    "deliveredAt": 1774352369361
  }
}
```

## Discovery Process

1. Initial attempt used legacy `messaging/conversations?action=create` endpoint (from python linkedin-api) — returned 403
2. Tried Dash endpoint `voyagerMessagingDashMessengerMessages?action=create` — returned 400
3. Used Chrome DevTools MCP to send a message from the browser and capture the exact request
4. Discovered the action is `createMessage` (from network capture)
5. Replaying from JS console without `trackingId` still returned 400
6. Adding `trackingId` with 16 random bytes (as `String.fromCharCode(...new Uint8Array(16))`) returned 200
7. Confirmed the same works from the Rust CLI with just `li_at` + self-generated JSESSIONID — no extra browser cookies needed

## Error Codes

| HTTP Status | Cause |
|-------------|-------|
| 400 | Missing `trackingId`, or `trackingId` is a UUID string instead of raw bytes |
| 403 | Wrong endpoint (e.g., `action=create`), or CSRF token mismatch |
| 200 | Success |
