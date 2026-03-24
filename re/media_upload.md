# Media Upload Protocol

Reverse-engineered from `com.linkedin.android` APK. Analysis date: 2026-03-24.

Source packages examined:
- `com.linkedin.android.publishing.shared.mediaupload` (VectorMediaUploader, VectorUploadManager, VectorService, ImageUploader)
- `com.linkedin.android.publishing.shared.preprocessing` (MediaPreprocessor, ImagePreprocessingTask, VideoPreprocessingConfigurator)
- `com.linkedin.android.pegasus.gen.mediauploader` (PartUploadRequest, MediaUploadMetadata, CompleteMultipartUploadRequest, PartUploadResponse)
- `com.linkedin.android.networking.filetransfer.api.request` (UploadRequest)
- `com.linkedin.android.infra.mediaupload` (MediaUploader -- legacy path)

---

## 1. Upload Endpoint URL Patterns

There are two distinct upload systems in the app:

### 1.1 Vector Upload System (Primary -- Modern)

The primary upload system is called "Vector". It uses a two-phase protocol: metadata request followed by data upload to signed URLs.

**Phase 1: Obtain upload metadata (signed URLs)**

```
POST {baseUrl}/zephyr/api/voyagerMediaUploadMetadata?action=upload
```

Route definition from `Routes.java`:
```java
MEDIA_UPLOAD_METADATA("voyagerMediaUploadMetadata")
```

The full URL is constructed as:
```
/zephyr/api/voyagerMediaUploadMetadata?action=upload
```

**Phase 2: Upload data to signed URL(s)**

The response from Phase 1 returns pre-signed upload URLs. The actual file data is PUT directly to these URLs (not to the LinkedIn API). The URLs are opaque -- they point to LinkedIn's internal storage backend (likely Amazon S3 or LinkedIn's Ambry blob store).

**Phase 3: Complete multipart upload (multipart only)**

```
POST {baseUrl}/zephyr/api/voyagerMediaUploadMetadata?action=completeMultipartUpload
```

### 1.2 SlideShare Upload System (Legacy)

Used for document/file uploads (PDFs, presentations):

```
POST https://slideshare.www.linkedin.com/upload    (production)
POST https://slideshare.www.linkedin-ei.com/upload  (staging)
```

This is a `multipart/form-data` upload with the actual file content embedded in the request body.

### 1.3 MUPLD Endpoints (Messaging Attachments)

From `Routes.java`:
```
MUPLD            -> mupld/upload
MUPLD_ATTACHMENT -> mupld/attachment
MUPLD_JOB_APPLICATION_RESUME -> mupld/cappts
```

These appear to be used for messaging file attachments and job application resumes.

---

## 2. Request Format

### 2.1 Phase 1: Metadata Request

**Method**: POST
**Content-Type**: application/json (via DataManager/Rest.li framework)
**URL**: `/zephyr/api/voyagerMediaUploadMetadata?action=upload`

**Request body** (JSON):
```json
{
  "mediaUploadType": "IMAGE_SHARING",
  "fileSize": 1234567,
  "hasOverlayImage": false,
  "uploadMetadataType": "MULTIPART",
  "filename": "photo.jpg",
  "organizationActor": "urn:li:organization:12345"
}
```

Fields:
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mediaUploadType` | string | Yes | One of the `MediaUploadType` enum values (see Section 6) |
| `fileSize` | long | Yes | File size in bytes. For `PARTIAL_MULTIPART`, inflated by 25% with minimum of 52,428,801 bytes |
| `hasOverlayImage` | boolean | Yes | Whether an overlay image (e.g., video thumbnail) is included |
| `uploadMetadataType` | string | No | `SINGLE`, `MULTIPART`, or `PARTIAL_MULTIPART` |
| `filename` | string | No | Original filename |
| `organizationActor` | string | No | Organization URN for company page posts |

**Response** (`MediaUploadMetadata`):
```json
{
  "urn": "urn:li:digitalmediaAsset:...",
  "mediaArtifactUrn": "urn:li:digitalmediaMediaArtifact:...",
  "type": "SINGLE",
  "singleUploadUrl": "https://...",
  "singleUploadHeaders": {"header1": "value1"},
  "partUploadRequests": [],
  "multipartMetadata": "...",
  "overlayImageUploadUrl": "https://...",
  "overlayImageUploadHeaders": {"header1": "value1"},
  "recipes": ["urn:li:digitalmediaRecipe:..."]
}
```

The response `type` determines the upload strategy:
- `SINGLE` -- one upload URL, entire file in one request
- `MULTIPART` -- multiple part upload URLs with byte ranges
- `PARTIAL_MULTIPART` -- partial multipart (same as MULTIPART but file size was inflated at request time)

### 2.2 Phase 2: Data Upload (Single Part)

**Method**: PUT (request method 2)
**Content-Type**: `application/octet-stream`
**URL**: The `singleUploadUrl` from the metadata response
**Headers**: All headers from `singleUploadHeaders` map, plus server-provided headers from the metadata response
**Body**: Raw file bytes

Key code from `VectorMediaUploader.uploadSinglePart()`:
```java
new UploadRequest.Builder()
    .setRequestMethod(2)  // PUT
    .setContentType("application/octet-stream")
    .setRequestTag("vector")
    .setUploadPath(Uri.parse(singleUploadUrl))
    .setLocalFile(uri)
    .setRetries(retryCount)
    // + headers from singleUploadHeaders map
```

### 2.3 Phase 2: Data Upload (Multipart)

For multipart uploads, each part has its own upload URL and byte range.

**Method**: PUT (request method 2)
**Content-Type**: Not set per-part (raw binary)
**URL**: Per-part `uploadUrl` from `PartUploadRequest`
**Headers**: Per-part headers from `PartUploadRequest.headers`
**Body**: File bytes from `firstByte` to `lastByte` (inclusive)

Each `PartUploadRequest` contains:
```json
{
  "uploadUrl": "https://...",
  "firstByte": 0,
  "lastByte": 5242879,
  "minPartSize": 5242880,
  "maxPartSize": 20971520,
  "headers": {"header1": "value1"},
  "urlExpiresAt": 1711234567890
}
```

The multipart builder creates individual `UploadRequest` objects per part, each pointing to its own URL with the appropriate byte range.

### 2.4 Phase 3: Complete Multipart Upload

**Method**: POST
**URL**: `/zephyr/api/voyagerMediaUploadMetadata?action=completeMultipartUpload`

**Request body** (JSON):
```json
{
  "completeUploadRequest": {
    "uploadMetadataType": "MULTIPART",
    "multipartMetadata": "...",
    "mediaArtifactUrn": "urn:li:digitalmediaMediaArtifact:...",
    "partUploadResponses": [
      {
        "httpStatusCode": 200,
        "headers": {"ETag": "\"abc123\"", ...},
        "body": "..."
      }
    ]
  }
}
```

The `partUploadResponses` are constructed from the HTTP responses of each individual part upload. Only responses with status 200 and non-empty headers are included.

### 2.5 Overlay Image Upload

For video posts with overlay/thumbnail images, a separate upload is made:

**Method**: PUT
**Content-Type**: `application/octet-stream`
**URL**: `overlayImageUploadUrl` from the metadata response
**Headers**: `overlayImageUploadHeaders` from the metadata response
**Tag**: `"vector_overlay"` (distinct from main `"vector"` tag)

Overlay uploads are always treated as SINGLE type regardless of the main upload type.

### 2.6 SlideShare Upload (Legacy)

**Method**: POST
**Content-Type**: `multipart/form-data`

Form parts:
| Part Name | Content | Notes |
|-----------|---------|-------|
| `file` | Binary file data | `Content-Disposition: form-data; name="file"; filename="name.ext"` |
| `csrfToken` | JSESSIONID value | CSRF protection |
| `store_securely` | `"true"` | Security flag |

---

## 3. How Upload URLs Are Obtained

Upload URLs are **not hardcoded** -- they are obtained dynamically from the API.

### Flow:

1. Client calls `POST /zephyr/api/voyagerMediaUploadMetadata?action=upload` with file metadata
2. Server responds with `MediaUploadMetadata` containing:
   - For SINGLE: `singleUploadUrl` + `singleUploadHeaders`
   - For MULTIPART: `partUploadRequests[]` where each entry has `uploadUrl`, byte range, `headers`, and `urlExpiresAt`
   - For overlay: `overlayImageUploadUrl` + `overlayImageUploadHeaders`
3. URLs have expiration times (`urlExpiresAt` field on part requests)
4. The response also includes `urn` (the media asset URN) and `recipes` (processing pipeline identifiers)

This is a **pre-signed URL pattern** similar to AWS S3 presigned uploads. LinkedIn's backend (likely Ambry blob store, given the `AMBRY_UPLOAD_URLS` route) generates time-limited, authenticated URLs. The client uploads directly to storage without proxying through the API layer.

---

## 4. Chunk/Resume Support

### 4.1 Three Upload Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `SINGLE` | Entire file in one PUT request | Small files (images, small videos) |
| `MULTIPART` | File split into byte-range chunks, each uploaded to a separate URL | Large files (videos, documents) |
| `PARTIAL_MULTIPART` | Same as MULTIPART but file size reported as `max(fileSize * 1.25, 52428801)` | Videos that may grow during transcoding |

### 4.2 Chunk Structure

Each chunk is defined by:
- `firstByte` / `lastByte` -- byte range (inclusive, 0-indexed)
- `minPartSize` / `maxPartSize` -- size constraints
- `uploadUrl` -- unique signed URL per chunk
- `headers` -- per-chunk required headers
- `urlExpiresAt` -- URL expiration timestamp

### 4.3 Retry Support

- Default retry count: **10** (from `UploadRequest.Builder.retries = 10`)
- Configurable via `fileUploadRetryCount` parameter
- Retry events tracked with `VectorUploadProgressEvent(indeterminate=true)`
- On retry, notification shows indeterminate progress

### 4.4 Resume Support

There is **no explicit resume-after-disconnect support** at the protocol level. The system tracks upload state per-request:

- States: pending (0), in-progress (1), retrying (2), completed, failed
- `VectorUploadManager` maintains `uploadsByUploadIds` and `requestToUploadIds` maps
- `VectorService` re-subscribes to incomplete uploads on service restart
- Failed parts set `bytesCompleted = 0` and `failed = true`

However, for multipart uploads, individual failed parts could theoretically be retried independently since each part has its own URL. The `VectorService` does reconnect to ongoing uploads via `subscribeForRequests()` -> `getIncompleteUploadRequestIds()`.

### 4.5 Wakelock

`VectorService` acquires a wakelock with a 2-minute timeout (`WAKELOCK_TIMEOUT = TimeUnit.MINUTES.toMillis(2)`) to prevent the device from sleeping during uploads.

---

## 5. Media Preprocessing Pipeline

### 5.1 Image Preprocessing

**Class**: `ImagePreprocessingTask` (runs on executor service thread pool)

Pipeline:
1. **EXIF rotation correction** -- reads EXIF orientation tag and applies rotation matrix
2. **Downscaling** -- applies if both dimensions > 1280px:
   - Scale factor: `1280 / min(width, height)`
   - Maximum output: 7680 x 4320 (8K UHD)
   - If width * scale > 7680: `scale = 7680 / width`
   - If height * scale > 4320: `scale = 4320 / height`
3. **Re-encoding** -- saves to temp file in cache directory (`preprocessed/images/`)
   - Supported formats: JPEG, PNG, WebP
   - JPEG quality: **100** (no quality loss from re-encoding)
   - Unsupported mime types are skipped (no preprocessing)
4. Result URI returned to caller (either original or preprocessed temp file)

**Note**: Preprocessing occurs BEFORE upload. The `ImageUploader` orchestrates: preprocess -> then start vector upload on preprocessed URI.

### 5.2 Video Preprocessing

**Class**: `VideoPreprocessingConfigurator`, `MediaPreprocessor`

**Important finding**: In the Zephyr (China) variant, video transcoding is a **no-op**:
```java
public String beginVideoTranscoding(...) {
    CrashReporter.reportNonFatal(new Exception("no op in zephyr"));
    return null;
}
```

The international (Voyager) variant likely performs actual transcoding via a native library not present in the Zephyr build.

Configuration parameters (from `VideoPreprocessingConfigurator`):
- **Target bitrate**: 5,000,000 bps (5 Mbps)
- **Target resolution**: 720p
- **Skip transcoding if**: either dimension <= 720px, OR bitrate <= 5 Mbps

Skip conditions return codes: -1 (resolution OK), -2 (bitrate OK), -3 (metadata unavailable), 0 (needs transcoding).

### 5.3 Preprocessing Service

`MediaPreprocessorService` runs as a foreground service with:
- Wakelock to prevent sleep during transcoding
- Notification channel: `PostCreationProgressChannel`
- Progress updates via EventBus

---

## 6. Upload Metadata Sent With Requests

### 6.1 MediaUploadType Enum

All supported media types (sent as `mediaUploadType` in the metadata request):

| Type | Description |
|------|-------------|
| `IMAGE_SHARING` | Image in a post |
| `VIDEO_SHARING` | Video in a post |
| `VIDEO_SHARING_WITH_CAPTION` | Video with captions |
| `VIDEO_MESSAGING` | Video in messages |
| `VIDEO_CAPTION` | Caption/subtitle file |
| `VIDEO_THUMBNAIL` | Video thumbnail |
| `MESSAGING_PHOTO_ATTACHMENT` | Image in messages |
| `MESSAGING_FILE_ATTACHMENT` | File in messages |
| `DOCUMENT_SHARING` | Document in a post |
| `VOICE_MESSAGE` | Voice message |
| `PROFILE_ORIGINAL_PHOTO` | Profile photo (original) |
| `PROFILE_DISPLAY_PHOTO` | Profile photo (display size) |
| `PROFILE_ORIGINAL_BACKGROUND` | Profile background (original) |
| `PROFILE_DISPLAY_BACKGROUND` | Profile background (display size) |
| `PUBLISHING_COVER_IMAGE` | Article cover image |
| `PUBLISHING_INLINE_IMAGE` | Article inline image |
| `PUBLISHING_SERIES_LOGO` | Newsletter series logo |
| `COMPANY_LOGO` | Company page logo |
| `COMPANY_BACKGROUND` | Company page background |
| `COMPANY_OVERVIEW` | Company overview image |
| `GROUP_LOGO` | Group logo |
| `GROUP_HERO_IMAGE` | Group hero image |
| `CAREER_FEATURED_IMAGE` | Career page featured image |
| `CAREER_ADDITIONAL_IMAGE` | Career page additional image |
| `CAREER_COMPANY_PHOTO` | Career page company photo |
| `CAREER_VIDEO` | Career page video |
| `COMMENT_IMAGE` | Image in comments |
| `EVENT_LOGO` | Event logo |
| `EVENT_BACKGROUND` | Event background |
| `PROFILE_TREASURY_IMAGE` | Featured section image |
| `PROFILE_TREASURY_DOCUMENT` | Featured section document |
| `ZEPHYR_CONTENT_IMAGE` | Zephyr-specific content image |
| `ZEPHYR_BIZCARD_IMAGE` | Zephyr business card image |
| `COMPANY_PIPELINE_BUILDER_BACKGROUND_IMAGE` | Pipeline builder background |

### 6.2 File Transfer Metadata

The `VectorFileTransferMetadata` object is serialized as JSON and attached to each `UploadRequest.metadata` field:

```json
{
  "uri": "content://...",
  "uploadTrackingId": "tracking-id-string",
  "uploadMethod": "SINGLE",
  "mediaUploadType": "IMAGE_SHARING",
  "mediaArtifactUrn": "urn:li:digitalmediaMediaArtifact:...",
  "startTime": 1711234567890,
  "size": 1234567,
  "multipartMetadata": "..."
}
```

This metadata travels with the upload request through the file transfer system for tracking and finalization purposes.

### 6.3 Tracking Headers

Upload requests can carry custom tracking headers (`Map<String, String>`) passed through from the calling code. These are serialized via `TrackingUtils.toTrackingBundle()` when passing through Android Intent extras.

---

## 7. Architecture Summary

```
ImageUploader / VideoUploader (domain layer)
  |
  v
MediaPreprocessor (preprocessing: resize, rotate, transcode)
  |
  v
VectorMediaUploader (orchestration -- implements VectorUploader + VectorManagedUploader)
  |
  |-- Phase 1: POST voyagerMediaUploadMetadata?action=upload
  |     -> receives MediaUploadMetadata (signed URLs)
  |
  |-- Phase 2: Submit to UploadManager (file transfer system)
  |     -> PUT to signed URL(s) with application/octet-stream
  |     -> VectorService (foreground service with wakelock) manages lifecycle
  |     -> EventBus for progress/success/failure events
  |
  |-- Phase 3 (multipart only): POST voyagerMediaUploadMetadata?action=completeMultipartUpload
  |     -> sends part responses (status codes, headers, bodies)
  |
  v
VectorUploadManager (state tracking -- upload IDs, request IDs, part responses)
```

### Key Design Decisions

1. **Signed URL pattern**: The API server generates pre-signed URLs; clients upload directly to storage. This offloads bandwidth from the API servers.
2. **Two-phase protocol**: Metadata first (to get upload instructions), then data upload. This allows the server to control chunk sizes, routing, and URL expiry.
3. **Foreground service**: Uploads run in a foreground Android service with wakelock to survive background killings.
4. **EventBus-driven**: All upload events (progress, success, failure, retry) propagate via GreenRobot EventBus.
5. **Upload tagging**: Requests tagged as `"vector"` (main) or `"vector_overlay"` (thumbnail) for lifecycle tracking.

---

## 8. Open Questions / Limitations

1. **Signed URL format**: The actual URLs are opaque from the decompiled code. Live traffic capture would reveal the storage backend (S3, Ambry, or other).
2. **Video transcoding in Voyager**: The international variant likely has a native transcoding library not present in this Zephyr build. The transcoding parameters (720p, 5Mbps) are configured but the implementation is stubbed.
3. **PARTIAL_MULTIPART behavior**: The 25% file size inflation and 52MB minimum suggest this mode is for streaming/progressive uploads where the final file size is not yet known (e.g., during live transcoding).
4. **URL expiration**: Part URLs have `urlExpiresAt` timestamps, but there is no observed refresh mechanism for expired URLs -- the upload would need to restart from Phase 1.
5. **No true resumability**: While the system tracks per-part state, there is no protocol-level support for resuming a partially uploaded part. Only whole-part retries are supported.
