//! Feed-related API methods on `LinkedInClient`.
//!
//! Covers: feed listing, single-post lookup, my-posts, reactions
//! (read/create/delete), comments (read/create), and post creation.

use serde_json::Value;

use crate::error::Error;
use crate::urn::{ActivityUrn, SocialDetailUrn};

use super::internal::{
    check_graphql_errors, check_response, graphql_params, normalize_social_thread_urn,
    restli_encode_string, unwrap_graphql,
};
use super::{LinkedInClient, API_PREFIX, BASE_URL};

/// Valid reaction type strings accepted by the LinkedIn API.
///
/// Extracted from `ReactionType.java` in the decompiled international APK
/// (`com.linkedin.android.pegasus.dash.gen.voyager.dash.feed.social`).
const VALID_REACTION_TYPES: &[&str] = &[
    "LIKE",
    "PRAISE",
    "EMPATHY",
    "INTEREST",
    "APPRECIATION",
    "ENTERTAINMENT",
    "CELEBRATION",
];

impl LinkedInClient {
    /// Fetch the user's feed (`/voyager/api/feed/updates?q=findFeed`).
    pub async fn get_feed(&self, start: u32, count: u32) -> Result<Value, Error> {
        let path = format!("feed/updates?q=findFeed&start={}&count={}", start, count);
        self.get(&path).await
    }

    /// Locate a feed update by activity URN inside the current top-of-feed
    /// window. Falls back to the highlighted-feed finder, then 404s with a
    /// devtools-capture hint.
    pub async fn get_post(&self, activity_urn: &ActivityUrn) -> Result<Value, Error> {
        let activity_id = activity_urn.activity_id();
        let urn = activity_urn.as_str();

        if let Some(found) = self.find_post_in_feed(activity_id).await? {
            return Ok(found);
        }
        if let Some(found) = self.find_post_via_highlighted(urn, activity_id).await {
            return Ok(found);
        }
        Err(Error::Api {
            status: 404,
            body: format!(
                "post {} not in the top-50 feed window and the highlightedFeed \
                 fallback returned no match. The permalink endpoint shape is \
                 unverified; capture the request from \
                 https://www.linkedin.com/feed/update/{} in browser devtools \
                 and update get_post() with the captured path.",
                urn, urn
            ),
            correlation_id: None,
        })
    }

    async fn find_post_in_feed(&self, activity_id: &str) -> Result<Option<Value>, Error> {
        let feed = self.get("feed/updates?q=findFeed&start=0&count=50").await?;
        let Some(elements) = feed.get("elements").and_then(|e| e.as_array()) else {
            return Ok(None);
        };
        Ok(elements
            .iter()
            .find(|el| element_matches_activity_id(el, activity_id))
            .cloned())
    }

    /// Try the `highlightedFeed` finder for each common update type. Returns
    /// the first matching element. Errors and non-matching responses are
    /// silently skipped — the caller turns "no match anywhere" into a 404.
    async fn find_post_via_highlighted(&self, urn: &str, activity_id: &str) -> Option<Value> {
        let urn_encoded = urn.replace(':', "%3A");
        for update_type in ["SHARE", "ARTICLE", "VIDEO", "IMAGE"] {
            let path = format!(
                "feed/updates?q=highlightedFeed\
                 &highlightedUpdateUrns=List({})\
                 &highlightedUpdateTypes=List({})",
                urn_encoded, update_type
            );
            let Ok(resp) = self.get(&path).await else {
                continue;
            };
            let Some(elements) = resp.get("elements").and_then(|e| e.as_array()) else {
                continue;
            };
            if let Some(found) = elements
                .iter()
                .find(|el| element_matches_activity_id(el, activity_id))
            {
                return Some(found.clone());
            }
        }
        None
    }

    /// Fetch comments on a post by its socialDetail URN.
    pub async fn get_comments(
        &self,
        social_detail_urn: &SocialDetailUrn,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        let encoded_urn = restli_encode_string(social_detail_urn.as_str());
        let variables = format!(
            "(count:{count},socialDetailUrn:{encoded_urn},sortOrder:RELEVANCE,start:{start})"
        );
        let params = graphql_params(
            &variables,
            "voyagerSocialDashComments.59bca422f480a4cc0ce56ccd81181488",
            "SocialDashCommentsBySocialDetail",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "socialDashCommentsBySocialDetail")
    }

    /// Fetch replies for a parent comment URN.
    ///
    /// LinkedIn's `ByRepliesByCursor` finder requires a non-null cursor.
    /// Extract the parent comment's reply cursor from `get_comments --json`.
    pub async fn get_comment_replies(
        &self,
        comment_urn: &str,
        count: u32,
        cursor: &str,
    ) -> Result<Value, Error> {
        let cursor = if cursor.is_empty() {
            return Err(Error::InvalidInput(
                "get_comment_replies requires a non-empty cursor from the parent comment".into(),
            ));
        } else {
            cursor
        };
        let encoded_urn = restli_encode_string(comment_urn);
        let encoded_cursor = restli_encode_string(cursor);
        let variables = format!("(commentUrn:{encoded_urn},count:{count},cursor:{encoded_cursor})");
        let params = graphql_params(
            &variables,
            "voyagerSocialDashComments.8ada653d14b465e4f86d3ed7dcbe6695",
            "SocialDashCommentsByRepliesByCursor",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "socialDashCommentsByRepliesByCursor")
    }

    /// Fetch a single comment by comment URN and update/thread URN.
    pub async fn get_single_comment(
        &self,
        comment_urn: &str,
        update_thread_urn: &str,
    ) -> Result<Value, Error> {
        let encoded_comment = restli_encode_string(comment_urn);
        let encoded_thread = restli_encode_string(update_thread_urn);
        let variables = format!("(commentUrn:{encoded_comment},updateThreadUrn:{encoded_thread})");
        let params = graphql_params(
            &variables,
            "voyagerSocialDashComments.a84e91d6baaa2d2018fdc49f21541de5",
            "SocialDashCommentsBySingleComment",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "socialDashCommentsBySingleComment")
    }

    /// Fetch the authenticated user's own posts with engagement metrics.
    pub async fn get_my_posts(&self, start: u32, count: u32) -> Result<Value, Error> {
        let profile_urn = self.my_profile_urn().await?;
        let encoded_urn = restli_encode_string(profile_urn);
        let path = format!(
            "identity/profileUpdatesV2?q=memberShareFeed&profileUrn={}&moduleKey=member-shares%3Aphone&start={}&count={}",
            encoded_urn, start, count
        );
        self.get(&path).await
    }

    /// Fetch the list of reactors for a specific post. Auto-paginates in
    /// batches of 10 (LinkedIn's per-page cap on this endpoint).
    pub async fn get_post_reactions(
        &self,
        activity_urn: &ActivityUrn,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        let encoded_urn = restli_encode_string(activity_urn.as_str());
        let page_size = 10u32;
        let mut all_elements = Vec::new();
        let mut current_start = start;
        let mut total: Option<u64> = None;

        loop {
            let batch = std::cmp::min(page_size, count - all_elements.len() as u32);
            if batch == 0 {
                break;
            }
            let (elements, page_total) = self
                .fetch_reactions_page(&encoded_urn, current_start, batch)
                .await?;
            total = total.or(page_total);

            if elements.is_empty() {
                break;
            }
            all_elements.extend(elements);
            current_start += batch;

            if total.is_some_and(|t| current_start as u64 >= t) {
                break;
            }
        }

        Ok(serde_json::json!({
            "elements": all_elements,
            "paging": {
                "start": start,
                "count": all_elements.len(),
                "total": total.unwrap_or(all_elements.len() as u64)
            }
        }))
    }

    /// Fetch one page of reactors. Returns the page's elements and the
    /// `paging.total` field if present. Pure orchestration glue around
    /// the GraphQL endpoint — does not mutate caller state.
    async fn fetch_reactions_page(
        &self,
        encoded_urn: &str,
        start: u32,
        count: u32,
    ) -> Result<(Vec<Value>, Option<u64>), Error> {
        let params = format!(
            "variables=(count:{},start:{},threadUrn:{})&queryId=voyagerSocialDashReactions.41ebf31a9f4c4a84e35a49d5abc9010b",
            count, start, encoded_urn
        );
        let result = self.graphql_get(&params).await?;
        let page = unwrap_graphql(&result, "socialDashReactionsByReactionType")?;

        let total = page
            .get("paging")
            .and_then(|p| p.get("total"))
            .and_then(|t| t.as_u64());
        let elements = page
            .get("elements")
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();
        Ok((elements, total))
    }

    /// React to a post or activity with a specific reaction type.
    pub async fn react_to_post(
        &self,
        thread_urn: &ActivityUrn,
        reaction_type: &str,
    ) -> Result<Value, Error> {
        let rt = validate_reaction_type(reaction_type)?;
        let thread = thread_urn.as_str();

        // INTENTIONAL DUPLICATION: threadUrn and reactionType appear both at
        // the top level AND inside `entity`. This matches the decompiled
        // FeedFrameworkGraphQLClient.java mutation builder. Removing either
        // level causes the server to reject the request.
        let variables = serde_json::json!({
            "threadUrn": thread,
            "reactionType": rt,
            "entity": {
                "threadUrn": thread,
                "reactionType": rt,
            }
        });
        self.graphql_post(
            &variables,
            "voyagerSocialDashReactions.fd68eadaf15da416b0d839e21399b763",
            "CreateSocialDashReactions",
        )
        .await
    }

    /// Remove a reaction from a post or activity.
    pub async fn unreact_from_post(
        &self,
        thread_urn: &ActivityUrn,
        reaction_type: &str,
    ) -> Result<Value, Error> {
        let rt = validate_reaction_type(reaction_type)?;
        let thread = thread_urn.as_str();

        let variables = serde_json::json!({
            "threadUrn": thread,
            "reactionType": rt,
        });
        self.graphql_post(
            &variables,
            "voyagerSocialDashReactions.315cef4773de8e3a0ddad7655cc1685f",
            "DoDeleteReactionSocialDashReactions",
        )
        .await
    }

    /// Comment on a feed post.
    pub async fn comment_on_post(
        &self,
        post_urn: &ActivityUrn,
        text: &str,
    ) -> Result<Value, Error> {
        let thread = post_urn.as_str();
        let variables = serde_json::json!({
            "entity": {
                "commentary": { "text": text },
                "threadUrn": thread,
                "origin": "FEED"
            }
        });
        self.graphql_post(
            &variables,
            "voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e",
            "CreateSocialDashNormComments",
        )
        .await
    }

    /// Reply to a feed comment by creating a nested comment under the parent.
    ///
    /// Uses the same `CreateSocialDashNormComments` mutation as top-level
    /// comments but sets `threadUrn` to the parent comment URN (after
    /// normalizing from Dash `fsd_comment` to backend `comment` format).
    pub async fn reply_to_comment(&self, comment_urn: &str, text: &str) -> Result<Value, Error> {
        let thread = normalize_social_thread_urn(comment_urn);
        let variables = serde_json::json!({
            "entity": {
                "commentary": { "text": text },
                "threadUrn": thread,
                "origin": "FEED"
            }
        });
        self.graphql_post(
            &variables,
            "voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e",
            "CreateSocialDashNormComments",
        )
        .await
    }

    /// Create a new text-only post on the authenticated user's feed.
    ///
    /// Uses the web-style mutation format: `variables` + `queryId` are sent
    /// in the POST body, not just URL params. The server uses both, but the
    /// body shape is required for the modern endpoint.
    pub async fn create_post(&self, text: &str, visibility: &str) -> Result<Value, Error> {
        let body = build_share_body(text, visibility, "PUBLISHED", None, None)?;
        self.execute_share_mutation(&body).await
    }

    /// Schedule a text-only post for future publication.
    ///
    /// `scheduled_at_ms` is a Unix epoch in milliseconds. The LinkedIn API
    /// requires `lifecycleState = SCHEDULED` and a non-null `scheduledAt`.
    pub async fn schedule_post(
        &self,
        text: &str,
        visibility: &str,
        scheduled_at_ms: i64,
    ) -> Result<Value, Error> {
        let body = build_share_body(text, visibility, "SCHEDULED", Some(scheduled_at_ms), None)?;
        self.execute_share_mutation(&body).await
    }

    /// Schedule a post with media attachment.
    ///
    /// Uploads the file via [`upload_media`], waits for processing to complete
    /// (polling up to `ready_timeout_secs`), then schedules the post.
    pub async fn schedule_post_with_media(
        &self,
        text: &str,
        visibility: &str,
        scheduled_at_ms: i64,
        media_path: &std::path::Path,
        title: Option<&str>,
        ready_timeout_secs: u64,
    ) -> Result<Value, Error> {
        let media = self.upload_media(media_path).await?;
        let ready = self
            .wait_media_ready(
                &media,
                std::time::Duration::from_secs(ready_timeout_secs.max(1)),
            )
            .await?;
        if !ready.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(Error::Api {
                status: 408,
                body: format!("media did not become READY: {ready}"),
                correlation_id: None,
            });
        }
        let body = build_share_body(
            text,
            visibility,
            "SCHEDULED",
            Some(scheduled_at_ms),
            Some((&media, title)),
        )?;
        self.execute_share_mutation(&body).await
    }

    /// Shared mutation executor for share creation.
    async fn execute_share_mutation(&self, body: &Value) -> Result<Value, Error> {
        let url = format!(
            "{}{}graphql?action=execute&queryId=voyagerContentcreationDashShares.279996efa5064c01775d5aff003d9377",
            BASE_URL, API_PREFIX
        );
        let resp = self
            .http()
            .post(&url)
            .header("Csrf-Token", self.jsessionid())
            .header("x-li-graphql-pegasus-client", "true")
            .json(body)
            .send()
            .await?;
        let json = check_response(resp).await?;
        check_graphql_errors(&json)?;
        Ok(json)
    }

    /// Upload an image/GIF/video/PDF to LinkedIn media storage.
    ///
    /// Handles single-part and multi-part uploads via the pre-signed CDN flow.
    /// Returns metadata including `urn`, `media_upload_type`, and `content_type`.
    pub async fn upload_media(&self, media_path: &std::path::Path) -> Result<Value, Error> {
        if !media_path.is_file() {
            return Err(Error::InvalidInput(format!(
                "media file does not exist: {}",
                media_path.display()
            )));
        }
        let filename = media_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::InvalidInput("media path has no valid filename".to_string()))?;
        let (upload_type, content_type) = media_upload_type(filename)?;
        let bytes = std::fs::read(media_path)
            .map_err(|e| Error::InvalidInput(format!("failed to read media file: {e}")))?;

        let body = serde_json::json!({
            "mediaUploadType": upload_type,
            "fileSize": bytes.len(),
            "hasOverlayImage": false,
            "uploadMetadataType": "SINGLE",
            "filename": filename,
        });
        let meta = self
            .post("voyagerMediaUploadMetadata?action=upload", &body)
            .await?;
        let value = meta.get("value").cloned().ok_or_else(|| Error::Api {
            status: 200,
            body: format!("missing upload metadata value: {meta}"),
            correlation_id: None,
        })?;

        let cdn = self.cdn_upload_client()?;

        // Multi-part flow if LinkedIn returned part upload requests.
        if let Some(parts) = value.get("partUploadRequests").and_then(|v| v.as_array()) {
            if !parts.is_empty() {
                return self
                    .upload_multipart(&cdn, &value, &bytes, content_type, upload_type, parts)
                    .await;
            }
        }

        // Single-part upload.
        let upload_url = value
            .get("singleUploadUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Api {
                status: 200,
                body: format!("missing singleUploadUrl: {value}"),
                correlation_id: None,
            })?;

        let mut req = cdn
            .put(upload_url)
            .header(reqwest::header::CONTENT_TYPE, content_type);
        if let Some(headers) = value.get("singleUploadHeaders").and_then(|v| v.as_object()) {
            for (name, val) in headers {
                if let Some(val) = val.as_str() {
                    req = req.header(name.as_str(), val);
                }
            }
        }
        let put = req.body(bytes.clone()).send().await?;
        let put_status = put.status().as_u16();
        if !put.status().is_success() {
            let err_body = put.text().await.unwrap_or_default();
            return Err(Error::Api {
                status: put_status,
                body: format!(
                    "CDN single-upload failed ({} bytes, status {put_status}): {err_body}",
                    bytes.len()
                ),
                correlation_id: None,
            });
        }
        let mut out = value;
        out["media_upload_type"] = serde_json::json!(upload_type);
        out["content_type"] = serde_json::json!(content_type);
        out["singleUploadStatus"] = serde_json::json!(put_status);
        Ok(out)
    }

    /// Upload file parts to LinkedIn's CDN using pre-signed URLs.
    async fn upload_multipart(
        &self,
        cdn: &reqwest::Client,
        value: &Value,
        bytes: &[u8],
        content_type: &str,
        upload_type: &str,
        parts: &[Value],
    ) -> Result<Value, Error> {
        for (i, part) in parts.iter().enumerate() {
            let url = part
                .get("partUploadUrl")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Api {
                    status: 200,
                    body: format!("part {i} missing partUploadUrl: {part}"),
                    correlation_id: None,
                })?;
            let (start, end) = parse_byte_range(
                part.get("byteRange").and_then(|v| v.as_str()).unwrap_or(""),
                bytes.len(),
            )?;
            let slice = bytes[start..=end].to_vec();
            let mut req = cdn
                .put(url)
                .header(reqwest::header::CONTENT_TYPE, content_type);
            if let Some(headers) = part.get("partUploadHeaders").and_then(|v| v.as_object()) {
                for (name, val) in headers {
                    if let Some(val) = val.as_str() {
                        req = req.header(name.as_str(), val);
                    }
                }
            }
            let resp = req.body(slice).send().await?;
            let status = resp.status().as_u16();
            if !resp.status().is_success() {
                let err_body = resp.text().await.unwrap_or_default();
                return Err(Error::Api {
                    status,
                    body: format!("CDN part {i} upload failed: {err_body}"),
                    correlation_id: None,
                });
            }
        }
        let mut out = value.clone();
        out["media_upload_type"] = serde_json::json!(upload_type);
        out["content_type"] = serde_json::json!(content_type);
        Ok(out)
    }

    /// Poll LinkedIn's media-processing status until the asset is READY.
    ///
    /// Polls every 2 seconds up to `timeout`. Returns `{ ok: bool, last: <status> }`.
    pub async fn wait_media_ready(
        &self,
        media: &Value,
        timeout: std::time::Duration,
    ) -> Result<Value, Error> {
        let urn = media
            .get("urn")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("media metadata missing urn".to_string()))?;
        let upload_type = media
            .get("media_upload_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::InvalidInput("media metadata missing media_upload_type".to_string())
            })?;
        let status_type = media_status_type(upload_type);
        let encoded_urn = restli_encode_string(urn);
        let path =
            format!("voyagerVideoDashMediaAssetStatus/{encoded_urn}?mediaStatusType={status_type}");
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_status = serde_json::Value::Null;
        while tokio::time::Instant::now() < deadline {
            match self.get(&path).await {
                Ok(json) => {
                    last_status = json.clone();
                    if json.get("processingStatus").and_then(|v| v.as_str()) == Some("READY")
                        || json.get("documentProcessingResult").is_some()
                    {
                        return Ok(serde_json::json!({
                            "ok": true,
                            "mediaStatusType": status_type,
                            "last": json,
                        }));
                    }
                    if matches!(
                        json.get("processingStatus").and_then(|v| v.as_str()),
                        Some("FAILED") | Some("PROCESSING_FAILED")
                    ) {
                        return Ok(serde_json::json!({
                            "ok": false,
                            "mediaStatusType": status_type,
                            "last": json,
                        }));
                    }
                }
                Err(_) => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        Ok(serde_json::json!({
            "ok": false,
            "mediaStatusType": status_type,
            "timeout_secs": timeout.as_secs(),
            "last": last_status,
        }))
    }

    /// Fetch a scheduled/published share by URN.
    pub async fn get_share(&self, share_urn: &str) -> Result<Value, Error> {
        let urn = share_urn
            .strip_prefix("urn:li:fsd_share:")
            .unwrap_or(share_urn);
        let enc = restli_encode_string(urn);
        let q = format!(
            "variables=(shareUrns:List({enc}))&queryId=voyagerContentcreationDashShares.b6c8295dc377a63224101ecce6d3c1ca"
        );
        let raw = self.graphql_get(&q).await?;
        Ok(raw)
    }

    /// Delete a scheduled/published share by URN.
    pub async fn delete_share(&self, share_urn: &str) -> Result<Value, Error> {
        let urn = share_urn
            .strip_prefix("urn:li:fsd_share:")
            .unwrap_or(share_urn);
        let encoded = restli_encode_string(urn);
        let url = format!(
            "{}{}contentcreation/normShares/{encoded}",
            BASE_URL, API_PREFIX
        );
        let resp = self
            .http()
            .delete(&url)
            .header("Csrf-Token", self.jsessionid())
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(Error::Api {
                status: status.as_u16(),
                body,
                correlation_id: None,
            });
        }
        Ok(serde_json::json!({
            "status_code": status.as_u16(),
            "share_urn": urn,
        }))
    }
}

/// Returns the uppercased reaction type on success, or an `InvalidInput`
/// error if the value is not recognized.
fn validate_reaction_type(reaction_type: &str) -> Result<String, Error> {
    let rt = reaction_type.to_uppercase();
    if VALID_REACTION_TYPES.contains(&rt.as_str()) {
        Ok(rt)
    } else {
        Err(Error::InvalidInput(format!(
            "invalid reaction type '{}'. Valid types: {}",
            reaction_type,
            VALID_REACTION_TYPES.join(", ")
        )))
    }
}

/// Check whether a feed element carries the given activity ID. The feed uses
/// several URN prefixes for the same post; we look in predictable carrier
/// slots (`entityUrn`, `updateMetadata.urn`, `updateMetadata.shareUrn`)
/// rather than serialising the whole element to a string.
fn element_matches_activity_id(element: &Value, activity_id: &str) -> bool {
    if entity_urn_contains(element, activity_id) {
        return true;
    }
    let metadata = element
        .pointer("/value/com.linkedin.voyager.feed.render.UpdateV2/updateMetadata")
        .or_else(|| element.pointer("/updateMetadata"));
    metadata.is_some_and(|m| metadata_contains_id(m, activity_id))
}

fn entity_urn_contains(element: &Value, id: &str) -> bool {
    element
        .get("entityUrn")
        .and_then(Value::as_str)
        .is_some_and(|s| s.contains(id))
}

fn metadata_contains_id(metadata: &Value, id: &str) -> bool {
    let urn_match = metadata
        .get("urn")
        .and_then(Value::as_str)
        .is_some_and(|s| s.contains(id));
    let share_match = metadata
        .get("shareUrn")
        .and_then(Value::as_str)
        .is_some_and(|s| s.contains(id));
    urn_match || share_match
}

/// Build the JSON body for a share-creation mutation (publish or schedule).
///
/// Centralises validation so `create_post`, `schedule_post`, and
/// `schedule_post_with_media` all go through one code path.
fn build_share_body(
    text: &str,
    visibility: &str,
    lifecycle_state: &str,
    scheduled_at_ms: Option<i64>,
    media: Option<(&Value, Option<&str>)>,
) -> Result<Value, Error> {
    let vis = visibility.to_uppercase();
    if vis != "ANYONE" && vis != "CONNECTIONS_ONLY" {
        return Err(Error::InvalidInput(format!(
            "invalid visibility '{visibility}'. Must be ANYONE or CONNECTIONS_ONLY"
        )));
    }
    let lifecycle = lifecycle_state.to_uppercase();
    if lifecycle != "PUBLISHED" && lifecycle != "SCHEDULED" {
        return Err(Error::InvalidInput(format!(
            "invalid lifecycle_state '{lifecycle_state}'. Must be PUBLISHED or SCHEDULED"
        )));
    }
    let mut post = serde_json::json!({
        "allowedCommentersScope": "ALL",
        "intendedShareLifeCycleState": lifecycle,
        "origin": "FEED",
        "visibilityDataUnion": { "visibilityType": vis },
        "commentary": { "text": text, "attributesV2": [] }
    });
    if lifecycle == "SCHEDULED" {
        let ts = scheduled_at_ms.ok_or_else(|| {
            Error::InvalidInput("scheduled_at_ms is required for SCHEDULED shares".to_string())
        })?;
        post["scheduledAt"] = serde_json::json!(ts);
    }
    if let Some((media_val, title)) = media {
        post["media"] = media_payload_from_upload(media_val, title)?;
    }
    Ok(serde_json::json!({
        "variables": { "post": post },
        "queryId": "voyagerContentcreationDashShares.279996efa5064c01775d5aff003d9377",
        "includeWebMetadata": true
    }))
}

/// Convert upload metadata into the `media` field expected by the share mutation.
fn media_payload_from_upload(media: &Value, title: Option<&str>) -> Result<Value, Error> {
    let upload_type = media
        .get("media_upload_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            Error::InvalidInput("media metadata missing media_upload_type".to_string())
        })?;
    let urn = media
        .get("urn")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::InvalidInput("media metadata missing urn".to_string()))?;
    let recipes = media.get("recipes").cloned();
    match upload_type {
        "VIDEO_SHARING" => Ok(serde_json::json!({
            "category": "VIDEO",
            "mediaUrn": urn,
            "recipes": recipes.unwrap_or_else(|| serde_json::json!([
                "urn:li:digitalmediaRecipe:feedshare-video-captions-thumbnails-ambry",
                "urn:li:digitalmediaRecipe:feedshare-video-auto-caption-public"
            ])),
            "nativeMediaSource": "PRE_RECORDED"
        })),
        "DOCUMENT_SHARING" => {
            let mut payload = serde_json::json!({
                "category": "NATIVE_DOCUMENT",
                "mediaUrn": urn,
                "recipes": recipes.unwrap_or_else(|| serde_json::json!([
                    "urn:li:digitalmediaRecipe:feedshare-document-preview",
                    "urn:li:digitalmediaRecipe:feedshare-document"
                ]))
            });
            if let Some(title) = title {
                payload["title"] = serde_json::json!(title);
            }
            Ok(payload)
        }
        "IMAGE_SHARING" => Ok(serde_json::json!({
            "category": "IMAGE",
            "mediaUrn": urn,
            "recipes": recipes.unwrap_or_else(|| serde_json::json!([
                "urn:li:digitalmediaRecipe:feedshare-image"
            ]))
        })),
        other => Err(Error::InvalidInput(format!(
            "unsupported media_upload_type '{other}'"
        ))),
    }
}

/// Map a filename extension to LinkedIn's `(uploadType, contentType)` pair.
fn media_upload_type(filename: &str) -> Result<(&'static str, &'static str), Error> {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Ok(("IMAGE_SHARING", "image/png")),
        "jpg" | "jpeg" => Ok(("IMAGE_SHARING", "image/jpeg")),
        "gif" => Ok(("IMAGE_SHARING", "image/gif")),
        "webp" => Ok(("IMAGE_SHARING", "image/webp")),
        "mp4" => Ok(("VIDEO_SHARING", "video/mp4")),
        "mov" => Ok(("VIDEO_SHARING", "video/quicktime")),
        "m4v" => Ok(("VIDEO_SHARING", "video/x-m4v")),
        "webm" => Ok(("VIDEO_SHARING", "video/webm")),
        "pdf" => Ok(("DOCUMENT_SHARING", "application/pdf")),
        _ => Err(Error::InvalidInput(format!(
            "unsupported LinkedIn media type for '{filename}'"
        ))),
    }
}

/// Map upload type to the status-type query parameter for media polling.
fn media_status_type(upload_type: &str) -> &'static str {
    match upload_type {
        "DOCUMENT_SHARING" => "DOCUMENT_PREVIEW",
        "VIDEO_SHARING" => "VIDEO",
        _ => "IMAGE",
    }
}

/// Parse a LinkedIn byte-range string like `"bytes 0-4194303"` into `(start, end)`
/// inclusive indices. Falls back to `(0, total - 1)` if the range is empty.
fn parse_byte_range(range: &str, total: usize) -> Result<(usize, usize), Error> {
    if range.is_empty() {
        return Ok((0, total.saturating_sub(1)));
    }
    let stripped = range.strip_prefix("bytes ").unwrap_or(range);
    let parts: Vec<&str> = stripped.split('-').collect();
    if parts.len() != 2 {
        return Err(Error::InvalidInput(format!(
            "invalid byte range '{range}', expected 'start-end'"
        )));
    }
    let start: usize = parts[0]
        .parse()
        .map_err(|_| Error::InvalidInput(format!("invalid byte range start '{}'", parts[0])))?;
    let end: usize = parts[1]
        .parse()
        .map_err(|_| Error::InvalidInput(format!("invalid byte range end '{}'", parts[1])))?;
    Ok((start, end.min(total.saturating_sub(1))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn element_matches_activity_id_via_entity_urn() {
        let element = json!({
            "entityUrn": "urn:li:fs_updateV2:(urn:li:activity:7447168805107032064,MEMBER_SHARES,DEBUG_REASON,DEFAULT,false)",
            "value": {}
        });
        assert!(element_matches_activity_id(&element, "7447168805107032064"));
        assert!(!element_matches_activity_id(
            &element,
            "9999999999999999999"
        ));
    }

    #[test]
    fn element_matches_activity_id_via_update_metadata_urn() {
        let element = json!({
            "value": {
                "com.linkedin.voyager.feed.render.UpdateV2": {
                    "updateMetadata": { "urn": "urn:li:activity:7312345678901234567" }
                }
            }
        });
        assert!(element_matches_activity_id(&element, "7312345678901234567"));
    }

    #[test]
    fn element_matches_activity_id_via_share_urn() {
        let element = json!({
            "value": {
                "com.linkedin.voyager.feed.render.UpdateV2": {
                    "updateMetadata": {
                        "urn": "urn:li:activity:7312345678901234567",
                        "shareUrn": "urn:li:ugcPost:7312345678901234560"
                    }
                }
            }
        });
        assert!(element_matches_activity_id(&element, "7312345678901234560"));
    }

    #[test]
    fn element_matches_activity_id_returns_false_when_absent() {
        let element = json!({ "value": { "unrelated": true } });
        assert!(!element_matches_activity_id(
            &element,
            "7312345678901234567"
        ));
    }

    #[test]
    fn media_upload_type_maps_supported_extensions() {
        assert_eq!(
            media_upload_type("photo.png").unwrap(),
            ("IMAGE_SHARING", "image/png")
        );
        assert_eq!(
            media_upload_type("photo.jpg").unwrap(),
            ("IMAGE_SHARING", "image/jpeg")
        );
        assert_eq!(
            media_upload_type("anim.gif").unwrap(),
            ("IMAGE_SHARING", "image/gif")
        );
        assert_eq!(
            media_upload_type("clip.mp4").unwrap(),
            ("VIDEO_SHARING", "video/mp4")
        );
        assert_eq!(
            media_upload_type("doc.pdf").unwrap(),
            ("DOCUMENT_SHARING", "application/pdf")
        );
        assert!(media_upload_type("data.csv").is_err());
    }

    #[test]
    fn parse_byte_range_parses_valid_ranges() {
        assert_eq!(
            parse_byte_range("bytes 0-4194303", 8_000_000).unwrap(),
            (0, 4194303)
        );
        assert_eq!(parse_byte_range("0-99", 100).unwrap(), (0, 99));
        assert_eq!(parse_byte_range("", 100).unwrap(), (0, 99));
        // End clamped to total - 1
        assert_eq!(parse_byte_range("0-200", 100).unwrap(), (0, 99));
    }

    #[test]
    fn build_share_body_requires_scheduled_at_for_scheduled() {
        let err = build_share_body("text", "ANYONE", "SCHEDULED", None, None).unwrap_err();
        assert!(err.to_string().contains("scheduled_at_ms"));
    }

    #[test]
    fn build_share_body_rejects_bad_visibility() {
        let err = build_share_body("text", "PRIVATE", "PUBLISHED", None, None).unwrap_err();
        assert!(err.to_string().contains("visibility"));
    }

    #[test]
    fn build_share_body_builds_scheduled_payload() {
        let body =
            build_share_body("hello", "ANYONE", "SCHEDULED", Some(1700000000000i64), None).unwrap();
        let post = &body["variables"]["post"];
        assert_eq!(post["intendedShareLifeCycleState"], "SCHEDULED");
        assert_eq!(post["scheduledAt"], 1700000000000i64);
    }
}
