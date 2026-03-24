# Model Corrections & Live Validation Checklist

Consolidation of known risks and uncertainties from TASK-0027 through TASK-0032.
All models were built from jadx decompilation of LinkedIn Android APK (Zephyr build,
v6.1.1). None have been validated against the live API yet.

Analysis date: 2026-03-24.

---

## 1. Known Risks: Decompiled Models vs Live API

### 1.1 Zephyr vs Voyager Model Differences

Our Pegasus models were extracted from the **Zephyr** (China) APK build. The
international build differs in several ways that may affect response shapes:

| Area | Risk | Source |
|------|------|--------|
| **Dash endpoints** | International build has ~219 "Dash" route variants (e.g. `voyagerIdentityDashProfiles`). These may return different model shapes than the legacy routes we modelled. | `re/intl_vs_zephyr_diff.md` section 1 |
| **Messaging models** | International build has fewer standalone messaging Pegasus models. `Conversation` and `Event` may have migrated to Dash endpoints with different field sets. | `re/intl_vs_zephyr_diff.md` section 4 |
| **Missing models** | International build has `voyager/hiring` models absent from Zephyr. Zephyr has `voyager/news`, `voyager/video` absent from international. | `re/intl_vs_zephyr_diff.md` section 4 |
| **GraphQL route** | International build has a `GRAPHQL("graphql")` route entirely absent from Zephyr. Some data may only be available via GraphQL. | `re/intl_vs_zephyr_diff.md` section 1 |
| **Zephyr-only fields** | Fields like `weChatNameCardURL` on Profile, `zephyrNearby*` routes, etc. will not appear in international responses. Harmless (they're Optional) but noisy. | `re/intl_vs_zephyr_diff.md` section 1 |

### 1.2 Fields Using `Option<Value>` That Need Type Refinement

These fields are typed as `Option<serde_json::Value>` in our Rust models because
we don't yet know the exact live response shape. Each needs live validation to
determine the correct typed struct.

| Model | Field | Expected Type (from decompilation) | Risk |
|-------|-------|------------------------------------|------|
| `UpdateV2` | `actor` | `ActorComponent` (complex struct with name, image, subDescription) | May have different fields in Dash feed |
| `UpdateV2` | `commentary` | `TextComponent` (wrapper around `AttributedText`) | May be plain string or rich text object |
| `UpdateV2` | `content` | `FeedComponent` (union: article, image, video, etc.) | Polymorphic; needs union discriminator validation |
| `UpdateV2` | `social_detail` | `SocialDetail` (we have a typed struct but elements array is `Value`) | Reaction elements shape unknown |
| `UpdateV2` | `update_metadata` | `UpdateMetadata` struct | Tracking/visibility fields |
| `UpdateV2` | `contextual_header` | `ContextualHeaderComponent` | Text + actor info |
| `UpdateV2` | `header` | `HeaderComponent` | Text component |
| `UpdateV2` | `reshared_update` | `UpdateV2` (recursive) | Self-referential; may hit serde depth limits |
| `MiniProfile` | `picture` | `Image` (union: VectorImage, MediaProxyImage, etc.) | Union discriminator format uncertain |
| `MiniProfile` | `background_image` | `Image` (union) | Same as picture |
| `Profile` | `location` | `ProfileLocation` struct | May have `countryCode`, `postalCode` |
| `Profile` | `geo_location` | `ProfileGeoLocation` struct | May have `geoUrn` |
| `Profile` | `mini_profile` | `MiniProfile` (we have the struct but keep it as Value) | Should be typed once live shape confirmed |
| `Profile` | `profile_picture` | `PhotoFilterPicture` wrapping `Image` | Extra wrapping layer unclear |
| `Profile` | `background_image` | `BackgroundImage` struct or `Image` union | May differ from MiniProfile's image |
| `Conversation` | `participants` | `Vec<MessagingProfile>` (union: member/company/bot) | Union encoding uncertain |
| `Conversation` | `events` | `Vec<Event>` (we have MessagingEvent struct) | May not be inlined in conversation response |
| `Conversation` | `receipts` | `Vec<ParticipantReceipts>` | Read receipt shape unknown |
| `MessagingEvent` | `from` | `MessagingProfile` (union) | Union encoding format |
| `MessagingEvent` | `event_content` | `EventContent` (union: MessageEvent, etc.) | Union discriminator key format critical |
| `MessagingEvent` | `quick_replies` | `Vec<AttributedText>` | Rich text shape |
| `Connection` | `mini_profile` | `MiniProfile` | Should be typed |
| `Connection` | `phone_numbers` | `Vec<PhoneNumber>` | Phone number shape unknown |
| `Connection` | `twitter_handles` | `Vec<TwitterHandle>` | Handle shape unknown |
| `Connection` | `we_chat_contact_info` | `WeChatContactInfo` | China-only, likely absent |
| `NotificationCard` | `headline` | `TextViewModel` (has `text` field) | May have `attributedText` with rich formatting |
| `NotificationCard` | `sub_headline` | `TextViewModel` | Same as headline |
| `NotificationCard` | `kicker` | `TextViewModel` | Same |
| `NotificationCard` | `header_image` | `ImageViewModel` | Shape unknown |
| `NotificationCard` | `badge_icon` | `ImageViewModel` | Shape unknown |
| `NotificationCard` | `actions` | `Vec<CardAction>` | Action shape unknown |
| `NotificationCard` | `card_action` | `CardAction` | Action shape unknown |
| `SearchHit` | `hit_info` | Union: `SearchProfile`, `SearchJob`, etc. | Union discriminator format |

### 1.3 Union Type Encoding Uncertainty

The decompiled code shows unions serialized with FQN keys:

```json
{
  "com.linkedin.voyager.messaging.event.message.MessageEvent": {
    "body": "Hello!"
  }
}
```

Risks:
- **FQN vs short key**: The server may use short keys (e.g. `messageEvent`) instead
  of FQNs in JSON responses. The FQN format is confirmed in decompiled `accept()`
  methods and test assertions, but the server-side serialization config may differ.
- **Protobuf-first**: If the server defaults to protobuf and our `Accept: application/json`
  request gets a different serialization path, union keys might use a different format.
- **Dash endpoints**: The newer Dash endpoints may use a completely different union
  encoding (possibly GraphQL-style `__typename` discriminator).

Source: `re/serialization_edge_cases.md` section 4.

### 1.4 Protobuf vs JSON Response Format Risk

The production Android app requests `application/vnd.linkedin.deduped+x-protobuf+2.0`
by default. Our Rust client requests `application/json`.

| Risk | Detail |
|------|--------|
| Server may not honor JSON Accept | The server might require protobuf for certain endpoints or return `406 Not Acceptable`. |
| Deduped vs inlined | The protobuf format uses deduplication (entities referenced by URN, not inlined). JSON responses may or may not use the same deduplication. If deduped, we need entity resolution logic. |
| Debug JSON format | `application/vnd.linkedin.mobile.debug+json` forces hierarchical (non-deduped) JSON but may trigger detection or rate limiting. |
| Field omission | The server may omit fields in JSON that would be present in protobuf, or vice versa. |

Source: `re/restli_protocol.md` section 5.

### 1.5 Decoration/Recipe Version Mismatch Risk

Our recipe IDs come from the decompiled APK (specific version). Recipe versions are
integer-suffixed and change between app releases:

```
com.linkedin.voyager.deco.identity.normalizedprofile.shared.ApplicantProfile-13
com.linkedin.voyager.deco.jobs.shared.FullJobPosting-71
```

Risks:
- **Version drift**: The server may reject outdated recipe versions, returning
  a minimal response or an error.
- **Missing recipes**: Some recipes in our list may have been removed or renamed
  in newer server versions.
- **Recipe determines fields**: The specific recipe version determines which
  fields are returned. A wrong version may return a different field set than
  we modelled.

Source: `re/restli_protocol.md` section 6.

### 1.6 Timestamp Format

All timestamps are `i64` epoch milliseconds. This is well-documented in the
decompilation and consistent across models. Low risk, but verify:
- Confirm the server sends numbers, not ISO 8601 strings.
- Confirm millisecond precision (not seconds).

Source: `re/serialization_edge_cases.md` section 3.

### 1.7 camelCase Field Name Assumption

All our Rust models use `#[serde(rename_all = "camelCase")]`. The field names come
from `startRecordField("fieldName", ...)` in decompiled `accept()` methods, which
should match the JSON wire format. However:
- Some field names in decompiled code may be obfuscated (Proguard) in the Zephyr build.
- The international build may use different field names for some models.

### 1.8 SocialActivityCounts Missing Fields

Our `SocialActivityCounts` struct has 5 fields. The decompiled model has 11 fields
including `entityUrn`, `urn`, `socialDetailEntityUrn`, `reacted`, `reactionTypeCounts`,
`likedByOrganizationActor`, and `reactionByOrganizationActor`. We should add these
if they appear in live responses.

### 1.9 Conversation `lastActivityAt` Field

Our `Conversation` struct has `last_activity_at: Option<u64>` but this field does
NOT appear in the decompiled `Conversation` model from pegasus_models.md. It may
be computed client-side or returned differently by the Dash messaging endpoint.
Needs live validation to confirm whether the server sends it.

---

## 2. Live Validation Checklist

### 2.1 Basic Protocol Validation

- [ ] **Does `Accept: application/json` work?**
  Send a simple GET to `/voyager/api/me` with `Accept: application/json`.
  Confirm the response is JSON, not protobuf. Check Content-Type header.

- [ ] **Does `Accept: application/vnd.linkedin.mobile.debug+json` work?**
  Try the debug JSON format. If it returns non-deduped hierarchical JSON,
  it simplifies our parsing. If it triggers detection, fall back.

- [ ] **Is the response deduped or inlined?**
  Check whether entities (e.g. MiniProfile inside Conversation participants)
  are inlined or referenced by URN with an `included` array at top level.

- [ ] **Are recipe/decoration IDs accepted?**
  Try `decorationId=com.linkedin.voyager.deco.identity.FullProfile` (or similar).
  Verify the response includes the expected fields. Try an outdated version number
  to see the error behavior.

### 2.2 Field Name and Shape Validation

- [ ] **Confirm camelCase field names** in actual JSON responses match our
  `rename_all = "camelCase"` assumption. Spot-check at least: `entityUrn`,
  `firstName`, `lastName`, `publicIdentifier`, `createdAt`, `totalEventCount`.

- [ ] **Null vs absent behavior**: Send a request and check whether optional
  fields are absent from JSON or present with `null` value. Our `#[serde(default)]`
  handles both, but know which pattern the server uses.

- [ ] **Empty collections**: Check whether the server sends `"events": []` or
  omits the field entirely when a conversation has no inline events.

- [ ] **Timestamp format**: Confirm timestamps like `createdAt`, `publishedAt`
  are numeric (epoch millis), not ISO 8601 strings.

### 2.3 Union Encoding Validation

- [ ] **MessagingEvent.eventContent union key format**: Send a request to
  `messaging/conversations/{id}/events` and inspect the `eventContent` field.
  Is the key a FQN like `"com.linkedin.voyager.messaging.event.MessageEvent"`
  or a short key like `"messageEvent"`?

- [ ] **MessagingProfile (participant) union key format**: Check `participants`
  array in a Conversation response. Same FQN vs short key question.

- [ ] **Image union key format**: Check `picture` or `backgroundImage` on a
  MiniProfile. Is it `"com.linkedin.common.VectorImage"` or `"vectorImage"`?

- [ ] **SearchHit.hitInfo union key format**: Check search results for the
  discriminator format.

### 2.4 Pagination Validation

- [ ] **Pagination format**: Confirm `paging` object has `start`, `count`, `total`.
  Check whether `total` is always present or sometimes absent.

- [ ] **Cursor-based pagination**: Check whether any endpoint returns a
  `paginationToken` instead of/in addition to `start`/`count`/`total`.

- [ ] **Empty page behavior**: Request a page past the end (e.g. `start=99999`).
  Confirm the response has `elements: []` and appropriate paging metadata.

### 2.5 Endpoint-Specific Validation

#### Feed (`GET feed/updates`)
- [ ] Confirm `q=findFeed` query parameter is required.
- [ ] Confirm `elements` contains `UpdateV2`-shaped objects.
- [ ] Inspect `actor` field shape -- is it an object with `name`, `image`?
- [ ] Inspect `commentary` field -- is it `{text: {text: "..."}}` or flat string?
- [ ] Inspect `content` field -- what union variants appear for articles, images, videos?

#### Messaging (`GET messaging/conversations`)
- [ ] Confirm conversations list has expected fields.
- [ ] Check whether `events` are inlined or require separate fetch.
- [ ] Check `participants` union encoding.
- [ ] Verify `unreadCount`, `totalEventCount` are present.

#### Messaging Events (`GET messaging/conversations/{id}/events`)
- [ ] Confirm event list is paginated.
- [ ] Check `eventContent` union encoding (FQN key vs short key).
- [ ] Inspect `MessageEvent.body` -- plain string or rich text?
- [ ] Check `from` field union encoding.

#### Profile (`GET identity/profiles/{id}`)
- [ ] With decoration: confirm expected fields are present.
- [ ] Without decoration: what minimal fields are returned?
- [ ] Check `miniProfile` embedding -- is it inlined or a URN reference?
- [ ] Confirm `profilePicture` shape.

#### Connections (`GET relationships/connections`)
- [ ] Confirm `sortType=RECENTLY_ADDED` parameter works.
- [ ] Check `miniProfile` embedding in each connection.
- [ ] Verify `createdAt` is present and is epoch millis.

#### Search (`GET search/hits`)
- [ ] Confirm `q=guided&guides=List(v->people)` query format works.
- [ ] Inspect `hitInfo` union format in results.
- [ ] Check whether `SearchProfile.miniProfile` is inlined.

#### Notifications (`GET identity/notificationCards`)
- [ ] Confirm endpoint path is correct.
- [ ] Inspect `TextViewModel` shape for `headline`, `subHeadline`, `kicker`.
- [ ] Check `contentType` enum values against our expectations.

### 2.6 Error Handling Validation

- [ ] **401 response shape**: Confirm error response matches `ErrorResponse`
  model from `re/restli_protocol.md` section 8.
- [ ] **Rate limiting**: Check for 429 responses and inspect headers.
- [ ] **Invalid recipe version**: What error does the server return?

---

## 3. Per-Model Correction Log

This section will be filled in as live validation is performed. Each entry should
record: field name, expected type, actual type, action taken.

### 3.1 Feed Models

| Field | Expected | Actual | Action |
|-------|----------|--------|--------|
| _(pending live validation)_ | | | |

### 3.2 Messaging Models

| Field | Expected | Actual | Action |
|-------|----------|--------|--------|
| _(pending live validation)_ | | | |

### 3.3 Profile Models

| Field | Expected | Actual | Action |
|-------|----------|--------|--------|
| _(pending live validation)_ | | | |

### 3.4 Connection Models

| Field | Expected | Actual | Action |
|-------|----------|--------|--------|
| _(pending live validation)_ | | | |

### 3.5 Search Models

| Field | Expected | Actual | Action |
|-------|----------|--------|--------|
| _(pending live validation)_ | | | |

### 3.6 Notification Models

| Field | Expected | Actual | Action |
|-------|----------|--------|--------|
| _(pending live validation)_ | | | |

---

## 4. Summary of Required Serde Changes (Post-Validation)

After live validation, these changes are anticipated:

1. **Union type deserialization**: Implement custom deserializers for `EventContent`,
   `MessagingProfile`, `Image`, and `SearchHit.hitInfo` based on the actual
   discriminator key format (FQN vs short key).

2. **Type refinement**: Replace `Option<Value>` fields with typed structs where
   the live shape is confirmed.

3. **Missing fields**: Add any fields present in live responses that are missing
   from our current models (captured by `#[serde(flatten)] extra` fields).

4. **Field removal**: Remove fields from our models that never appear in live
   responses (e.g. Zephyr-specific fields on international API).

5. **Required vs optional**: Based on live responses, promote consistently-present
   fields from `Option<T>` to bare `T`.

6. **Deduplication handling**: If responses use entity deduplication (URN references
   with top-level `included` array), implement entity resolution logic.
