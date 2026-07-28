//! Profile-related API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::{EndpointOutcome, Error, ProfileResolutionFailure, ResolutionAttempt};
use crate::urn::ProfileUrn;

use super::internal::{graphql_params, restli_encode_string, unwrap_graphql};
use super::{LinkedInClient, BASE_URL};

/// Endpoint labels used in [`ResolutionAttempt`] records. Kept short and
/// stable: they end up in user-visible error messages and in the CLI's
/// `--json` error output, so they double as the locator a reader uses to
/// check which endpoint said what.
const ENDPOINT_MINIPROFILE: &str = "identity/miniprofiles";
const ENDPOINT_REST_PROFILE: &str = "identity/profiles";
const ENDPOINT_GRAPHQL: &str = "graphql:ProfilesByMemberIdentity";
const ENDPOINT_PRELOAD: &str = "preload/custom-invite";

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
    ///
    /// Also caches the user's own vanity slug off the same response, so the
    /// self-slug check in [`LinkedInClient::resolve_profile_urn`] costs no
    /// extra request once anything in the session has touched `/me`.
    pub async fn my_profile_urn(&self) -> Result<&str, Error> {
        self.profile_urn
            .get_or_try_init(|| async {
                let me = self.get("me").await?;
                self.cache_self_public_id(&me);
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

    /// Return the authenticated user's vanity slug (`publicIdentifier`),
    /// fetching and caching it from `/me` on first call.
    ///
    /// This costs a network request when nothing has populated the cache
    /// yet. [`LinkedInClient::resolve_profile_urn`] deliberately does NOT
    /// call it: adding a request to an already-failing path is the wrong
    /// trade when the same answer can be seeded offline. Callers that want
    /// a definitive self-slug answer can call this explicitly.
    pub async fn my_public_id(&self) -> Result<&str, Error> {
        self.self_public_id
            .get_or_try_init(|| async {
                let me = self.get("me").await?;
                extract_self_public_id(&me).ok_or_else(|| Error::Api {
                    status: 0,
                    body: "could not extract miniProfile.publicIdentifier from /me response"
                        .to_string(),
                    correlation_id: None,
                })
            })
            .await
            .map(|s| s.as_str())
    }

    /// Record the authenticated user's own vanity slug without a network
    /// call, e.g. from a persisted session file.
    ///
    /// Returns `true` when the value was stored, `false` when a value was
    /// already cached (first writer wins, as with any [`tokio::sync::OnceCell`]).
    pub fn set_self_public_id(&self, public_id: &str) -> bool {
        self.self_public_id.set(public_id.to_string()).is_ok()
    }

    /// The authenticated user's own vanity slug when it is already known
    /// offline. `None` means "not known", never "not this one".
    pub fn known_self_public_id(&self) -> Option<&str> {
        self.self_public_id.get().map(String::as_str)
    }

    /// Three-valued, offline-only self-slug test.
    ///
    /// `Some(true)` / `Some(false)` when the user's own slug is known,
    /// `None` when it isn't. The distinction matters: the classifier must
    /// not read "unknown" as "not a self-slug".
    fn slug_is_self(&self, public_id: &str) -> Option<bool> {
        self.known_self_public_id()
            .map(|me| me.eq_ignore_ascii_case(public_id))
    }

    /// Opportunistically cache the self slug off a `/me` response.
    fn cache_self_public_id(&self, me: &Value) {
        if let Some(public_id) = extract_self_public_id(me) {
            let _ = self.self_public_id.set(public_id);
        }
    }

    /// Fetch "who viewed my profile" data.
    pub async fn get_profile_viewers(&self) -> Result<Value, Error> {
        self.get("identity/wvmpCards").await
    }

    /// Resolve a public identifier (vanity URL slug) to an `fsd_profile` URN.
    /// Tries the REST miniprofile endpoint, the REST profile endpoint, the
    /// GraphQL profile endpoint, and finally the preload-page scraper.
    ///
    /// On failure returns [`Error::ProfileResolution`], which classifies the
    /// failure (rate limited / not found / self-slug / inconclusive) and
    /// carries the per-endpoint outcomes that produced the classification.
    /// The old behaviour collapsed all of those into one opaque
    /// "could not extract entityUrn from any profile endpoint" string, which
    /// is unusable to a caller because the recoveries are opposites.
    ///
    /// An [`Error::Auth`] from any endpoint short-circuits the whole
    /// resolver: an expired session is not a fact about the requested slug,
    /// and reporting it as "profile not found" sends the caller after the
    /// wrong fix.
    ///
    /// A slug already known to be the caller's own is NOT short-circuited
    /// before the endpoint round: the endpoints are still tried, and the
    /// self-slug verdict is only applied to the failure. That keeps the
    /// classification honest if one of them turns out to answer for a
    /// self-slug after all.
    pub async fn resolve_profile_urn(&self, public_id: &str) -> Result<ProfileUrn, Error> {
        let mut attempts: Vec<ResolutionAttempt> = Vec::with_capacity(4);

        match self.resolve_via_miniprofile(public_id).await {
            Ok(urn) => return Ok(ProfileUrn::new(urn)),
            Err(err) => record_attempt(&mut attempts, ENDPOINT_MINIPROFILE, err)?,
        }
        match self.resolve_via_rest_profile(public_id).await {
            Ok(urn) => return Ok(ProfileUrn::new(urn)),
            Err(err) => record_attempt(&mut attempts, ENDPOINT_REST_PROFILE, err)?,
        }
        match self.resolve_via_graphql(public_id).await {
            Ok(urn) => return Ok(ProfileUrn::new(urn)),
            Err(err) => record_attempt(&mut attempts, ENDPOINT_GRAPHQL, err)?,
        }
        match self.resolve_profile_urn_via_preload(public_id).await {
            Ok(urn) => return Ok(urn),
            Err(err) => record_attempt(&mut attempts, ENDPOINT_PRELOAD, err)?,
        }

        let reason = classify_resolution_failure(&attempts, self.slug_is_self(public_id));
        Err(Error::ProfileResolution {
            public_id: public_id.to_string(),
            reason,
            attempts,
        })
    }

    async fn resolve_via_miniprofile(&self, public_id: &str) -> Result<String, Error> {
        let path = format!(
            "identity/miniprofiles?q=memberIdentity&memberIdentity={}",
            public_id
        );
        let resp = self.get(&path).await?;
        resp.get("elements")
            .and_then(|e| e.as_array())
            .and_then(|arr| arr.first())
            .and_then(|mp| mp.get("dashEntityUrn").or(mp.get("entityUrn")))
            .and_then(|v| v.as_str())
            .map(coerce_to_fsd_profile)
            .ok_or_else(|| no_urn_in_payload(ENDPOINT_MINIPROFILE, public_id))
    }

    async fn resolve_via_rest_profile(&self, public_id: &str) -> Result<String, Error> {
        let path = format!("identity/profiles/{}", public_id);
        let resp = self.get(&path).await?;
        if let Some(urn) = resp
            .get("miniProfile")
            .and_then(|mp| mp.get("dashEntityUrn"))
            .and_then(|v| v.as_str())
        {
            return Ok(urn.to_string());
        }
        resp.get("entityUrn")
            .and_then(|v| v.as_str())
            .map(coerce_to_fsd_profile)
            .ok_or_else(|| no_urn_in_payload(ENDPOINT_REST_PROFILE, public_id))
    }

    /// Resolve a slug via the GraphQL profile query, returning the
    /// `entityUrn` of the first matching profile.
    async fn resolve_via_graphql(&self, public_id: &str) -> Result<String, Error> {
        let profile = self.get_profile(public_id).await?;
        profile
            .get("entityUrn")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| no_urn_in_payload(ENDPOINT_GRAPHQL, public_id))
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
        // `status: 0` (not 200) because this is a synthesised error, not an
        // HTTP one: the request succeeded and the page simply carried no
        // URN. The crate-wide convention is that 0 means "we made this up",
        // and `EndpointOutcome::from_error` reads it as an empty answer.
        extract_profile_urn_from_preload_html(&html, public_id)
            .map(ProfileUrn::new)
            .ok_or_else(|| no_urn_in_payload(ENDPOINT_PRELOAD, public_id))
    }
}

/// Record one failed resolver step, or bail out of the whole resolver.
///
/// Returns `Err` only for errors that are facts about the *session* rather
/// than about the requested slug (currently [`Error::Auth`]); the caller
/// propagates those untouched with `?`.
fn record_attempt(
    attempts: &mut Vec<ResolutionAttempt>,
    endpoint: &'static str,
    err: Error,
) -> Result<(), Error> {
    if matches!(err, Error::Auth(_)) {
        return Err(err);
    }
    attempts.push(ResolutionAttempt::from_error(endpoint, &err));
    Ok(())
}

/// Synthesised "the endpoint answered but carried no URN" error.
fn no_urn_in_payload(endpoint: &str, public_id: &str) -> Error {
    Error::Api {
        status: 0,
        body: format!("{endpoint} returned no profile URN for '{public_id}'"),
        correlation_id: None,
    }
}

/// Classify a fully-failed resolution from the per-endpoint outcomes and an
/// offline self-slug verdict.
///
/// `slug_is_self` is three-valued on purpose: `None` means the client does
/// not know the authenticated user's own slug, and must not be read as
/// `Some(false)`.
///
/// Precedence, and why:
///
/// 1. **Self-slug** outranks everything. It is a permanent structural fact
///    about these endpoints, so backing off and retrying would spend quota
///    on a request that can never succeed.
/// 2. **Rate limited** next, keyed on a real HTTP 429 that survived the
///    client's own retry loop, never on a guess about response shape.
/// 3. **Not found** only when every endpoint gave a clean absence (404 or
///    an empty payload). One odd status is enough to disqualify it.
/// 4. Otherwise **inconclusive**, which is a real answer rather than a
///    dressed-up "not found".
fn classify_resolution_failure(
    attempts: &[ResolutionAttempt],
    slug_is_self: Option<bool>,
) -> ProfileResolutionFailure {
    if slug_is_self == Some(true) {
        return ProfileResolutionFailure::SelfSlugUnsupported;
    }
    if attempts
        .iter()
        .any(|a| a.outcome == EndpointOutcome::Status(429))
    {
        return ProfileResolutionFailure::RateLimited;
    }
    let every_endpoint_says_absent = !attempts.is_empty()
        && attempts.iter().all(|a| {
            matches!(
                a.outcome,
                EndpointOutcome::Empty | EndpointOutcome::Status(404)
            )
        });
    if every_endpoint_says_absent {
        return ProfileResolutionFailure::NotFound;
    }
    ProfileResolutionFailure::Inconclusive
}

/// Pull `miniProfile.publicIdentifier` out of a `/me` response.
fn extract_self_public_id(me: &Value) -> Option<String> {
    me.get("miniProfile")
        .and_then(|mp| mp.get("publicIdentifier"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
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

    // ----- resolution failure classification -----

    fn attempt(endpoint: &'static str, outcome: EndpointOutcome) -> ResolutionAttempt {
        ResolutionAttempt { endpoint, outcome }
    }

    /// The shape of a slug nobody has ever heard of: clean absence everywhere.
    fn all_absent() -> Vec<ResolutionAttempt> {
        vec![
            attempt(ENDPOINT_MINIPROFILE, EndpointOutcome::Empty),
            attempt(ENDPOINT_REST_PROFILE, EndpointOutcome::Status(404)),
            attempt(ENDPOINT_GRAPHQL, EndpointOutcome::Empty),
            attempt(ENDPOINT_PRELOAD, EndpointOutcome::Empty),
        ]
    }

    #[test]
    fn classify_all_absent_is_not_found() {
        assert_eq!(
            classify_resolution_failure(&all_absent(), Some(false)),
            ProfileResolutionFailure::NotFound
        );
    }

    #[test]
    fn classify_unknown_self_identity_still_reports_not_found() {
        // `None` (self slug unknown) must not block the not-found verdict.
        assert_eq!(
            classify_resolution_failure(&all_absent(), None),
            ProfileResolutionFailure::NotFound
        );
    }

    #[test]
    fn classify_any_429_is_rate_limited() {
        let mut attempts = all_absent();
        attempts[1].outcome = EndpointOutcome::Status(429);
        assert_eq!(
            classify_resolution_failure(&attempts, Some(false)),
            ProfileResolutionFailure::RateLimited
        );
    }

    #[test]
    fn classify_self_slug_outranks_rate_limit() {
        // Both signals present. Waiting out the 429 would never help, so the
        // self-slug verdict has to win.
        let mut attempts = all_absent();
        attempts[0].outcome = EndpointOutcome::Status(429);
        assert_eq!(
            classify_resolution_failure(&attempts, Some(true)),
            ProfileResolutionFailure::SelfSlugUnsupported
        );
    }

    #[test]
    fn classify_self_slug_on_clean_absence() {
        assert_eq!(
            classify_resolution_failure(&all_absent(), Some(true)),
            ProfileResolutionFailure::SelfSlugUnsupported
        );
    }

    #[test]
    fn classify_odd_status_is_inconclusive_not_not_found() {
        // A 403 (block/captcha) or a 5xx is not evidence the profile is
        // missing, so it must not be laundered into NotFound.
        for status in [403u16, 500, 999] {
            let mut attempts = all_absent();
            attempts[3].outcome = EndpointOutcome::Status(status);
            assert_eq!(
                classify_resolution_failure(&attempts, Some(false)),
                ProfileResolutionFailure::Inconclusive,
                "status {status} must stay inconclusive"
            );
        }
    }

    #[test]
    fn classify_transport_failure_is_inconclusive() {
        let mut attempts = all_absent();
        attempts[0].outcome = EndpointOutcome::Transport("connection reset".to_string());
        assert_eq!(
            classify_resolution_failure(&attempts, Some(false)),
            ProfileResolutionFailure::Inconclusive
        );
    }

    #[test]
    fn classify_no_attempts_is_inconclusive() {
        assert_eq!(
            classify_resolution_failure(&[], None),
            ProfileResolutionFailure::Inconclusive
        );
    }

    #[test]
    fn synthesised_empty_error_maps_to_empty_outcome() {
        let err = no_urn_in_payload(ENDPOINT_GRAPHQL, "someone");
        assert_eq!(EndpointOutcome::from_error(&err), EndpointOutcome::Empty);
    }

    #[test]
    fn http_status_error_maps_to_status_outcome() {
        let err = Error::Api {
            status: 429,
            body: "too many requests".to_string(),
            correlation_id: None,
        };
        assert_eq!(
            EndpointOutcome::from_error(&err),
            EndpointOutcome::Status(429)
        );
    }

    #[test]
    fn record_attempt_bails_out_on_auth_error() {
        let mut attempts = Vec::new();
        let result = record_attempt(
            &mut attempts,
            ENDPOINT_MINIPROFILE,
            Error::Auth("session expired".to_string()),
        );
        assert!(result.is_err(), "auth errors must short-circuit");
        assert!(
            attempts.is_empty(),
            "an auth failure is not an endpoint verdict about the slug"
        );
    }

    #[test]
    fn record_attempt_collects_non_auth_error() {
        let mut attempts = Vec::new();
        record_attempt(
            &mut attempts,
            ENDPOINT_REST_PROFILE,
            Error::Api {
                status: 404,
                body: String::new(),
                correlation_id: None,
            },
        )
        .expect("non-auth errors must be collected, not propagated");
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].endpoint, ENDPOINT_REST_PROFILE);
        assert_eq!(attempts[0].outcome, EndpointOutcome::Status(404));
    }

    // ----- self-slug identity -----

    #[test]
    fn extract_self_public_id_reads_mini_profile() {
        let me = serde_json::json!({
            "miniProfile": { "publicIdentifier": "jane-doe-42" }
        });
        assert_eq!(extract_self_public_id(&me), Some("jane-doe-42".to_string()));
    }

    #[test]
    fn extract_self_public_id_rejects_missing_and_empty() {
        assert!(extract_self_public_id(&serde_json::json!({})).is_none());
        assert!(extract_self_public_id(
            &serde_json::json!({ "miniProfile": { "publicIdentifier": "" } })
        )
        .is_none());
    }

    #[test]
    fn slug_is_self_is_unknown_until_seeded() {
        let client = LinkedInClient::new().expect("client creation must succeed");
        assert_eq!(client.slug_is_self("olgasafonova"), None);
        assert!(client.known_self_public_id().is_none());
    }

    #[test]
    fn slug_is_self_matches_case_insensitively_once_seeded() {
        let client = LinkedInClient::new().expect("client creation must succeed");
        assert!(client.set_self_public_id("olgasafonova"));
        assert_eq!(client.slug_is_self("OlgaSafonova"), Some(true));
        assert_eq!(client.slug_is_self("eric-vyacheslav"), Some(false));
    }

    #[test]
    fn set_self_public_id_is_first_writer_wins() {
        let client = LinkedInClient::new().expect("client creation must succeed");
        assert!(client.set_self_public_id("first"));
        assert!(!client.set_self_public_id("second"));
        assert_eq!(client.known_self_public_id(), Some("first"));
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
