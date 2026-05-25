//! Feed-related API methods on `LinkedInClient`.
//!
//! Covers: feed listing, single-post lookup, my-posts, reactions
//! (read/create/delete), comments (read/create), and post creation.

use serde_json::Value;

use crate::error::Error;
use crate::urn::{ActivityUrn, ProfileUrn, SocialDetailUrn};

use super::internal::{
    check_graphql_errors, check_response, graphql_params, restli_encode_string, unwrap_graphql,
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

    /// Fetch the authenticated user's own posts with engagement metrics.
    /// Thin convenience wrapper around `get_member_posts` that self-resolves
    /// the caller's `fsd_profile` URN.
    pub async fn get_my_posts(&self, start: u32, count: u32) -> Result<Value, Error> {
        let profile_urn = self.my_profile_urn().await?;
        let urn = ProfileUrn::new(profile_urn);
        self.get_member_posts(&urn, start, count).await
    }

    /// Fetch posts and reshares by a specific member.
    ///
    /// Hits the `memberShareFeed` finder on `identity/profileUpdatesV2` for any
    /// `fsd_profile` URN. See `re/my_posts.md` for the full endpoint contract.
    ///
    /// Reshared posts surface with the *original* author in the `actor` field,
    /// not the queried member. Callers that want to separate originals from
    /// reshares should compare `actor.entityUrn` against the URN they queried
    /// for.
    pub async fn get_member_posts(
        &self,
        profile_urn: &ProfileUrn,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        self.get(&member_posts_path(profile_urn.as_str(), start, count))
            .await
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

    /// Create a new text-only post on the authenticated user's feed.
    ///
    /// Uses the web-style mutation format: `variables` + `queryId` are sent
    /// in the POST body, not just URL params. The server uses both, but the
    /// body shape is required for the modern endpoint.
    pub async fn create_post(&self, text: &str, visibility: &str) -> Result<Value, Error> {
        let vis = visibility.to_uppercase();
        let body = serde_json::json!({
            "variables": {
                "post": {
                    "allowedCommentersScope": "ALL",
                    "intendedShareLifeCycleState": "PUBLISHED",
                    "origin": "FEED",
                    "visibilityDataUnion": { "visibilityType": vis },
                    "commentary": { "text": text, "attributesV2": [] }
                }
            },
            "queryId": "voyagerContentcreationDashShares.279996efa5064c01775d5aff003d9377",
            "includeWebMetadata": true
        });

        let url = format!(
            "{}{}graphql?action=execute&queryId=voyagerContentcreationDashShares.279996efa5064c01775d5aff003d9377",
            BASE_URL, API_PREFIX
        );
        let resp = self
            .http()
            .post(&url)
            .header("Csrf-Token", self.jsessionid())
            .header("x-li-graphql-pegasus-client", "true")
            .json(&body)
            .send()
            .await?;
        let json = check_response(resp).await?;
        check_graphql_errors(&json)?;
        Ok(json)
    }
}

/// Build the `identity/profileUpdatesV2` path for the `memberShareFeed`
/// finder. The profile URN is Rest.li-encoded (colons → `%3A`); the path
/// fragment then lives inside a URL query string where those `%` already
/// represent literal percent characters, so no further encoding is needed
/// here. See `re/my_posts.md` for the double-encoding discussion.
fn member_posts_path(profile_urn: &str, start: u32, count: u32) -> String {
    let encoded_urn = restli_encode_string(profile_urn);
    format!(
        "identity/profileUpdatesV2?q=memberShareFeed&profileUrn={}&moduleKey=member-shares%3Aphone&start={}&count={}",
        encoded_urn, start, count
    )
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
    fn member_posts_path_encodes_colons_and_carries_pagination() {
        let path = member_posts_path("urn:li:fsd_profile:ACoAAA111", 0, 10);
        // The fsd_profile URN's colons must be Rest.li-encoded as %3A so the
        // server's URL parser hands the full URN to the q=memberShareFeed
        // finder rather than splitting on the first colon.
        assert!(path.contains("profileUrn=urn%3Ali%3Afsd_profile%3AACoAAA111"));
        assert!(path.contains("q=memberShareFeed"));
        assert!(path.contains("moduleKey=member-shares%3Aphone"));
        assert!(path.contains("start=0"));
        assert!(path.contains("count=10"));
        assert!(path.starts_with("identity/profileUpdatesV2?"));
    }

    #[test]
    fn member_posts_path_round_trips_arbitrary_pagination() {
        let path = member_posts_path("urn:li:fsd_profile:ACoAAA222", 25, 50);
        assert!(path.contains("start=25"));
        assert!(path.contains("count=50"));
    }
}
