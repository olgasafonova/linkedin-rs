# Pegasus Model Extraction: Data Models & DTOs

Extracted from `com.linkedin.android.pegasus.gen.voyager` in the jadx decompiled output.
Analysis date: 2026-03-24.

---

## 1. Architecture: How Pegasus Models Work

### 1.1 Base Interfaces

All Pegasus models are generated from LinkedIn's Rest.li IDL (Pegasus schema language). The type hierarchy:

```
DataTemplate<T>                    -- base interface: id() + accept(DataProcessor)
  |
  +-- RecordTemplate<T>            -- struct-like models (most models)
  |     Flavor: RECORD | PARTIAL | PATCH
  |
  +-- UnionTemplate<T>             -- tagged unions (discriminated sum types)
```

**Key observations for Rust serde:**

- `RecordTemplate` = Rust struct with `#[serde(default)]` on optional fields
- `UnionTemplate` = Rust enum with `#[serde(untagged)]` or Rest.li's union encoding
- Every field has a companion `hasXxx: boolean` -- this is how optionality works at runtime
- The `accept(DataProcessor)` method reveals the **exact JSON field names** via `startRecordField("fieldName", hashCode)` calls
- Union members are identified by their fully-qualified schema name (e.g., `"com.linkedin.voyager.messaging.MessagingMember"`)

### 1.2 Optionality Pattern

Every model uses this pattern for optional fields:

```java
public final String firstName;         // the value (may be null)
public final boolean hasFirstName;     // whether the field was present in the response
```

In the builder's `build(Flavor.RECORD)` method, required fields are validated:
```java
validateRequiredRecordField("entityUrn", this.hasEntityUrn);
```

**Rust mapping:** Fields NOT validated in `build(RECORD)` are optional. Use `Option<T>` for those, bare `T` for required ones.

### 1.3 URN System

URNs are the universal entity reference format:

```
urn:{namespace}:{entityType}:{id}
```

Most URNs use namespace `li`:
- `urn:li:member:123456789` -- member/profile
- `urn:li:fs_miniProfile:ACoAABxxxxxx` -- mini profile
- `urn:li:messagingThread:2-YmI1...` -- messaging conversation
- `urn:li:fs_event:...` -- messaging event
- `urn:li:activity:...` -- feed activity
- `urn:li:jobPosting:...` -- job posting
- `urn:li:invitation:...` -- connection invitation
- `urn:li:company:...` -- company
- `urn:li:school:...` -- school

The `Urn` class (`com.linkedin.android.pegasus.gen.common.Urn`) parses URNs lazily. For Rust, a simple newtype `String` with parse-on-demand is appropriate. The URN `entityType` field maps directly to the entity domain.

Two URN fields appear on most models:
- `entityUrn` -- the canonical identifier for this entity within the Pegasus data store
- `objectUrn` -- the "real" entity URN (e.g., `urn:li:member:XXX`), sometimes absent

### 1.4 Serde: JSON Field Names

The `accept()` method reveals exact JSON field names. The pattern is:
```java
dataProcessor.startRecordField("fieldName", hashCode);
```

The field name string (first arg) is the exact JSON key. The hashCode is an internal optimization.

For union types:
```java
dataProcessor.startUnionMember("com.linkedin.voyager.messaging.MessagingMember", hashCode);
```

The string is the Rest.li schema FQN used as the union discriminator key in JSON.

### 1.5 Collection Response Shape

API responses for collections follow this shape:
```json
{
  "elements": [...],
  "paging": {
    "start": 0,
    "count": 10,
    "total": 42
  },
  "metadata": { ... }
}
```

The `CollectionMetadata` model:
- `id`: String (optional) -- collection identifier
- `type`: String (optional) -- collection type
- `paginationToken`: String (optional) -- opaque token for cursor-based pagination

### 1.6 Enum Pattern

All enums include a `$UNKNOWN` variant as a catch-all for forward compatibility. They also have an `of(String)` factory method that maps string names to enum values, falling back to `$UNKNOWN`.

**Rust mapping:**
```rust
#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ReactionType {
    Like,
    Praise,
    // ...
    #[serde(other)]
    Unknown,
}
```

---

## 2. Package Catalog

Full sub-package tree under `com.linkedin.android.pegasus.gen.voyager` (1458 non-builder .java files total):

| Package | Sub-packages | Purpose |
|---------|-------------|---------|
| `common` | `communications`, `handles`, `heathrow`, `inlay`, `lego`, `ux` | Shared types: Me, Image, DateRange, GraphDistance, CollectionMetadata, TextViewModel, etc. |
| `identity` | `ecosystem`, `guidededit`, `me`, `normalizedprofile`, `notifications`, `profile`, `profilequality`, `shared`, `ubiquitousedit`, `zephyredit` | Profile, education, positions, skills, endorsements, notifications |
| `messaging` | `bots`, `create`, `event`, `peripheral`, `presence`, `realtime`, `shared`, `typeahead` | Conversations, events, messages, member profiles, typing indicators |
| `feed` | `actions`, `packageRecommendations`, `prototype`, `render`, `shared`, `social`, `urlpreview` | Feed updates, comments, reactions, social details |
| `jobs` | `jobsearch`, `premiuminsights`, `shared` | Job postings, applications, seeker preferences |
| `relationships` | `invitation`, `notifications`, `shared` (with `annotation`, `connection`, `discovery`, `prop`, `pymk`, `suggestion`, `thermometer`) | Connections, invitations, PYMK |
| `search` | `shared` | Search profiles, facets, typeahead |
| `entities` | `common`, `company`, `gamification`, `group`, `incommon`, `job`, `school`, `shared` | Entity cards for jobs, companies, schools |
| `contentcreation` | `shared` | Post creation (shares) |
| `organization` | `ads`, `analytics`, `content`, `feed`, `insights`, `landingPage`, `media`, `pendingadmin`, `premium`, `shared` | Company pages |
| `growth` | `abi`, `bizcard`, `calendar`, `chinalaunchpad`, `communications`, `confirmation`, `events`, `goal`, `handles`, `interactive`, `interests`, `invitation`, `lego`, `nearbyPeople`, `onboarding`, `profile`, `seo`, `socialProof` | Onboarding, growth features |
| `premium` | `boost`, `mypremium`, `onboarding`, `profinder`, `shared`, `welcome` | Premium features |
| `deco` | `common`, `identity`, `jobs`, `organization` | Decoration schemas (field projection descriptors) |
| `publishing` | -- | Articles, first-party content |
| `groups` | -- | Group membership |
| `typeahead` | -- | Typeahead suggestions |
| `video` | `stories` | Video/stories content |
| `learning` | `shared` | LinkedIn Learning |
| `news` | -- | News storylines |
| `salary` | `shared`, `submission` | Salary data |

---

## 3. Core Models by Domain

### 3.1 Common Types

#### `Me` (voyager.common)
Current authenticated user. Required for session bootstrapping.

| Field | Type | JSON Key | Required | Notes |
|-------|------|----------|----------|-------|
| plainId | long | `plainId` | **yes** | Numeric member ID |
| miniProfile | MiniProfile | `miniProfile` | **yes** | Embedded mini profile |
| publicContactInfo | PublicContactInfo | `publicContactInfo` | **yes** | Twitter handles etc. |
| premiumSubscriber | boolean | `premiumSubscriber` | no | Premium status |
| realByEmerald | boolean | `realByEmerald` | no | Identity verification |

#### `CollectionMetadata` (voyager.common)
Pagination metadata for collection responses.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| id | String | `id` | no |
| type | String | `type` | no |
| paginationToken | String | `paginationToken` | no |

#### `Image` (voyager.common) -- **UnionTemplate**
A tagged union for different image source types.

| Variant | Schema FQN | Type |
|---------|-----------|------|
| mediaProcessorImage | `com.linkedin.voyager.common.MediaProcessorImage` | MediaProcessorImage |
| mediaProxyImage | `com.linkedin.voyager.common.MediaProxyImage` | MediaProxyImage |
| url | `string` | String |
| vectorImage | `com.linkedin.common.VectorImage` | VectorImage |

#### `VectorImage` (common -- non-voyager)
The most common image format in responses.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| rootUrl | String | `rootUrl` | no |
| artifacts | List\<VectorArtifact\> | `artifacts` | no |
| attribution | String | `attribution` | no |
| digitalmediaAsset | String | `digitalmediaAsset` | no |

Image URLs are constructed as: `{rootUrl}{artifact.fileIdentifyingUrlPathSegment}`

#### `Date` (common -- non-voyager)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| day | int | `day` | no |
| month | int | `month` | no |
| year | int | `year` | no |

All fields optional -- allows partial dates like "March 2020" (no day).

#### `DateRange` (voyager.common)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| startDate | Date | `startDate` | no |
| endDate | Date | `endDate` | no |

#### `GraphDistance` (voyager.common) -- Enum
Connection degree between members.

| Variant | Meaning |
|---------|---------|
| SELF | You |
| DISTANCE_1 | Direct connection |
| DISTANCE_2 | 2nd degree |
| DISTANCE_3 | 3rd degree |
| OUT_OF_NETWORK | Not connected |
| $UNKNOWN | Fallback |

---

### 3.2 Identity / Profile

#### `MiniProfile` (voyager.identity.shared)
Lightweight profile used everywhere as an embedded reference.

| Field | Type | JSON Key | Required | Notes |
|-------|------|----------|----------|-------|
| entityUrn | Urn | `entityUrn` | **yes** | e.g. `urn:li:fs_miniProfile:ACoAABxxxxxx` |
| firstName | String | `firstName` | **yes** | |
| publicIdentifier | String | `publicIdentifier` | **yes** | URL slug (e.g., `john-doe-123`) |
| trackingId | String | `trackingId` | no | Base64 tracking token |
| objectUrn | Urn | `objectUrn` | no | e.g. `urn:li:member:123456` |
| lastName | String | `lastName` | no | |
| occupation | String | `occupation` | no | Current role headline |
| backgroundImage | Image | `backgroundImage` | no | |
| picture | Image | `picture` | no | Profile photo |

`id()` returns the `entityUrn` string -- this is the cache key.

#### `Profile` (voyager.identity.profile)
Full profile with all details. ~30+ fields.

| Field | Type | JSON Key | Required | Notes |
|-------|------|----------|----------|-------|
| entityUrn | Urn | `entityUrn` | **yes** | |
| firstName | String | `firstName` | **yes** | |
| lastName | String | `lastName` | no | |
| maidenName | String | `maidenName` | no | |
| headline | String | `headline` | no | |
| summary | String | `summary` | no | "About" section |
| industryName | String | `industryName` | no | |
| industryUrn | Urn | `industryUrn` | no | |
| locationName | String | `locationName` | no | |
| geoLocationName | String | `geoLocationName` | no | |
| geoCountryName | String | `geoCountryName` | no | |
| geoCountryUrn | Urn | `geoCountryUrn` | no | |
| geoLocation | ProfileGeoLocation | `geoLocation` | no | |
| geoLocationBackfilled | boolean | `geoLocationBackfilled` | no | |
| location | ProfileLocation | `location` | no | |
| address | String | `address` | no | |
| birthDate | Date | `birthDate` | no | |
| defaultLocale | Locale | `defaultLocale` | no | |
| entityLocale | Locale | `entityLocale` | no | |
| supportedLocales | List\<Locale\> | `supportedLocales` | no | |
| versionTag | String | `versionTag` | no | For optimistic concurrency |
| miniProfile | MiniProfile | `miniProfile` | no | Embedded mini profile ref |
| profilePicture | PhotoFilterPicture | `profilePicture` | no | |
| profilePictureOriginalImage | Image | `profilePictureOriginalImage` | no | |
| backgroundImage | BackgroundImage | `backgroundImage` | no | |
| backgroundPicture | PhotoFilterPicture | `backgroundPicture` | no | |
| backgroundPictureOriginalImage | Image | `backgroundPictureOriginalImage` | no | |
| pictureInfo | Picture | `pictureInfo` | no | |
| student | boolean | `student` | no | |
| elt | boolean | `elt` | no | Enterprise Learning Tab |
| state | State | `state` | no | Account state |
| phoneticFirstName | String | `phoneticFirstName` | no | |
| phoneticLastName | String | `phoneticLastName` | no | |
| showEducationOnProfileTopCard | boolean | `showEducationOnProfileTopCard` | no | |
| weChatNameCardURL | String | `weChatNameCardURL` | no | China variant |

#### `Position` (voyager.identity.profile)
Work experience entry.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| title | String | `title` | no |
| companyName | String | `companyName` | no |
| companyUrn | Urn | `companyUrn` | no |
| company | PositionCompany | `company` | no |
| description | String | `description` | no |
| locationName | String | `locationName` | no |
| geoLocationName | String | `geoLocationName` | no |
| geoUrn | Urn | `geoUrn` | no |
| timePeriod | DateRange | `timePeriod` | no |
| promotion | boolean | `promotion` | no |
| entityLocale | Locale | `entityLocale` | no |
| courses | List\<Urn\> | `courses` | no |
| honors | List\<Urn\> | `honors` | no |
| projects | List\<Urn\> | `projects` | no |
| recommendations | List\<Urn\> | `recommendations` | no |
| organizations | List\<Urn\> | `organizations` | no |
| region | Urn | `region` | no |

#### `Education` (voyager.identity.profile)
Education entry.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| schoolName | String | `schoolName` | no |
| schoolUrn | Urn | `schoolUrn` | no |
| school | MiniSchool | `school` | no |
| degreeName | String | `degreeName` | no |
| degreeUrn | Urn | `degreeUrn` | no |
| fieldOfStudy | String | `fieldOfStudy` | no |
| fieldOfStudyUrn | Urn | `fieldOfStudyUrn` | no |
| description | String | `description` | no |
| grade | String | `grade` | no |
| activities | String | `activities` | no |
| timePeriod | DateRange | `timePeriod` | no |
| entityLocale | Locale | `entityLocale` | no |
| companyUrn | Urn | `companyUrn` | no |
| courses | List\<Urn\> | `courses` | no |
| honors | List\<Urn\> | `honors` | no |
| projects | List\<Urn\> | `projects` | no |
| recommendations | List\<Urn\> | `recommendations` | no |
| testScores | List\<Urn\> | `testScores` | no |

#### `PublicContactInfo` (voyager.identity.shared)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| twitterHandles | List\<TwitterHandle\> | `twitterHandles` | no |

---

### 3.3 Messaging

#### `Conversation` (voyager.messaging)
A messaging thread.

| Field | Type | JSON Key | Required | Notes |
|-------|------|----------|----------|-------|
| entityUrn | Urn | `entityUrn` | no | `urn:li:messagingThread:...` |
| backendUrn | Urn | `backendUrn` | no | |
| participants | List\<MessagingProfile\> | `participants` | no | Union: member/company/bot |
| events | List\<Event\> | `events` | no | Messages in this conversation |
| read | boolean | `read` | no | |
| muted | boolean | `muted` | no | |
| archived | boolean | `archived` | no | |
| blocked | boolean | `blocked` | no | |
| withNonConnection | boolean | `withNonConnection` | no | |
| withNonConnectedCoworker | boolean | `withNonConnectedCoworker` | no | |
| unreadCount | int | `unreadCount` | no | |
| totalEventCount | int | `totalEventCount` | no | |
| name | String | `name` | no | Group chat name |
| pendingInvitation | Invitation | `pendingInvitation` | no | |
| receipts | List\<ParticipantReceipts\> | `receipts` | no | Read receipts |
| notificationStatus | NotificationStatus | `notificationStatus` | no | |
| contextText | TextViewModel | `contextText` | no | |
| shortContextText | TextViewModel | `shortContextText` | no | |
| messageRequestState | MessageRequestState | `messageRequestState` | no | |
| sponsoredConversationMetadata | SponsoredConversationMetadata | `sponsoredConversationMetadata` | no | |
| firstMessageUrn | Urn | `firstMessageUrn` | no | |

#### `Event` (voyager.messaging)
A single event within a conversation (message, participant change, etc.).

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| backendUrn | Urn | `backendUrn` | no |
| createdAt | long | `createdAt` | no |
| expiresAt | long | `expiresAt` | no |
| from | MessagingProfile | `from` | no |
| subtype | EventSubtype | `subtype` | no |
| eventContent | EventContent | `eventContent` | no |
| quickReplies | List\<AttributedText\> | `quickReplies` | no |
| quickReplyRecommendations | List\<QuickReply\> | `quickReplyRecommendations` | no |
| previousEventInConversation | Urn | `previousEventInConversation` | no |
| originToken | String | `originToken` | no |
| inlineWarning | TextViewModel | `inlineWarning` | no |
| reportSpamCTAText | String | `reportSpamCTAText` | no |
| inlineWarningDismissCTAText | String | `inlineWarningDismissCTAText` | no |
| obfuscatedMessageWarning | TextViewModel | `obfuscatedMessageWarning` | no |
| viewObfuscatedMessageCTAText | String | `viewObfuscatedMessageCTAText` | no |

#### `EventContent` (voyager.messaging.event) -- **UnionTemplate**

| Variant | Schema FQN | Type |
|---------|-----------|------|
| messageEvent | `com.linkedin.voyager.messaging.event.MessageEvent` | MessageEvent |
| participantChangeEvent | `com.linkedin.voyager.messaging.event.ParticipantChangeEvent` | ParticipantChangeEvent |
| stickerEvent | `com.linkedin.voyager.messaging.event.StickerEvent` | StickerEvent |
| genericMessageEvent | `com.linkedin.voyager.messaging.event.GenericMessageEvent` | GenericMessageEvent |

#### `MessageEvent` (voyager.messaging.event)
The actual message content.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| body | String | `body` | no |
| subject | String | `subject` | no |
| attachments | List\<File\> | `attachments` | no |
| customContent | CustomContent | `customContent` | no |
| shareContent | ShareContent | `shareContent` | no |
| attributedBody | AttributedText | `attributedBody` | no |
| mediaAttachments | List\<MediaMetadata\> | `mediaAttachments` | no |
| feedUpdate | UpdateV2 | `feedUpdate` | no |
| messageBodyRenderFormat | MessageBodyRenderFormat | `messageBodyRenderFormat` | no |

#### `MessagingProfile` (voyager.messaging) -- **UnionTemplate**
Participant in a conversation.

| Variant | Schema FQN | Type |
|---------|-----------|------|
| messagingMember | `com.linkedin.voyager.messaging.MessagingMember` | MessagingMember |
| messagingCompany | `com.linkedin.voyager.messaging.MessagingCompany` | MessagingCompany |
| messagingBot | `com.linkedin.voyager.messaging.MessagingBot` | MessagingBot |

#### `MessagingMember` (voyager.messaging)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| miniProfile | MiniProfile | `miniProfile` | no |
| alternateName | String | `alternateName` | no |
| alternateImage | Image | `alternateImage` | no |
| nameInitials | String | `nameInitials` | no |
| restrictedProfile | boolean | `restrictedProfile` | no |

#### Messaging Enums

**EventSubtype:**
`CAREER_ADVICE`, `CONVERSATION_UPDATE`, `GROUP_INVITATION`, `HIRER_TO_APPLICANT`, `INMAIL`, `INMAIL_REPLY`, `INVITATION_ACCEPT`, `JOB_REFERRAL`, `MEMBER_TO_GROUP_MEMBER`, `MEMBER_TO_MEMBER`, `PARTICIPANT_CHANGE`, `SHARING`, `SPONSORED_INMAIL`, `SPONSORED_MESSAGE`, `SPONSORED_MESSAGE_REPLY`, `SYSTEM_MESSAGE`, `$UNKNOWN`

**MessageRequestState:**
`ACCEPTED`, `DECLINED`, `PENDING`, `$UNKNOWN`

**NotificationStatus:**
`ACTIVE`, `MUTE`, `ACTIVE_FOR_MENTIONS_ONLY`, `$UNKNOWN`

**ConversationAction:**
`UPDATE`, `DELETE`, `$UNKNOWN`

**MailboxFolder:**
`INBOX`, `SENT`, `ARCHIVED`, `TRASH`, `$UNKNOWN`

---

### 3.4 Feed / Posts

#### `UpdateV2` (voyager.feed.render)
The primary feed item model (v2 rendering format).

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| updateMetadata | UpdateMetadata | `updateMetadata` | no |
| header | HeaderComponent | `header` | no |
| detailHeader | HeaderComponent | `detailHeader` | no |
| contextualHeader | ContextualHeaderComponent | `contextualHeader` | no |
| actor | ActorComponent | `actor` | no |
| contextualCommentary | ContextualCommentaryComponent | `contextualCommentary` | no |
| commentary | TextComponent | `commentary` | no |
| content | FeedComponent | `content` | no |
| relatedContent | UpdateAttachment | `relatedContent` | no |
| resharedUpdate | UpdateV2 | `resharedUpdate` | no |
| aggregatedContent | AggregatedContent | `aggregatedContent` | no |
| carouselContent | CarouselContent | `carouselContent` | no |
| leadGenFormContent | LeadGenFormContent | `leadGenFormContent` | no |
| socialDetail | SocialDetail | `socialDetail` | no |
| highlightedComments | List\<Comment\> | `highlightedComments` | no |
| footer | FeedComponent | `footer` | no |

Note: `resharedUpdate` is self-referential -- a reshare embeds the original UpdateV2.

#### `SocialDetail` (voyager.feed)
Social engagement metadata for a feed item.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| urn | Urn | `urn` | no |
| entityUrn | Urn | `entityUrn` | no |
| detailPageUpdateUrn | Urn | `detailPageUpdateUrn` | no |
| totalSocialActivityCounts | SocialActivityCounts | `totalSocialActivityCounts` | no |
| socialUpdateType | SocialUpdateType | `socialUpdateType` | no |
| reactionElements | List\<Reaction\> | `reactionElements` | no |
| likes | Likes | `likes` | no |
| comments | Comments | `comments` | no |
| commentingDisabled | boolean | `commentingDisabled` | no |
| threadId | String | `threadId` | no |
| quickComments | List\<AttributedText\> | `quickComments` | no |
| showShareButton | boolean | `showShareButton` | no |
| legacyGroupPost | boolean | `legacyGroupPost` | no |

#### `SocialActivityCounts` (voyager.feed.shared)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| urn | Urn | `urn` | no |
| socialDetailEntityUrn | Urn | `socialDetailEntityUrn` | no |
| numLikes | long | `numLikes` | no |
| numComments | long | `numComments` | no |
| numShares | long | `numShares` | no |
| numViews | long | `numViews` | no |
| liked | boolean | `liked` | no |
| reacted | ReactionType | `reacted` | no |
| reactionTypeCounts | List\<ReactionTypeCount\> | `reactionTypeCounts` | no |
| likedByOrganizationActor | boolean | `likedByOrganizationActor` | no |
| reactionByOrganizationActor | ReactionType | `reactionByOrganizationActor` | no |

#### `Comment` (voyager.feed)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| urn | Urn | `urn` | no |
| entityUrn | Urn | `entityUrn` | no |
| commenter | SocialActor | `commenter` | no |
| comment | AnnotatedText | `comment` | no |
| createdTime | long | `createdTime` | no |
| socialDetail | SocialDetail | `socialDetail` | no |
| threadId | String | `threadId` | no |
| index | int | `index` | no |
| insight | Insight | `insight` | no |
| edited | boolean | `edited` | no |
| canDelete | boolean | `canDelete` | no |
| actions | List\<CommentAction\> | `actions` | no |
| parentCommentUrn | Urn | `parentCommentUrn` | no |
| content | Content | `content` | no |
| permalink | String | `permalink` | no |
| trackingId | String | `trackingId` | no |
| groupMembership | GroupMembership | `groupMembership` | no |
| timeOffset | long | `timeOffset` | no |

#### Feed Enums

**ReactionType:**
`LIKE`, `PRAISE`, `INSPIRATION`, `MAYBE`, `EMPATHY`, `INTEREST`, `APPRECIATION`, `USEFULNESS`, `ENTERTAINMENT`, `CELEBRATION`, `ASSENT`, `$UNKNOWN`

**CommentAction:**
`DELETE`, `REPORT`, `SHARE_VIA`, `MESSAGE`, `REMOVE_MENTION`, `EDIT_COMMENT`, `REMOVE_GROUP_COMMENT`, `BLOCK_GROUP_MEMBER`, `REMOVE_GROUP_MEMBER`, `$UNKNOWN`

---

### 3.5 Jobs

#### `JobDescription` (voyager.entities.job)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| description | String | `description` | no |
| employmentStatus | String | `employmentStatus` | no |
| experience | String | `experience` | no |
| jobFunctions | List\<String\> | `jobFunctions` | no |
| industries | List\<String\> | `industries` | no |
| jobId | long | `jobId` | no |
| skillsAndExperience | String | `skillsAndExperience` | no |

#### `JobDetails` (voyager.entities.job)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| employmentStatus | String | `employmentStatus` | no |
| experience | String | `experience` | no |
| jobFunctions | List\<String\> | `jobFunctions` | no |
| industries | List\<String\> | `industries` | no |
| jobId | long | `jobId` | no |

#### `JobSeekerPreference` (voyager.jobs)

| Field | Type | JSON Key |
|-------|------|----------|
| (see source for full fields) | | |

Note: The actual JobPosting model does not appear as a standalone Pegasus class in the decompiled output. Job data appears to be fetched through `JobItem` wrapper objects and entity decoration, where the job posting fields are inlined via the `deco/jobs` decoration schemas rather than a single top-level model.

#### Jobs Enums

**JobState:**
`LISTED`, `CLOSED`, `SUSPENDED`, `DRAFT`, `$UNKNOWN`

**ApplicationFlow:**
`PREMIUM_SIMPLE_ONSITE`, `PREMIUM_COMPLEX_ONSITE`, `PREMIUM_OFFSITE`, `PREMIUM_INSTANT_APPLY`, `BASIC_OFFSITE`, `PRIVATE_SIMPLE_ONSITE`, `$UNKNOWN`

**JobSeekerStatus:**
`ACTIVE_SEEKING`, `CASUAL_SEEKING`, `OPEN`, `NOT_OPEN`, `$UNKNOWN`

---

### 3.6 Connections / Relationships

#### `Connection` (voyager.relationships.shared.connection)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| miniProfile | MiniProfile | `miniProfile` | no |
| phoneNumbers | List\<PhoneNumber\> | `phoneNumbers` | no |
| primaryEmailAddress | String | `primaryEmailAddress` | no |
| twitterHandles | List\<TwitterHandle\> | `twitterHandles` | no |
| weChatContactInfo | WeChatContactInfo | `weChatContactInfo` | no |
| createdAt | long | `createdAt` | no |

#### `Invitation` (voyager.relationships.invitation)

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| toMemberId | String | `toMemberId` | no |
| fromMemberId | String | `fromMemberId` | no |
| fromMember | MiniProfile | `fromMember` | no |
| toMember | MiniProfile | `toMember` | no |
| invitee | Invitee | `invitee` | no |
| message | String | `message` | no |
| sharedSecret | String | `sharedSecret` | no |
| sentTime | long | `sentTime` | no |
| invitationType | InvitationType | `invitationType` | no |
| customMessage | boolean | `customMessage` | no |
| mailboxItemId | String | `mailboxItemId` | no |
| unseen | boolean | `unseen` | no |
| inviterActors | List\<EntitiesMiniProfile\> | `inviterActors` | no |
| fromEvent | ProfessionalEvent | `fromEvent` | no |

`Invitee` is a **UnionTemplate** with variants: `EmailInvitee`, `PhoneInvitee`, `ProfileInvitee`.

#### Relationships Enums

**InvitationType:**
`UNKNOWN`, `PENDING`, `ARCHIVED`, `SENT`, `BLOCKED`, `ACCEPTED`, `NONE`, `$UNKNOWN`

---

### 3.7 Search

#### `SearchProfile` (voyager.search)
Search result for a person.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| id | String | `id` | no |
| backendUrn | Urn | `backendUrn` | no |
| miniProfile | MiniProfile | `miniProfile` | no |
| distance | MemberDistance | `distance` | no |
| location | String | `location` | no |
| industry | String | `industry` | no |
| educations | List\<Education\> | `educations` | no |
| followingInfo | FollowingInfo | `followingInfo` | no |
| memberBadges | MemberBadges | `memberBadges` | no |
| profileActions | ProfileActions | `profileActions` | no |
| sharedConnectionCount | int | (inferred) | no |
| sharedConnectionsInfo | SharedConnectionsInfo | (inferred) | no |
| numConnections | int | (inferred) | no |
| headless | boolean | (inferred) | no |
| snippets | (list) | (inferred) | no |

---

### 3.8 Notifications

#### `Card` (voyager.identity.notifications)
A notification card.

| Field | Type | JSON Key | Required |
|-------|------|----------|----------|
| entityUrn | Urn | `entityUrn` | no |
| headline | TextViewModel | `headline` | no |
| subHeadline | TextViewModel | `subHeadline` | no |
| kicker | TextViewModel | `kicker` | no |
| headerImage | ImageViewModel | `headerImage` | no |
| badgeIcon | ImageViewModel | `badgeIcon` | no |
| contentType | CardContentType | `contentType` | no |
| contentPrimaryText | List\<TextViewModel\> | `contentPrimaryText` | no |
| contentSecondaryText | List\<TextViewModel\> | `contentSecondaryText` | no |
| contentImages | List\<ImageViewModel\> | `contentImages` | no |
| contentImagesTotalCount | int | `contentImagesTotalCount` | no |
| actions | List\<CardAction\> | `actions` | no |
| cardAction | CardAction | `cardAction` | no |
| contentAction | CardAction | `contentAction` | no |
| insightAction | CardAction | `insightAction` | no |
| insight | TextViewModel | `insight` | no |
| insightType | InsightType | `insightType` | no |
| publishedAt | long | `publishedAt` | no |
| read | boolean | `read` | no |
| trackingObject | TrackingObject | `trackingObject` | no |
| socialActivityCounts | SocialActivityCounts | `socialActivityCounts` | no |
| setting | NotificationSetting | (inferred) | no |
| settingOptionData | SettingOptionData | (inferred) | no |
| valuePropositionAnnotation | (unknown) | (inferred) | no |

---

## 4. Type Mapping: Java to Rust

| Java Type | Rust Type | Serde Strategy |
|-----------|-----------|----------------|
| `String` | `String` | Default |
| `long` | `i64` | Default |
| `int` | `i32` | Default |
| `boolean` | `bool` | Default |
| `Urn` | `String` (newtype `Urn(String)`) | `#[serde(transparent)]` |
| `List<T>` | `Vec<T>` | Default; empty vec if absent |
| `RecordTemplate` | `struct` with `#[derive(Deserialize)]` | `#[serde(rename_all = "camelCase")]` |
| `UnionTemplate` | `enum` | See union encoding below |
| Pegasus enum | `enum` | `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` with `#[serde(other)]` on `Unknown` |
| Optional field | `Option<T>` | `#[serde(default)]` |
| Required field | `T` (bare) | No annotation needed |

### Union Encoding in JSON

Rest.li unions are encoded as a JSON object with exactly one key, where the key is the schema FQN:

```json
{
  "com.linkedin.voyager.messaging.MessagingMember": {
    "entityUrn": "urn:li:...",
    "miniProfile": { ... }
  }
}
```

In Rust, this maps to a tagged enum. The serde approach depends on whether the union is internally tagged. For Rest.li, use a custom deserializer or `#[serde(untagged)]` with careful variant ordering.

---

## 5. Cross-Cutting Patterns

### 5.1 Model Relationships Graph

```
Me
  -> MiniProfile (required)
  -> PublicContactInfo (required)

MiniProfile
  -> Image (picture, backgroundImage)
  -> Urn (entityUrn, objectUrn)

Profile
  -> MiniProfile
  -> Image (multiple)
  -> Date, Locale, DateRange

Position -> DateRange, Urn (company)
Education -> DateRange, MiniSchool, Urn (school)

Conversation
  -> List<MessagingProfile> (union: Member/Company/Bot)
  -> List<Event>

Event
  -> MessagingProfile (from)
  -> EventContent (union: Message/ParticipantChange/Sticker/Generic)

MessageEvent
  -> List<File> (attachments)
  -> AttributedText (rich body)
  -> UpdateV2 (embedded feed update)

UpdateV2
  -> SocialDetail
  -> UpdateV2 (resharedUpdate -- recursive)
  -> List<Comment>

Comment -> SocialDetail (recursive social engagement)

Connection -> MiniProfile
Invitation -> MiniProfile (from, to)

SearchProfile -> MiniProfile
```

### 5.2 Recurring Entity Reference Pattern

Most entities follow this pattern:
1. `entityUrn` -- the primary key (e.g., `urn:li:fs_miniProfile:...`)
2. `objectUrn` -- optional, the "real" entity URN (e.g., `urn:li:member:...`)
3. `trackingId` -- base64 tracking token for analytics
4. `backendUrn` -- alternative backend identifier

The `entityUrn` is used as the cache key (returned by `id()` method).

### 5.3 Union Types That Need Custom Serde

1. **Image** -- 4 variants (MediaProcessorImage, MediaProxyImage, String URL, VectorImage)
2. **EventContent** -- 4 variants (MessageEvent, ParticipantChangeEvent, StickerEvent, GenericMessageEvent)
3. **MessagingProfile** -- 3 variants (MessagingMember, MessagingCompany, MessagingBot)
4. **Invitee** -- 3 variants (EmailInvitee, PhoneInvitee, ProfileInvitee)
5. **Comment.Content** -- union of content types within comments
6. **SocialActor** -- union of actor types in feed

---

## 6. Priority for Rust Implementation

### Tier 1: Must Have (used across all features)
- `Urn` (newtype)
- `Date`, `DateRange`
- `Image` (union), `VectorImage`, `VectorArtifact`
- `CollectionMetadata` (pagination)
- `MiniProfile`
- `Me`
- `GraphDistance` (enum)

### Tier 2: Domain Core (one per feature)
- `Profile`, `Position`, `Education` (identity)
- `Conversation`, `Event`, `EventContent`, `MessageEvent`, `MessagingMember` (messaging)
- `UpdateV2`, `SocialDetail`, `SocialActivityCounts`, `Comment` (feed)
- `Connection` (connections)
- `Invitation` (invitations)
- `Card` (notifications)
- `SearchProfile` (search)

### Tier 3: Supporting Types
- `JobDescription`, `JobDetails` (jobs)
- All enums: `ReactionType`, `EventSubtype`, `MessageRequestState`, etc.
- `AttributedText`, `TextViewModel`, `ImageViewModel`
- `PublicContactInfo`, `TwitterHandle`
- `MessagingCompany`, `MessagingBot`
- `SponsoredConversationMetadata`, `ParticipantReceipts`

### Tier 4: Defer (complex feed rendering)
- Feed component types: `ActorComponent`, `HeaderComponent`, `FeedComponent`, etc.
- Organization models
- Premium/Learning/Growth models
- Decoration schemas (`deco/` package)
