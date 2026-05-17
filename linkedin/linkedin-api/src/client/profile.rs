//! Profile-related API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;
use crate::urn::ProfileUrn;

use super::internal::{graphql_params, restli_encode_string, unwrap_graphql};
use super::{LinkedInClient, BASE_URL};

impl LinkedInClient {
    /// Fetch a user's full profile by public identifier (vanity URL slug).
    pub async fn get_profile(&self, public_id: &str) -> Result<Value, Error> {
        self.profile_query_first(
            public_id,
            "voyagerIdentityDashProfiles.5f50f83f76a1e270603613bdd0fb0252",
            "memberIdentity",
        )
        .await
    }

    /// Visit a profile, registering the view so the target sees it in
    /// "Who Viewed My Profile".
    pub async fn visit_profile(&self, public_id: &str) -> Result<Value, Error> {
        self.profile_query_first(
            public_id,
            "voyagerIdentityDashProfiles.a3de77c32c473719f1c58fae6bff43a5",
            "vanityName",
        )
        .await
    }

    /// Shared GraphQL profile query that takes a single string variable
    /// (`memberIdentity` for plain reads, `vanityName` for visits) and
    /// returns the first element of the resulting collection.
    async fn profile_query_first(
        &self,
        public_id: &str,
        query_id: &str,
        var_name: &str,
    ) -> Result<Value, Error> {
        let restli_id = restli_encode_string(public_id);
        let variables = format!("({}:{})", var_name, restli_id);
        let params = graphql_params(&variables, query_id, "ProfilesByMemberIdentity");
        let raw = self.graphql_get(&params).await?;
        let collection = unwrap_graphql(&raw, "identityDashProfilesByMemberIdentity")?;
        collection
            .get("elements")
            .and_then(|e| e.as_array())
            .and_then(|arr| arr.first())
            .cloned()
            .ok_or_else(|| Error::Api {
                status: 0,
                body: format!(
                    "unexpected GraphQL response shape (missing elements[0] in identityDashProfilesByMemberIdentity): {}",
                    serde_json::to_string(&raw).unwrap_or_default()
                ),
            correlation_id: None,
            })
    }

    /// Fetch the authenticated user's own profile (`/voyager/api/me`).
    pub async fn get_me(&self) -> Result<Value, Error> {
        self.get("me").await
    }

    /// Return the authenticated user's `fsd_profile` URN, fetching and
    /// caching it from `/me` on first call.
    pub async fn my_profile_urn(&self) -> Result<&str, Error> {
        self.profile_urn
            .get_or_try_init(|| async {
                let me = self.get("me").await?;
                me.get("miniProfile")
                    .and_then(|mp| mp.get("dashEntityUrn"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::Api {
                        status: 0,
                        body: "could not extract miniProfile.dashEntityUrn from /me response"
                            .to_string(),
                        correlation_id: None,
                    })
            })
            .await
            .map(|s| s.as_str())
    }

    /// Fetch "who viewed my profile" data.
    pub async fn get_profile_viewers(&self) -> Result<Value, Error> {
        self.get("identity/wvmpCards").await
    }

    /// Resolve a public identifier (vanity URL slug) to an `fsd_profile` URN.
    /// Tries the REST miniprofile endpoint, the REST profile endpoint, the
    /// GraphQL profile endpoint, and finally the preload-page scraper.
    pub async fn resolve_profile_urn(&self, public_id: &str) -> Result<ProfileUrn, Error> {
        if let Some(urn) = self.resolve_via_miniprofile(public_id).await {
            return Ok(ProfileUrn::new(urn));
        }
        if let Some(urn) = self.resolve_via_rest_profile(public_id).await {
            return Ok(ProfileUrn::new(urn));
        }
        if let Ok(profile) = self.get_profile(public_id).await {
            if let Some(urn) = profile.get("entityUrn").and_then(|v| v.as_str()) {
                return Ok(ProfileUrn::new(urn));
            }
        }
        if let Ok(urn) = self.resolve_profile_urn_via_preload(public_id).await {
            return Ok(urn);
        }
        Err(Error::Api {
            status: 0,
            body: format!(
                "could not extract entityUrn from any profile endpoint for '{}'",
                public_id
            ),
            correlation_id: None,
        })
    }

    async fn resolve_via_miniprofile(&self, public_id: &str) -> Option<String> {
        let path = format!(
            "identity/miniprofiles?q=memberIdentity&memberIdentity={}",
            public_id
        );
        let resp = self.get(&path).await.ok()?;
        let urn = resp
            .get("elements")
            .and_then(|e| e.as_array())
            .and_then(|arr| arr.first())
            .and_then(|mp| mp.get("dashEntityUrn").or(mp.get("entityUrn")))
            .and_then(|v| v.as_str())?;
        Some(coerce_to_fsd_profile(urn))
    }

    async fn resolve_via_rest_profile(&self, public_id: &str) -> Option<String> {
        let path = format!("identity/profiles/{}", public_id);
        let resp = self.get(&path).await.ok()?;
        if let Some(urn) = resp
            .get("miniProfile")
            .and_then(|mp| mp.get("dashEntityUrn"))
            .and_then(|v| v.as_str())
        {
            return Some(urn.to_string());
        }
        let urn = resp.get("entityUrn").and_then(|v| v.as_str())?;
        Some(coerce_to_fsd_profile(urn))
    }

    /// Resolve a slug → fsd_profile URN by scraping the
    /// `/preload/custom-invite/?vanityName=<slug>` page. Used as a fallback
    /// after REST/GraphQL paths exhaust their retries.
    pub async fn resolve_profile_urn_via_preload(
        &self,
        public_id: &str,
    ) -> Result<ProfileUrn, Error> {
        let encoded_slug =
            url::form_urlencoded::byte_serialize(public_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/preload/custom-invite/?vanityName={}",
            BASE_URL, encoded_slug
        );

        let resp = self
            .http()
            .get(&url)
            .header("Csrf-Token", self.jsessionid())
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Error::Api {
                status: resp.status().as_u16(),
                body: format!(
                    "preload page returned non-success for vanityName={}",
                    public_id
                ),
                correlation_id: None,
            });
        }

        let html = resp.text().await?;
        extract_profile_urn_from_preload_html(&html, public_id)
            .map(ProfileUrn::new)
            .ok_or_else(|| Error::Api {
                status: 200,
                body: format!(
                    "preload page did not contain an fsd_profile URN paired with publicIdentifier={}",
                    public_id
                ),
                correlation_id: None,
            })
    }
}

/// Convert `fs_miniProfile` / `fs_profile` URNs to the modern `fsd_profile`
/// form, leaving already-correct URNs alone.
fn coerce_to_fsd_profile(urn: &str) -> String {
    if urn.contains("fs_miniProfile") || urn.contains("fs_profile") {
        urn.replace("fs_miniProfile", "fsd_profile")
            .replace("fs_profile", "fsd_profile")
    } else {
        urn.to_string()
    }
}

/// Extract the fsd_profile URN paired with a given slug from the preload
/// custom-invite page's embedded JSON state.
///
/// The page is a 1MB+ HTML document; the relevant JSON is doubly encoded
/// (an HTML-escaped `&quot;` string inside a `<code>` block) so quote
/// characters appear as `&quot;`. The viewer's URN appears earlier in the
/// document, so we anchor on the slug and look 500 bytes back for the
/// nearest preceding `entityUrn` value.
fn extract_profile_urn_from_preload_html(html: &str, slug: &str) -> Option<String> {
    let slug_marker = format!("publicIdentifier&quot;:&quot;{}&quot;", slug);
    let slug_pos = html.find(&slug_marker)?;
    let window_start = slug_pos.saturating_sub(500);
    let window = &html[window_start..slug_pos];

    let urn_key = "entityUrn&quot;:&quot;urn:li:fsd_profile:";
    let urn_key_pos = window.rfind(urn_key)?;
    let urn_start = window_start + urn_key_pos + "entityUrn&quot;:&quot;".len();
    let rel_end = html[urn_start..].find("&quot;")?;
    let urn = &html[urn_start..urn_start + rel_end];

    if urn.starts_with("urn:li:fsd_profile:") {
        Some(urn.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preload_extractor_finds_target_urn() {
        let html = r#"
        <code>
        {&quot;data&quot;:{&quot;identityProfile&quot;:[{
        &quot;firstName&quot;:&quot;Test&quot;,
        &quot;lastName&quot;:&quot;Viewer&quot;,
        &quot;dashEntityUrn&quot;:&quot;urn:li:fsd_profile:ACoAAA111VIEWER&quot;,
        &quot;publicIdentifier&quot;:&quot;test-viewer&quot;
        }]}}
        ...lots of unrelated bytes...
        {&quot;target&quot;:{
        &quot;entityUrn&quot;:&quot;urn:li:fsd_profile:ACoAAA222TARGET&quot;,
        &quot;emailRequired&quot;:false,
        &quot;publicIdentifier&quot;:&quot;target-user&quot;
        }}
        </code>"#;

        let urn = extract_profile_urn_from_preload_html(html, "target-user").unwrap();
        assert_eq!(urn, "urn:li:fsd_profile:ACoAAA222TARGET");
    }

    #[test]
    fn preload_extractor_picks_correct_urn_when_viewer_listed_first() {
        let html = r#"&quot;entityUrn&quot;:&quot;urn:li:fsd_profile:VIEWER&quot;,&quot;publicIdentifier&quot;:&quot;me&quot;,...later...&quot;entityUrn&quot;:&quot;urn:li:fsd_profile:TARGET&quot;,&quot;publicIdentifier&quot;:&quot;target-slug&quot;"#;
        let urn = extract_profile_urn_from_preload_html(html, "target-slug").unwrap();
        assert_eq!(urn, "urn:li:fsd_profile:TARGET");
    }

    #[test]
    fn preload_extractor_returns_none_when_slug_absent() {
        let html = r#"&quot;publicIdentifier&quot;:&quot;someone-else&quot;"#;
        assert!(extract_profile_urn_from_preload_html(html, "missing").is_none());
    }

    #[test]
    fn preload_extractor_returns_none_when_no_urn_in_window() {
        let big_padding = "x".repeat(2000);
        let html = format!(
            "&quot;entityUrn&quot;:&quot;urn:li:fsd_profile:TARGET&quot; {} &quot;publicIdentifier&quot;:&quot;jomar&quot;",
            big_padding
        );
        assert!(extract_profile_urn_from_preload_html(&html, "jomar").is_none());
    }
}
