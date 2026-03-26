//! HTTP client wrapper for LinkedIn API requests.
//!
//! Implements the LinkedIn Android app's HTTP client behavior, including:
//! - Cookie jar with JSESSIONID (CSRF token) auto-generation
//! - X-LI-Track device telemetry header (main-app format)
//! - X-UDID persistent device identifier
//! - X-RestLi-Protocol-Version 2.0.0
//! - Csrf-Token header echoing the JSESSIONID cookie value
//!
//! Reference: `re/architecture_overview.md`, `re/device_fingerprinting.md`,
//! `re/restli_protocol.md`, `re/auth_flow.md`.

use std::sync::Arc;

use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::auth::Session;
use crate::error::Error;

/// Base URL for all LinkedIn API requests. There is no separate API subdomain;
/// all API calls go to `www.linkedin.com` with path-based routing.
const BASE_URL: &str = "https://www.linkedin.com";

/// API path prefix for the international variant (Voyager).
/// The China variant uses `/zephyr/api/` but we target production.
const API_PREFIX: &str = "/voyager/api/";

/// LinkedIn app version to impersonate. Should match a recent Play Store release.
const CLIENT_VERSION: &str = "4.2.1058";

/// Numeric build/version code corresponding to CLIENT_VERSION.
const CLIENT_MINOR_VERSION: i64 = 562100;

/// HTTP client configured to impersonate the LinkedIn Android app.
///
/// Holds a `reqwest::Client` with cookie jar, persistent device identity,
/// and pre-built headers that match the Android app's networking stack.
///
/// # Cookie Management
///
/// The client uses reqwest's built-in cookie store (`cookie_store(true)`).
/// The JSESSIONID cookie is set on `.linkedin.com` before any request.
/// The `li_at` session cookie will be set by the server after authentication.
///
/// # Thread Safety
///
/// `LinkedInClient` is `Send + Sync` and can be shared across threads via `Arc`.
pub struct LinkedInClient {
    /// The underlying reqwest HTTP client with cookie jar enabled.
    http: reqwest::Client,

    /// Cookie jar reference for adding cookies after construction.
    cookie_jar: Arc<reqwest::cookie::Jar>,

    /// JSESSIONID value used for CSRF protection.
    /// Format: `ajax:{19-digit zero-padded random number}`.
    /// Echoed as the `Csrf-Token` header on every request.
    jsessionid: String,

    /// Persistent device identifier (UUID v4).
    /// Used as both the `X-UDID` header and the `deviceId` field in X-LI-Track.
    /// The real app persists this across restarts (but not reinstalls).
    device_id: String,

    /// Pre-serialized X-LI-Track JSON string.
    /// Built once at client creation, matching the main-app format from
    /// `XLiTrackHeader.initJson()`.
    x_li_track: String,

    /// Cached `fsd_profile` URN for the authenticated user, lazily fetched
    /// from the `/me` endpoint on first use. Avoids redundant `/me` calls
    /// when multiple messaging methods need the user's profile URN.
    profile_urn: OnceCell<String>,
}

impl LinkedInClient {
    /// Create a new `LinkedInClient` with auto-generated device identity and CSRF token.
    ///
    /// This generates:
    /// - A random JSESSIONID in the format `ajax:{19-digit zero-padded number}`
    /// - A random UUID v4 for device identification
    /// - A static X-LI-Track JSON blob matching a Google Pixel 8 device
    ///
    /// The JSESSIONID is immediately set as a cookie on the `.linkedin.com` domain
    /// via the cookie jar.
    pub fn new() -> Result<Self, Error> {
        let device_id = uuid::Uuid::new_v4().to_string();
        let jsessionid = generate_jsessionid();
        Self::build(&device_id, &jsessionid, None)
    }

    /// Create a client with a specific device ID and JSESSIONID (for testing or persistence).
    ///
    /// Use this when you want to restore a previous session's device identity
    /// rather than generating a new one.
    pub fn with_identity(device_id: String, jsessionid: String) -> Result<Self, Error> {
        Self::build(&device_id, &jsessionid, None)
    }

    /// Create a client from a persisted [`Session`].
    ///
    /// Uses the session's JSESSIONID and injects the `li_at` cookie into the
    /// cookie jar on the `.linkedin.com` domain. A new device ID is generated
    /// (the session doesn't persist device identity -- that's a separate concern).
    pub fn with_session(session: &Session) -> Result<Self, Error> {
        let device_id = uuid::Uuid::new_v4().to_string();
        Self::build(&device_id, &session.jsessionid, Some(&session.li_at))
    }

    /// Create a client using full browser cookies (from a cookies JSON file).
    ///
    /// This injects all cookies from the browser session, enabling write
    /// operations that require more than just `li_at` + `JSESSIONID`.
    pub fn with_browser_cookies(
        cookies: &std::collections::HashMap<String, String>,
    ) -> Result<Self, Error> {
        let device_id = uuid::Uuid::new_v4().to_string();
        let jsessionid = cookies
            .get("JSESSIONID")
            .cloned()
            .unwrap_or_else(generate_jsessionid);

        let li_at = cookies.get("li_at").map(|s| s.as_str());
        let client = Self::build(&device_id, &jsessionid, li_at)?;

        // Inject all remaining cookies into the jar.
        let base_url: url::Url = BASE_URL.parse().unwrap();
        for (name, value) in cookies {
            if name == "JSESSIONID" || name == "li_at" {
                continue; // already set by build()
            }
            // Quote values that contain special chars.
            let cookie_str = if value.contains(';') || value.contains(',') || value.contains(' ') {
                format!(
                    "{}=\"{}\"; Domain=.linkedin.com; Path=/; Secure",
                    name, value
                )
            } else {
                format!("{}={}; Domain=.linkedin.com; Path=/; Secure", name, value)
            };
            client.cookie_jar.add_cookie_str(&cookie_str, &base_url);
        }

        Ok(client)
    }

    /// Shared client construction logic.
    ///
    /// Builds the reqwest client with cookie jar, default headers matching
    /// the LinkedIn Android app, and seeds the JSESSIONID cookie (plus
    /// optionally the `li_at` session cookie).
    fn build(device_id: &str, jsessionid: &str, li_at: Option<&str>) -> Result<Self, Error> {
        let x_li_track = build_x_li_track(device_id);

        // Build default headers applied to every request.
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            "X-RestLi-Protocol-Version",
            HeaderValue::from_static("2.0.0"),
        );
        default_headers.insert("X-LI-Lang", HeaderValue::from_static("en_US"));
        default_headers.insert("Accept-Language", HeaderValue::from_static("en-US"));
        default_headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        default_headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static("ANDROID OS"),
        );
        default_headers.insert(
            "X-UDID",
            HeaderValue::from_str(device_id)
                .map_err(|e| Error::Auth(format!("invalid device_id header value: {e}")))?,
        );
        default_headers.insert(
            "X-LI-Track",
            HeaderValue::from_str(&x_li_track)
                .map_err(|e| Error::Auth(format!("invalid X-LI-Track header value: {e}")))?,
        );

        // Cookie jar is enabled so reqwest automatically manages cookies
        // across requests (JSESSIONID, li_at, etc.).
        let jar = Arc::new(reqwest::cookie::Jar::default());
        let base_url: url::Url = BASE_URL
            .parse()
            .map_err(|e| Error::Auth(format!("invalid base URL: {e}")))?;

        // Seed the JSESSIONID cookie. The real app generates this client-side
        // via CsrfCookieHelper.generateJsessionId() before the first request.
        jar.add_cookie_str(
            &format!(
                "JSESSIONID=\"{}\"; Domain=.linkedin.com; Path=/; Secure",
                jsessionid
            ),
            &base_url,
        );

        // Set li_at cookie (session authentication) if provided.
        if let Some(li_at_value) = li_at {
            jar.add_cookie_str(
                &format!(
                    "li_at={}; Domain=.linkedin.com; Path=/; Secure",
                    li_at_value
                ),
                &base_url,
            );
        }

        let http = reqwest::Client::builder()
            .cookie_provider(jar.clone())
            .default_headers(default_headers)
            .build()?;

        Ok(Self {
            cookie_jar: jar,
            http,
            jsessionid: jsessionid.to_string(),
            device_id: device_id.to_string(),
            x_li_track,
            profile_urn: OnceCell::new(),
        })
    }

    /// Send a GET request to a Voyager API endpoint.
    ///
    /// The `path` is relative to `/voyager/api/` -- do not include the prefix.
    /// For example, `client.get("me")` requests `https://www.linkedin.com/voyager/api/me`.
    ///
    /// The CSRF token header is added automatically.
    /// Returns an [`Error::Api`] if the server responds with a non-success status code.
    pub async fn get(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}{}", BASE_URL, API_PREFIX, path);
        let resp = self
            .http
            .get(&url)
            .header("Csrf-Token", &self.jsessionid)
            .send()
            .await?;
        check_response(resp).await
    }

    /// Send a POST request to a Voyager API endpoint with a JSON body.
    ///
    /// The `path` is relative to `/voyager/api/` -- do not include the prefix.
    /// The `body` is serialized as JSON in the request body.
    ///
    /// The CSRF token header is added automatically.
    /// Returns an [`Error::Api`] if the server responds with a non-success status code.
    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, Error> {
        let url = format!("{}{}{}", BASE_URL, API_PREFIX, path);
        let resp = self
            .http
            .post(&url)
            .header("Csrf-Token", &self.jsessionid)
            .json(body)
            .send()
            .await?;
        check_response(resp).await
    }

    /// Send a GET request to the Voyager GraphQL endpoint.
    ///
    /// The `query_params` string is appended to `/voyager/api/graphql?`.
    /// This adds the `x-li-graphql-pegasus-client: true` header that LinkedIn
    /// requires for GraphQL requests (discovered in `BaseGraphQLClient.java`).
    ///
    /// The CSRF token header is added automatically.
    /// Returns an [`Error::Api`] if the server responds with a non-success status code.
    /// Also checks for a top-level `errors` array in the JSON response and
    /// returns [`Error::Api`] if present.
    pub async fn graphql_get(&self, query_params: &str) -> Result<Value, Error> {
        let url = format!("{}{}graphql?{}", BASE_URL, API_PREFIX, query_params);
        let resp = self
            .http
            .get(&url)
            .header("Csrf-Token", &self.jsessionid)
            .header("x-li-graphql-pegasus-client", "true")
            .send()
            .await?;
        let json = check_response(resp).await?;
        check_graphql_errors(&json)?;
        Ok(json)
    }

    /// Send a POST request to the Voyager GraphQL endpoint (mutation).
    ///
    /// Unlike [`graphql_get`](Self::graphql_get) which encodes variables into
    /// the URL query string, GraphQL mutations are POSTed with a JSON body
    /// containing `queryId`, `queryName`, and `variables`.
    ///
    /// Discovered from `BaseGraphQLClient.generateRequestBuilder()` in the
    /// decompiled international APK: when `query.isMutation` is true, the
    /// request builder serializes the query into a JSON body instead of
    /// URL parameters.
    ///
    /// The variables are passed as a JSON value (not Rest.li-encoded).
    ///
    /// Returns [`Ok(Value::Null)`] for success responses with empty bodies.
    pub async fn graphql_post(
        &self,
        variables: &Value,
        query_id: &str,
        query_name: &str,
    ) -> Result<Value, Error> {
        // GraphQLMutationRequestBuilder.fillInQueryParams() adds
        // action=execute, queryId, and queryName as URL query parameters.
        let url = format!(
            "{}{}graphql?action=execute&queryId={}&queryName={}",
            BASE_URL, API_PREFIX, query_id, query_name
        );

        let mut body = serde_json::json!({
            "queryId": query_id,
            "queryName": query_name,
        });

        if !variables.is_null() {
            body["variables"] = variables.clone();
        }

        let resp = self
            .http
            .post(&url)
            .header("Csrf-Token", &self.jsessionid)
            .header("x-li-graphql-pegasus-client", "true")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            let text = resp.text().await?;
            if text.is_empty() {
                return Ok(Value::Null);
            }
            let json: Value = serde_json::from_str(&text)?;
            check_graphql_errors(&json)?;
            return Ok(json);
        }

        let status_code = status.as_u16();
        let body_text = resp.text().await.unwrap_or_default();

        if status_code == 401 {
            return Err(Error::Auth(format!(
                "session expired or invalid (HTTP 401): {body_text}"
            )));
        }

        Err(Error::Api {
            status: status_code,
            body: body_text,
        })
    }

    /// Send a GET request to an arbitrary path on the LinkedIn host.
    ///
    /// Unlike [`get`](Self::get), the `path` is NOT prefixed with `/voyager/api/`.
    /// Use this for endpoints outside the Voyager API prefix (e.g., `/uas/authenticate`).
    ///
    /// The `path` should start with `/`. The CSRF token header is added automatically.
    /// Returns an [`Error::Api`] if the server responds with a non-success status code.
    pub async fn api_get(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}", BASE_URL, path);
        let resp = self
            .http
            .get(&url)
            .header("Csrf-Token", &self.jsessionid)
            .send()
            .await?;
        check_response(resp).await
    }

    /// Send a POST request to an arbitrary path on `www.linkedin.com`.
    pub async fn api_post(&self, path: &str, body: &Value) -> Result<Value, Error> {
        let url = format!("{}{}", BASE_URL, path);
        let resp = self
            .http
            .post(&url)
            .header("Csrf-Token", &self.jsessionid)
            .json(body)
            .send()
            .await?;
        check_response(resp).await
    }

    /// Fetch the user's feed (`/voyager/api/feed/updates?q=findFeed`).
    ///
    /// Uses the `q=findFeed` finder with standard pagination parameters.
    /// Returns the raw JSON response which should contain `elements` (array of
    /// `UpdateV2` items) and `paging` (with `start`, `count`, `total`).
    ///
    /// # Parameters
    ///
    /// - `start`: 0-based offset for pagination.
    /// - `count`: Number of feed items to request per page.
    ///
    /// See `re/api_endpoint_catalog.md` section 4 and `re/pegasus_models.md`
    /// for the `UpdateV2` model definition.
    pub async fn get_feed(&self, start: u32, count: u32) -> Result<Value, Error> {
        let path = format!("feed/updates?q=findFeed&start={}&count={}", start, count);
        self.get(&path).await
    }

    /// Fetch a user's full profile by public identifier (vanity URL slug).
    ///
    /// Uses the Voyager GraphQL endpoint with the `identityDashProfilesByMemberIdentity`
    /// finder query from `ProfileGraphQLClient.java` in the decompiled international APK.
    ///
    /// The legacy REST endpoint `identity/profiles/{id}?decorationId=...` returns
    /// HTTP 400 -- the decoration recipes have been removed server-side and profile
    /// fetching has migrated to the Dash/GraphQL surface.
    ///
    /// The `public_id` is the URL slug portion of a LinkedIn profile URL,
    /// e.g. `john-doe-123` from `https://www.linkedin.com/in/john-doe-123`.
    ///
    /// Returns the raw JSON response containing the profile data under
    /// `data.identityDashProfilesByMemberIdentity`.
    ///
    /// See `re/api_endpoint_catalog.md` section 3 and
    /// `re/intl_vs_zephyr_diff.md` for the Dash endpoint migration.
    pub async fn get_profile(&self, public_id: &str) -> Result<Value, Error> {
        // Rest.li-encode the public identifier for safe inclusion in the
        // variables parenthesized record syntax.
        let restli_id = restli_encode_string(public_id);

        // The `identityDashProfilesByMemberIdentity` finder takes a
        // `memberIdentity` variable (the vanity URL slug / public identifier).
        //
        // queryId from ProfileGraphQLClient.java static initializer:
        //   voyagerIdentityDashProfiles.5f50f83f76a1e270603613bdd0fb0252
        let variables = format!("(memberIdentity:{})", restli_id);
        let params = graphql_params(
            &variables,
            "voyagerIdentityDashProfiles.5f50f83f76a1e270603613bdd0fb0252",
            "ProfilesByMemberIdentity",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope and extract first element:
        //   data.identityDashProfilesByMemberIdentity.elements[0]
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
            })
    }

    /// Visit a profile, registering the view so the target sees it in
    /// "Who Viewed My Profile".
    ///
    /// When the LinkedIn web app navigates to a profile, it fires a
    /// `voyagerIdentityDashProfiles` GraphQL query with a specific query ID
    /// that uses `vanityName` (the public identifier / URL slug) as the
    /// variable.  The server registers the profile view as a side effect of
    /// this query -- there is no separate POST endpoint.
    ///
    /// The query ID `a3de77c32c473719f1c58fae6bff43a5` corresponds to the
    /// "profile-top-card-supplementary" decoration used by the web client
    /// (captured via Chrome DevTools MCP on 2026-03-24).
    ///
    /// See `re/profile_visit.md` for the full analysis.
    pub async fn visit_profile(&self, public_id: &str) -> Result<Value, Error> {
        let restli_id = restli_encode_string(public_id);

        // Web client uses `vanityName` variable (not `memberIdentity`).
        // queryId: voyagerIdentityDashProfiles.a3de77c32c473719f1c58fae6bff43a5
        // (profile-top-card-supplementary decoration, triggers view registration)
        let variables = format!("(vanityName:{})", restli_id);
        let params = graphql_params(
            &variables,
            "voyagerIdentityDashProfiles.a3de77c32c473719f1c58fae6bff43a5",
            "ProfilesByMemberIdentity",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope, same structure as get_profile.
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
            })
    }

    /// Fetch the authenticated user's own profile (`/voyager/api/me`).
    ///
    /// This is the simplest authenticated endpoint and serves as a session
    /// validation check. If the `li_at` cookie is expired or invalid, LinkedIn
    /// returns HTTP 401.
    ///
    /// Returns the raw JSON response from the `me` endpoint, which contains
    /// fields like `miniProfile`, `plainId`, `publicContactInfo`, etc.
    pub async fn get_me(&self) -> Result<Value, Error> {
        self.get("me").await
    }

    /// Return the authenticated user's `fsd_profile` URN, fetching and caching
    /// it from `/me` on first call.
    ///
    /// Subsequent calls return the cached value without making a network request.
    /// The URN format is `urn:li:fsd_profile:{opaque_id}` and is extracted from
    /// `miniProfile.dashEntityUrn` in the `/me` response.
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
                    })
            })
            .await
            .map(|s| s.as_str())
    }

    /// Returns the raw reqwest client for advanced use (e.g., auth endpoints
    /// outside the `/voyager/api/` prefix).
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Returns the JSESSIONID value (the CSRF token).
    pub fn jsessionid(&self) -> &str {
        &self.jsessionid
    }

    /// Returns the persistent device ID (UUID v4).
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Returns the pre-built X-LI-Track JSON string.
    pub fn x_li_track(&self) -> &str {
        &self.x_li_track
    }

    /// Returns the base URL (`https://www.linkedin.com`).
    pub fn base_url(&self) -> &str {
        BASE_URL
    }

    /// Fetch the user's messaging conversations.
    ///
    /// Calls `GET /voyager/api/messaging/conversations` with cursor-based
    /// pagination. LinkedIn uses `createdBefore` timestamps for pagination
    /// rather than offset-based `start` parameters.
    ///
    /// # Parameters
    ///
    /// - `count`: Number of conversations to request per page.
    /// - `created_before`: Optional epoch-millis cursor for pagination.
    ///   Omit (pass `None`) for the first page.
    ///
    /// See `re/api_endpoint_catalog.md` section 6 and `re/pegasus_models.md`
    /// for the `Conversation` model definition.
    pub async fn get_conversations(
        &self,
        count: u32,
        created_before: Option<u64>,
    ) -> Result<Value, Error> {
        // The REST endpoint messaging/conversations returns HTTP 500
        // (deprecated server-side for the international build).
        //
        // Use the Messenger GraphQL query `messengerConversationsByCategory`
        // from MessengerGraphQLClient.java in the decompiled code.
        // queryId: voyagerMessagingDashMessengerConversations.7dc50d3efc3953190125aca9c05f0af6
        //
        // Required variables:
        //   mailboxUrn: urn:li:fsd_profile:{member_id}
        //   category: PRIMARY_INBOX (or other category)
        //   count: number of conversations
        // Optional:
        //   lastActivityBefore: epoch-millis cursor for pagination

        // Get the user's fsd_profile URN (cached after first /me call).
        let mailbox_urn = self.my_profile_urn().await?;

        // Rest.li AsciiHex-encode the URN (colons are Rest.li reserved).
        let encoded_urn = restli_encode_string(mailbox_urn);
        let vars = if let Some(ts) = created_before {
            format!(
                "(mailboxUrn:{},category:PRIMARY_INBOX,count:{},lastActivityBefore:{})",
                encoded_urn, count, ts
            )
        } else {
            format!(
                "(mailboxUrn:{},category:PRIMARY_INBOX,count:{})",
                encoded_urn, count
            )
        };
        let params = graphql_params(
            &vars,
            "voyagerMessagingDashMessengerConversations.7dc50d3efc3953190125aca9c05f0af6",
            "MessengerConversationsByCategory",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope:
        //   data.messengerConversationsByCategory
        // which contains { elements, paging }.
        unwrap_graphql(&raw, "messengerConversationsByCategory")
    }

    /// Fetch the user's connections.
    ///
    /// Calls `GET /voyager/api/relationships/connections` with pagination and
    /// sort order. Returns the raw JSON response containing `elements` (array
    /// of `Connection` items) and `paging`.
    ///
    /// # Parameters
    ///
    /// - `start`: 0-based offset for pagination.
    /// - `count`: Number of connections to request per page.
    ///
    /// See `re/api_endpoint_catalog.md` section 8 and `re/pegasus_models.md`
    /// for the `Connection` model definition.
    pub async fn get_connections(&self, start: u32, count: u32) -> Result<Value, Error> {
        let path = format!(
            "relationships/connections?start={}&count={}&sortType=RECENTLY_ADDED",
            start, count
        );
        self.get(&path).await
    }

    /// Search for people by keywords.
    ///
    /// Uses the Voyager GraphQL endpoint with the `searchDashClustersByAll`
    /// query (queryId: `voyagerSearchDashClusters.fae19421cdd51a7cd735e0b7d7b32e0f`).
    ///
    /// The legacy REST endpoint `search/hits?q=guided&guides=List(v->people)`
    /// returns HTTP 404 on the international build -- search has migrated to
    /// GraphQL/Dash. Discovered via decompilation of `SearchGraphQLClient.java`
    /// and `SearchFrameworkUtils.java` in the international APK.
    ///
    /// The variables are Rest.li-encoded (parenthesized record syntax) matching
    /// the `DataEncoder` output from the Android app.
    ///
    /// # Parameters
    ///
    /// - `keywords`: Search terms.
    /// - `start`: 0-based pagination offset.
    /// - `count`: Number of results per page.
    pub async fn search_people(
        &self,
        keywords: &str,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        // Rest.li AsciiHex-encode the keywords: special chars ( ) , ' : and %
        // are escaped as %XX. Then URI-encode the result.
        let restli_keywords = restli_encode_string(keywords);

        // Build the variables in Rest.li record encoding.
        // The SearchQueryForInput record has:
        //   flagshipSearchIntent: SEARCH_SRP (required enum)
        //   keywords: <encoded string>
        //   queryParameters: map with resultType -> List(PEOPLE)
        //
        // The outer variables map has: query, count, origin, start
        let variables = format!(
            "(count:{count},origin:GLOBAL_SEARCH_HEADER,query:(flagshipSearchIntent:SEARCH_SRP,keywords:{restli_keywords},queryParameters:(resultType:List(PEOPLE))),start:{start})"
        );

        let params = graphql_params(
            &variables,
            "voyagerSearchDashClusters.fae19421cdd51a7cd735e0b7d7b32e0f",
            "SearchClusterCollection",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope: data.searchDashClustersByAll contains
        // the collection with `elements`, `paging`, and `metadata`.
        unwrap_graphql(&raw, "searchDashClustersByAll")
    }

    /// Search for jobs by keywords using the Voyager GraphQL
    /// `jobsDashJobCardsByJobSearch` finder.
    ///
    /// Job search uses a dedicated GraphQL query separate from the general
    /// `searchDashClustersByAll` finder (which returns HTTP 501 for JOBS).
    /// Discovered from `CareersGraphQLClient.jobCardsByJobSearch()` in the
    /// decompiled international APK.
    ///
    /// The query variables use `JobSearchQueryForInput` which requires:
    /// - `origin`: a `JobsMatchingOrigin` enum value (we use `FACETED_SEARCH`)
    /// - `keywords`: optional search terms
    /// - `selectedFilters`: optional filter map
    ///
    /// # Parameters
    ///
    /// - `keywords`: Search terms.
    /// - `start`: 0-based pagination offset.
    /// - `count`: Number of results per page.
    pub async fn search_jobs(
        &self,
        keywords: &str,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        let restli_keywords = restli_encode_string(keywords);

        // JobSearchQueryForInput record with required origin and optional keywords.
        let variables = format!(
            "(count:{count},includeJobState:true,query:(keywords:{restli_keywords},origin:FACETED_SEARCH),start:{start})"
        );

        let params = graphql_params(
            &variables,
            "voyagerJobsDashJobCards.4ef915ad5827cd8ea1351ad72f8e4268",
            "JobCardsByJobSearch",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope: data.jobsDashJobCardsByJobSearch
        unwrap_graphql(&raw, "jobsDashJobCardsByJobSearch")
    }

    /// Fetch the user's notification cards.
    ///
    /// Uses the Voyager GraphQL endpoint with the
    /// `identityDashNotificationCardsByFilterVanityName` finder query from
    /// `NotificationsGraphQLClient.java` in the decompiled international APK.
    ///
    /// The legacy REST endpoint `identity/notificationCards` returns HTTP 404
    /// on the international build -- notifications have migrated to the
    /// Dash/GraphQL surface.
    ///
    /// # Parameters
    ///
    /// - `start`: 0-based offset for pagination.
    /// - `count`: Number of notification cards to request per page.
    ///
    /// Returns the collection object (with `elements`, `paging`, `metadata`)
    /// unwrapped from the GraphQL envelope.
    ///
    /// See `re/api_endpoint_catalog.md` section 11 and `re/pegasus_models.md`
    /// section 3.8 for the `Card` (NotificationCard) model definition.
    pub async fn get_notifications(&self, start: u32, count: u32) -> Result<Value, Error> {
        // queryId from NotificationsGraphQLClient.java static initializer:
        //   voyagerIdentityDashNotificationCards.1a1ca07d1f7a6e1033fd88d5fd2da611
        //
        // Variables: start (Integer), count (Integer), filterVanityName (optional String).
        // We omit filterVanityName to get the default "all notifications" view.
        let variables = format!("(start:{},count:{})", start, count);
        let params = graphql_params(
            &variables,
            "voyagerIdentityDashNotificationCards.1a1ca07d1f7a6e1033fd88d5fd2da611",
            "NotificationsCardsByFilterVanityName",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope:
        //   data.identityDashNotificationCardsByFilterVanityName
        // which contains { elements, paging, metadata }.
        unwrap_graphql(&raw, "identityDashNotificationCardsByFilterVanityName")
    }

    /// Send a message to a LinkedIn member.
    ///
    /// Uses the Dash REST endpoint
    /// `POST /voyager/api/voyagerMessagingDashMessengerMessages?action=create`
    /// with the payload format derived from the decompiled international APK's
    /// `CreateMessagePayload`, `MessageComposerImpl.createDirectReplyMessage()`,
    /// and `DeliveryHelperImpl`. This is the newer Dash endpoint that the
    /// international build uses for all messaging.
    ///
    /// Falls back to the legacy REST endpoint
    /// `POST /voyager/api/messaging/conversations?action=create` if the Dash
    /// endpoint returns an error.
    ///
    /// # Parameters
    ///
    /// - `recipient_profile_urn`: The `urn:li:fsd_profile:...` URN of the recipient.
    ///   Can be obtained from `resolve_profile_urn()` or connections list.
    /// - `message_body`: The plain text message to send.
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API (typically contains the created event
    /// or conversation URN).
    ///
    /// See `re/api_endpoint_catalog.md` section 6 for the endpoint documentation.
    pub async fn send_message(
        &self,
        recipient_profile_urn: &str,
        message_body: &str,
    ) -> Result<Value, Error> {
        let origin_token = uuid::Uuid::new_v4().to_string();
        let my_urn = self.my_profile_urn().await?;

        // Captured from live browser traffic via Chrome DevTools MCP.
        // Endpoint: POST /voyager/api/voyagerMessagingDashMessengerMessages?action=createMessage
        // Key discovery: trackingId must be 16 random bytes mapped to chars
        // via `byte as char`. NOT base64-encoded — the messaging endpoint
        // specifically requires raw byte-to-char mapping, unlike
        // send_connection_request which uses base64.
        // See re/send_message.md: "Without this field, or with a UUID/string
        // value, the server returns {"status": 400} with no further details."
        let tracking_bytes: [u8; 16] = rand::random();
        let tracking_id: String = tracking_bytes.iter().map(|&b| b as char).collect();

        let payload = serde_json::json!({
            "message": {
                "body": {
                    "attributes": [],
                    "text": message_body
                },
                "originToken": origin_token,
                "renderContentUnions": []
            },
            "mailboxUrn": my_urn,
            "trackingId": tracking_id,
            "dedupeByClientGeneratedToken": false,
            "hostRecipientUrns": [recipient_profile_urn]
        });

        self.post(
            "voyagerMessagingDashMessengerMessages?action=createMessage",
            &payload,
        )
        .await
    }

    /// Resolve a public identifier (vanity URL slug) to an `fsd_profile` URN.
    ///
    /// Uses the REST `identity/profiles/{public_id}` endpoint (without
    /// decoration) to fetch the profile's `entityUrn`, then converts from
    /// `fs_miniProfile` to `fsd_profile` format if needed.
    ///
    /// Falls back to the GraphQL endpoint if the REST endpoint fails.
    ///
    /// Returns the URN string like `urn:li:fsd_profile:ACoAA...`.
    pub async fn resolve_profile_urn(&self, public_id: &str) -> Result<String, Error> {
        // Try the REST miniProfile endpoint first, which is more reliable
        // than the GraphQL profile endpoint and returns the entityUrn
        // directly.
        let miniprofile_path = format!(
            "identity/miniprofiles?q=memberIdentity&memberIdentity={}",
            public_id
        );
        if let Ok(resp) = self.get(&miniprofile_path).await {
            // Response has elements array with miniProfile objects.
            if let Some(urn) = resp
                .get("elements")
                .and_then(|e| e.as_array())
                .and_then(|arr| arr.first())
                .and_then(|mp| mp.get("dashEntityUrn").or(mp.get("entityUrn")))
                .and_then(|v| v.as_str())
            {
                // Ensure it's an fsd_profile URN.
                let urn = if urn.contains("fs_miniProfile") {
                    urn.replace("fs_miniProfile", "fsd_profile")
                } else {
                    urn.to_string()
                };
                return Ok(urn);
            }
        }

        // Try the REST profile endpoint.
        let profile_path = format!("identity/profiles/{}", public_id);
        if let Ok(resp) = self.get(&profile_path).await {
            // Look for miniProfile.dashEntityUrn or entityUrn.
            if let Some(urn) = resp
                .get("miniProfile")
                .and_then(|mp| mp.get("dashEntityUrn"))
                .and_then(|v| v.as_str())
            {
                return Ok(urn.to_string());
            }
            if let Some(urn) = resp.get("entityUrn").and_then(|v| v.as_str()) {
                let urn = if urn.contains("fs_miniProfile") || urn.contains("fs_profile") {
                    urn.replace("fs_miniProfile", "fsd_profile")
                        .replace("fs_profile", "fsd_profile")
                } else {
                    urn.to_string()
                };
                return Ok(urn);
            }
        }

        // Fall back to GraphQL profile endpoint.
        let profile = self.get_profile(public_id).await?;
        profile
            .get("entityUrn")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Api {
                status: 0,
                body: format!(
                    "could not extract entityUrn from any profile endpoint for '{}'",
                    public_id
                ),
            })
    }

    /// React to a post or activity with a specific reaction type.
    ///
    /// Uses the Dash REST endpoint
    /// `POST /voyager/api/voyagerSocialDashReactions?action=create`
    /// discovered in `FeedFrameworkGraphQLClient.java` in the decompiled
    /// international APK (queryId: `voyagerSocialDashReactions.fd68eadaf15da416b0d839e21399b763`,
    /// operation type: CREATE).
    ///
    /// The payload was reverse-engineered from the `Reaction`,
    /// `ReactionActorForCreate`, and `ReactionType` models in the
    /// `com.linkedin.android.pegasus.dash.gen.voyager.dash.social` package.
    ///
    /// # Parameters
    ///
    /// - `thread_urn`: The URN of the post/activity to react to. This is the
    ///   `urn:li:activity:...` or `urn:li:ugcPost:...` URN from the feed.
    ///   Accepted formats:
    ///   - Full URN: `urn:li:activity:7312345678901234567`
    ///   - Activity ID only: `7312345678901234567` (will be wrapped)
    /// - `reaction_type`: The reaction type string. One of:
    ///   `LIKE`, `PRAISE`, `EMPATHY`, `INTEREST`, `APPRECIATION`,
    ///   `ENTERTAINMENT`, `CELEBRATION`. (See `ReactionType.java` in the
    ///   decompiled APK for the full enum.)
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API. On success LinkedIn typically
    /// returns HTTP 201 with a minimal response body or the created entity.
    ///
    /// See `re/reactions.md` for the full analysis.
    pub async fn react_to_post(
        &self,
        thread_urn: &str,
        reaction_type: &str,
    ) -> Result<Value, Error> {
        let rt = validate_reaction_type(reaction_type)?;

        // Normalize the thread URN: if just an activity ID, wrap it.
        let thread = if thread_urn.starts_with("urn:li:") {
            thread_urn.to_string()
        } else {
            format!("urn:li:activity:{}", thread_urn)
        };

        // Use the GraphQL mutation endpoint discovered in
        // FeedFrameworkGraphQLClient.java (createSocialDashReactions).
        // queryId: voyagerSocialDashReactions.fd68eadaf15da416b0d839e21399b763
        //
        // The variables are JSON (not Rest.li-encoded) for mutations, as
        // determined from BaseGraphQLClient.generateRequestBuilder().
        //
        // The mutation expects a top-level `entity` variable of type
        // `dash_social_ReactionCreateInput!` containing `threadUrn` and
        // `reactionType`. The `actorUnion` with `profileUrn` identifies
        // who is reacting (required for non-personal page reactions).
        //
        // INTENTIONAL DUPLICATION: threadUrn and reactionType appear both at
        // the top level AND inside `entity`. This matches the decompiled
        // Android app's `FeedFrameworkGraphQLClient.java` mutation builder,
        // which populates both levels. The top-level fields are used by the
        // GraphQL variable binding, while `entity` carries the typed input
        // object. Removing either level causes the server to reject the request.
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
    ///
    /// Uses the Dash REST endpoint for deleting reactions, discovered as
    /// `doDeleteReactionSocialDashReactions` in `FeedFrameworkGraphQLClient.java`
    /// (queryId: `voyagerSocialDashReactions.315cef4773de8e3a0ddad7655cc1685f`).
    ///
    /// # Parameters
    ///
    /// - `thread_urn`: The URN of the post/activity to un-react from.
    /// - `reaction_type`: The reaction type to remove.
    ///
    /// See `re/reactions.md` for the full analysis.
    pub async fn unreact_from_post(
        &self,
        thread_urn: &str,
        reaction_type: &str,
    ) -> Result<Value, Error> {
        let rt = validate_reaction_type(reaction_type)?;

        let thread = if thread_urn.starts_with("urn:li:") {
            thread_urn.to_string()
        } else {
            format!("urn:li:activity:{}", thread_urn)
        };

        // Use the GraphQL mutation endpoint for deleting reactions.
        // queryId: voyagerSocialDashReactions.315cef4773de8e3a0ddad7655cc1685f
        // The ACTION mutation `doDeleteReaction` requires threadUrn and
        // reactionType as top-level variables.
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

    /// Comment on a feed post (activity).
    ///
    /// Uses the Dash GraphQL CREATE mutation `createSocialDashNormComments`
    /// discovered in `ConversationsGraphQLClient.java` in the decompiled
    /// international APK (queryId:
    /// `voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e`).
    ///
    /// The mutation variables mirror the `NormCommentForUpdate` model fields:
    /// - `commentary`: `{ text: "..." }` matching `TextViewModel`
    /// - `threadUrn`: The post/activity URN to comment on
    /// - `origin`: `FEED` (standard comment origin)
    ///
    /// # Parameters
    ///
    /// - `post_urn`: The URN of the post/activity to comment on. Accepted formats:
    ///   - Full URN: `urn:li:activity:7312345678901234567`
    ///   - Activity ID only: `7312345678901234567` (will be wrapped)
    /// - `text`: The comment text content.
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API. On success LinkedIn typically
    /// returns the created comment entity.
    ///
    /// # Warning
    ///
    /// This creates a **real comment** on a LinkedIn post. There is no
    /// draft/preview mode.
    ///
    /// See `re/comments.md` for the full analysis.
    pub async fn comment_on_post(&self, post_urn: &str, text: &str) -> Result<Value, Error> {
        // Normalize the post URN: if just an activity ID, wrap it.
        let thread = if post_urn.starts_with("urn:li:") {
            post_urn.to_string()
        } else {
            format!("urn:li:activity:{}", post_urn)
        };

        // Build the mutation variables matching the NormComment model.
        // The `entity` field contains the comment data for the CREATE mutation,
        // structured as a NormCommentForCreate record:
        // - commentary: TextViewModel with the comment text
        // - threadUrn: the post being commented on
        // - origin: FEED (standard origin for feed comments)
        //
        // queryId: voyagerSocialDashNormComments.cd3d2a3fd6c9b2881c7cac32847ec05e
        // from ConversationsGraphQLClient.java static initializer.
        let variables = serde_json::json!({
            "entity": {
                "commentary": {
                    "text": text
                },
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

    /// Fetch "who viewed my profile" data.
    ///
    /// Calls `GET /voyager/api/identity/wvmpCards` which returns the WVMP
    /// (Who Viewed My Profile) cards. This is a legacy REST endpoint that
    /// still works without Premium.
    ///
    /// The response uses deeply nested Rest.li union encoding with viewer
    /// details (names, occupations), view count change percentage, and
    /// aggregated/anonymous viewer entries.
    ///
    /// Returns the raw JSON response. The caller is responsible for
    /// unwrapping the nested union structure.
    ///
    /// See `re/profile_viewers.md` for the endpoint documentation and
    /// response structure.
    pub async fn get_profile_viewers(&self) -> Result<Value, Error> {
        self.get("identity/wvmpCards").await
    }

    /// Create a new text-only post (share) on the authenticated user's feed.
    ///
    /// Uses the Dash GraphQL CREATE mutation `createContentcreationDashShares`
    /// discovered in `SharingGraphQLClient.java` in the decompiled international
    /// APK (queryId: `voyagerContentcreationDashShares.f8a4f57de961be2d370fbcc862e867cf`).
    ///
    /// The mutation variables mirror the `ShareData` model fields from
    /// `com.linkedin.android.sharing.compose.dash.ShareData`:
    /// - `visibilityType`: `ANYONE` (public) or `CONNECTIONS_ONLY`
    /// - `origin`: `FEED` (standard compose flow)
    /// - `allowedScope`: `NONE` (no container/group context)
    /// - `shareText`: `{ text: "..." }` matching `TextViewModel`
    /// - `shareMediaForCreate`: absent (text-only, no media)
    /// - `shareVisibility`: `0` (standard visibility enum ordinal)
    ///
    /// # Parameters
    ///
    /// - `text`: The post body text.
    /// - `visibility`: Visibility setting. One of `ANYONE` or `CONNECTIONS_ONLY`.
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API. On success LinkedIn returns the
    /// created `Share` entity with `entityUrn` and `status`.
    ///
    /// # Warning
    ///
    /// This creates a **real public post** on your LinkedIn account. There is
    /// no draft/preview mode. Use with extreme caution.
    ///
    /// See `re/create_post.md` for the full analysis.
    pub async fn create_post(&self, text: &str, visibility: &str) -> Result<Value, Error> {
        // Visibility is not validated here -- the server will reject invalid
        // values. Callers (e.g. the CLI) should validate before calling if
        // they want a friendlier error message.
        let vis = visibility.to_uppercase();

        // Build the mutation variables matching the ShareData model structure.
        // The entity is the "input" parameter for the CREATE mutation.
        // Captured from live browser traffic via Chrome DevTools MCP.
        // The web client sends variables + queryId in the POST body (not URL params).
        // Key differences from the mobile APK:
        // - Top-level key is "post" (not "entity")
        // - Uses "attributesV2" (not "attributes")
        // - visibilityDataUnion wraps "visibilityType" (not "visibilityTypeValue")
        let body = serde_json::json!({
            "variables": {
                "post": {
                    "allowedCommentersScope": "ALL",
                    "intendedShareLifeCycleState": "PUBLISHED",
                    "origin": "FEED",
                    "visibilityDataUnion": {
                        "visibilityType": vis
                    },
                    "commentary": {
                        "text": text,
                        "attributesV2": []
                    }
                }
            },
            "queryId": "voyagerContentcreationDashShares.279996efa5064c01775d5aff003d9377",
            "includeWebMetadata": true
        });

        // The web-style mutation format sends variables+queryId in the POST
        // body (not just URL params). We still include queryId in URL params
        // as the server uses both. The `x-li-graphql-pegasus-client` header
        // is required for all GraphQL requests.
        let url = format!(
            "{}{}graphql?action=execute&queryId=voyagerContentcreationDashShares.279996efa5064c01775d5aff003d9377",
            BASE_URL, API_PREFIX
        );
        let resp = self
            .http
            .post(&url)
            .header("Csrf-Token", &self.jsessionid)
            .header("x-li-graphql-pegasus-client", "true")
            .json(&body)
            .send()
            .await?;
        let json = check_response(resp).await?;
        check_graphql_errors(&json)?;
        Ok(json)
    }

    /// Fetch events (messages) within a specific conversation.
    ///
    /// Calls the Voyager GraphQL endpoint to fetch messages for a conversation.
    /// Uses the `messengerMessagesByConversation` query from
    /// `MessengerGraphQLClient.java`.
    ///
    /// # Parameters
    ///
    /// - `conversation_urn`: The conversation URN or ID. If a plain ID like
    ///   `2-abc123` is provided, it is wrapped as
    ///   `urn:li:messagingThread:2-abc123`.
    /// - `created_before`: Optional epoch-millis cursor for pagination.
    ///   Omit for the most recent messages.
    pub async fn get_conversation_events(
        &self,
        conversation_urn: &str,
        created_before: Option<u64>,
    ) -> Result<Value, Error> {
        // The GraphQL query expects a `msg_conversation` URN format:
        //   urn:li:msg_conversation:(urn:li:fsd_profile:XXXX,<thread_id>)
        // If a plain thread ID or messagingThread URN is provided,
        // we need the user's fsd_profile URN to construct it.
        let full_urn = if conversation_urn.starts_with("urn:li:msg_conversation:") {
            conversation_urn.to_string()
        } else {
            // Extract the thread ID.
            let thread_id = conversation_urn
                .strip_prefix("urn:li:messagingThread:")
                .unwrap_or(conversation_urn);

            // Get the user's fsd_profile URN (cached after first /me call).
            let profile_urn = self.my_profile_urn().await?;

            format!("urn:li:msg_conversation:({},{})", profile_urn, thread_id)
        };
        // Rest.li AsciiHex-encode the URN for the variables record.
        let encoded_urn = restli_encode_string(&full_urn);

        // Use the Messenger GraphQL query `messengerMessagesByConversation`
        // from MessengerGraphQLClient.java.
        // queryId: voyagerMessagingDashMessengerMessages.7cde5843a127bbecc3de900d3894a74a
        let variables = if let Some(ts) = created_before {
            format!("(conversationUrn:{},deliveredBefore:{})", encoded_urn, ts)
        } else {
            format!("(conversationUrn:{})", encoded_urn)
        };
        let params = graphql_params(
            &variables,
            "voyagerMessagingDashMessengerMessages.7cde5843a127bbecc3de900d3894a74a",
            "MessengerMessagesByConversation",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope:
        //   data.messengerMessagesByConversation
        // which contains { elements, paging }.
        unwrap_graphql(&raw, "messengerMessagesByConversation")
    }

    /// Send a connection request (invitation) to another LinkedIn member.
    ///
    /// Uses the Voyager `normInvitations` REST endpoint discovered in the
    /// decompiled China APK's `InvitationNetworkUtil.sendInvite()` method.
    /// The route is `Routes.NORM_INVITATIONS` which maps to
    /// `voyagerGrowthNormInvitations` (see `MyNetworkRoutesUtil.makeSendGrowthInvitationRoute()`).
    ///
    /// The international APK uses the Dash variant at
    /// `voyagerRelationshipsDashInvitations?action=create`, but the legacy
    /// `normInvitations` endpoint is confirmed to still work on production.
    ///
    /// The request body is a `NormInvitation` model (see
    /// `MyNetworkRequestUtil.buildInvitation()` in the decompiled code):
    /// ```json
    /// {
    ///   "invitee": {
    ///     "com.linkedin.voyager.growth.invitation.InviteeProfile": {
    ///       "profileId": "<member_id>"
    ///     }
    ///   },
    ///   "trackingId": "<base64-encoded-16-random-bytes>",
    ///   "message": "optional custom message"
    /// }
    /// ```
    ///
    /// # Parameters
    ///
    /// - `profile_urn`: The target member's profile URN
    ///   (e.g. `urn:li:fsd_profile:ACoAAA...`). The member ID is extracted
    ///   from the URN automatically.
    /// - `message`: Optional custom message to include with the invitation.
    ///   LinkedIn limits this to ~300 characters; we do not enforce that here
    ///   (the server will reject messages that are too long).
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API. On success LinkedIn returns the
    /// created invitation entity.
    ///
    /// See `re/connection_request.md` for the full endpoint analysis.
    pub async fn send_connection_request(
        &self,
        profile_urn: &str,
        message: Option<&str>,
    ) -> Result<Value, Error> {
        // Extract the member ID from the URN. The NormInvitation model uses
        // a bare profileId (the part after the last colon in the URN), not
        // the full URN.
        let member_id = profile_urn.rsplit(':').next().unwrap_or(profile_urn);

        // Generate a tracking ID: 16 random bytes, base64-encoded.
        // This matches TrackingUtils.generateBase64EncodedTrackingId() in the
        // decompiled code.
        use base64::Engine;
        let tracking_bytes: [u8; 16] = rand::random();
        let tracking_id = base64::engine::general_purpose::STANDARD.encode(tracking_bytes);

        // Build the NormInvitation payload matching the model from
        // com.linkedin.android.pegasus.gen.voyager.growth.invitation.NormInvitation.
        // The invitee is a Rest.li union, requiring the fully-qualified type key.
        let mut payload = serde_json::json!({
            "trackingId": tracking_id,
            "invitee": {
                "com.linkedin.voyager.growth.invitation.InviteeProfile": {
                    "profileId": member_id
                }
            }
        });

        // Add optional custom message.
        if let Some(msg) = message {
            payload.as_object_mut().unwrap().insert(
                "message".to_string(),
                serde_json::Value::String(msg.to_string()),
            );
        }

        self.post("voyagerGrowthNormInvitations", &payload).await
    }

    /// Fetch pending (received) connection invitations.
    ///
    /// Uses the Dash GraphQL endpoint `voyagerRelationshipsDashInvitationViews`
    /// with the `ReceivedInvitationViews` finder query discovered in
    /// `MynetworkGraphQLClient.receivedInvitationViews()` in the decompiled
    /// international APK.
    ///
    /// queryId: `voyagerRelationshipsDashInvitationViews.48949225027e0a85d063176777f08e7f`
    ///
    /// Variables:
    /// - `start` (Integer): pagination offset
    /// - `count` (Integer): page size
    /// - `includeInsights` (Boolean): always `true` to get connection insights
    ///
    /// The response contains `InvitationView` elements, each with an embedded
    /// `invitation` object containing `entityUrn`, `sharedSecret`,
    /// `genericInvitationType`, `inviter` (profile), `message`, `sentTime`.
    ///
    /// # Parameters
    ///
    /// - `start`: 0-based offset for pagination.
    /// - `count`: Number of invitations to request per page.
    ///
    /// Returns the collection object (with `elements`, `paging`) unwrapped
    /// from the GraphQL envelope.
    ///
    /// See `re/invitations.md` for the endpoint documentation and response
    /// structure.
    pub async fn get_invitations(&self, start: u32, count: u32) -> Result<Value, Error> {
        let variables = format!("(start:{},count:{},includeInsights:true)", start, count);
        let params = graphql_params(
            &variables,
            "voyagerRelationshipsDashInvitationViews.48949225027e0a85d063176777f08e7f",
            "ReceivedInvitationViews",
        );
        let raw = self.graphql_get(&params).await?;

        // Unwrap the GraphQL envelope:
        //   data.relationshipsDashInvitationViewsByReceived
        // which contains { elements, paging }.
        unwrap_graphql(&raw, "relationshipsDashInvitationViewsByReceived")
    }

    /// Accept a pending connection invitation.
    ///
    /// Uses the Dash REST endpoint discovered in
    /// `InvitationActionsRepository.Companion.buildInvitationActionRoute()`:
    ///
    /// ```text
    /// POST /voyager/api/voyagerRelationshipsDashInvitations/{invitation_urn}?action=accept
    /// ```
    ///
    /// The invitation URN and shared secret are obtained from the invitation
    /// list response (`get_invitations`). The `sharedSecret` is required by
    /// LinkedIn to prevent CSRF on invitation acceptance.
    ///
    /// # Parameters
    ///
    /// - `invitation_urn`: The full invitation URN from the invitation's
    ///   `entityUrn` field (e.g. `urn:li:fsd_invitation:7...`).
    /// - `shared_secret`: The `sharedSecret` string from the invitation object.
    ///
    /// # Returns
    ///
    /// The raw JSON response from the API. On success LinkedIn typically
    /// returns the updated invitation or member relationship entity.
    ///
    /// See `re/invitations.md` for the full endpoint analysis.
    pub async fn accept_invitation(
        &self,
        invitation_urn: &str,
        shared_secret: &str,
    ) -> Result<Value, Error> {
        // The Android app uses `appendEncodedPath(urn.toString())` which
        // passes the URN as-is. Colons in URNs are valid in URL path segments
        // per RFC 3986.
        let path = format!(
            "voyagerRelationshipsDashInvitations/{}?action=accept",
            invitation_urn
        );

        // The accept action requires the invitation URN and shared secret
        // in the request body. The decompiled code shows the Dash endpoint
        // expects a minimal body with the invitation identification.
        let body = serde_json::json!({
            "invitationUrn": invitation_urn,
            "sharedSecret": shared_secret
        });

        self.post(&path, &body).await
    }
}

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

/// Validate a reaction type string against the known enum values.
///
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

/// Unwrap the standard GraphQL response envelope: `data.{data_key}`.
///
/// LinkedIn's Voyager GraphQL responses consistently wrap the payload under
/// `{ "data": { "<finderName>": <payload> } }`. This helper extracts the
/// payload and returns a clear error when the expected shape is missing.
fn unwrap_graphql(raw: &Value, data_key: &str) -> Result<Value, Error> {
    raw.get("data")
        .and_then(|d| d.get(data_key))
        .cloned()
        .ok_or_else(|| Error::Api {
            status: 0,
            body: format!(
                "unexpected GraphQL response shape (missing data.{}): {}",
                data_key,
                serde_json::to_string(raw).unwrap_or_default()
            ),
        })
}

/// Check a GraphQL JSON response for a top-level `errors` array and return
/// an error if any are present. GraphQL can return HTTP 200 with logical
/// errors in the response body.
fn check_graphql_errors(json: &Value) -> Result<(), Error> {
    if let Some(errors) = json.get("errors").and_then(|e| e.as_array()) {
        if !errors.is_empty() {
            let messages: Vec<&str> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect();
            return Err(Error::Api {
                status: 200,
                body: format!("GraphQL errors: {}", messages.join("; ")),
            });
        }
    }
    Ok(())
}

/// Build a GraphQL query parameter string for the Voyager GraphQL endpoint.
///
/// Combines the `variables` (Rest.li parenthesized record), `query_id`, and
/// `query_name` into the `variables=...&queryId=...&queryName=...` format
/// expected by `/voyager/api/graphql`.
fn graphql_params(variables: &str, query_id: &str, query_name: &str) -> String {
    format!(
        "variables={}&queryId={}&queryName={}",
        variables, query_id, query_name
    )
}

/// Encode a string using Rest.li's AsciiHex encoding for use in Rest.li
/// parenthesized record variables within a URL query parameter.
///
/// Rest.li reserves five characters: `( ) , ' :` plus the escape character `%`.
/// Each reserved char is replaced by `%XX` (uppercase hex of the ASCII code).
///
/// After AsciiHex encoding, characters that are unsafe in URL query strings
/// (spaces, etc.) are percent-encoded. The `%` signs introduced by AsciiHex
/// are NOT double-encoded because they are already valid percent-encoding.
///
/// Empty strings are encoded as `''` (two single quotes) per Rest.li convention.
///
/// Source: `DataEncoder.processString()` and `AsciiHexEncoding` in the
/// decompiled LinkedIn Android app (`com.linkedin.data.lite.restli`).
fn restli_encode_string(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut buf = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            // Rest.li reserved characters: AsciiHex-encode them.
            b'(' | b')' | b',' | b'\'' | b':' | b'%' => {
                buf.push('%');
                buf.push_str(&format!("{:02X}", byte));
            }
            // Characters that are safe in URL query strings unencoded.
            // RFC 3986 unreserved: ALPHA / DIGIT / "-" / "." / "_" / "~"
            // Plus a few sub-delimiters safe in query values: ! * @ /
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'!'
            | b'*'
            | b'@'
            | b'/' => {
                buf.push(byte as char);
            }
            // Everything else (spaces, non-ASCII bytes, etc.): percent-encode.
            _ => {
                buf.push('%');
                buf.push_str(&format!("{:02X}", byte));
            }
        }
    }
    buf
}

/// Generate a JSESSIONID in the format used by the LinkedIn Android app.
///
/// Format: `ajax:{19-digit zero-padded positive random number}`
///
/// Source: `CsrfCookieHelper.generateJsessionId()` in `com.linkedin.android.networking.cookies`.
///
/// Algorithm:
/// 1. Generate a random i64 via `SecureRandom.nextLong()`
/// 2. Take `abs()`, special-casing `i64::MIN` to `i64::MAX`
/// 3. Format as 19-digit zero-padded decimal
/// 4. Prefix with `"ajax:"`
fn generate_jsessionid() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let raw: i64 = rng.random();
    let value = if raw == i64::MIN {
        i64::MAX
    } else {
        raw.unsigned_abs() as i64
    };
    format!("ajax:{:019}", value)
}

/// Build the X-LI-Track JSON string for authenticated API requests.
///
/// This matches the main-app format from `XLiTrackHeader.initJson()`, not the
/// simplified auth-library variant. The auth-library variant is used only during
/// login/registration (see `re/device_fingerprinting.md` section 1.5).
///
/// Uses realistic static values for a Google Pixel 8 on Android 14.
fn build_x_li_track(device_id: &str) -> String {
    // Compute timezone offset from UTC in hours.
    let tz_offset_hours: f64 = {
        let now = chrono::Local::now();
        let offset_secs = now.offset().local_minus_utc();
        offset_secs as f64 / 3600.0
    };

    // Attempt to get the IANA timezone ID.
    // chrono doesn't expose this directly, so we use the iana-time-zone crate
    // which is already a transitive dependency of chrono.
    let timezone = iana_time_zone::get_timezone().unwrap_or_default();

    let track = serde_json::json!({
        "osName": "Android OS",
        "osVersion": "34",
        "clientVersion": CLIENT_VERSION,
        "clientMinorVersion": CLIENT_MINOR_VERSION,
        "carrier": "T-Mobile",
        "model": "Google_Pixel 8",
        "displayDensity": 2.625,
        "displayWidth": 1080,
        "displayHeight": 2400,
        "dpi": "xxhdpi",
        "deviceType": "android",
        "appId": "com.linkedin.android",
        "deviceId": device_id,
        "timezoneOffset": tz_offset_hours,
        "timezone": timezone,
        "storeId": "com.linkedin.android",
        "advertiserId": "00000000-0000-0000-0000-000000000000",
        "isAdTrackingLimited": true,
        "mpName": "voyager-android",
        "mpVersion": CLIENT_VERSION,
    });

    track.to_string()
}

/// Check an HTTP response for error status codes and parse the body as JSON.
///
/// On success (2xx), parses the response body as JSON and returns it.
/// On 401, returns [`Error::Auth`] with the response body for context.
/// On other non-success status codes, returns [`Error::Api`].
async fn check_response(resp: reqwest::Response) -> Result<Value, Error> {
    let status = resp.status();
    if status.is_success() {
        let json = resp.json::<Value>().await?;
        return Ok(json);
    }

    let status_code = status.as_u16();
    let body = resp.text().await.unwrap_or_default();

    if status_code == 401 {
        return Err(Error::Auth(format!(
            "session expired or invalid (HTTP 401): {body}"
        )));
    }

    Err(Error::Api {
        status: status_code,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsessionid_format() {
        let id = generate_jsessionid();
        assert!(id.starts_with("ajax:"), "must start with 'ajax:': {id}");
        let numeric_part = &id["ajax:".len()..];
        assert_eq!(
            numeric_part.len(),
            19,
            "numeric part must be 19 digits: {numeric_part}"
        );
        assert!(
            numeric_part.chars().all(|c| c.is_ascii_digit()),
            "numeric part must be all digits: {numeric_part}"
        );
        // Must be parseable as a non-negative i64
        let parsed: i64 = numeric_part.parse().expect("must parse as i64");
        assert!(parsed >= 0, "value must be non-negative: {parsed}");
    }

    #[test]
    fn jsessionid_uniqueness() {
        let a = generate_jsessionid();
        let b = generate_jsessionid();
        assert_ne!(a, b, "two generated JSESSIONIDs should differ");
    }

    #[test]
    fn x_li_track_has_required_fields() {
        let device_id = "test-uuid-1234";
        let track_json = build_x_li_track(device_id);
        let track: Value = serde_json::from_str(&track_json).expect("must be valid JSON");

        assert_eq!(track["osName"], "Android OS");
        assert_eq!(track["osVersion"], "34");
        assert_eq!(track["clientVersion"], CLIENT_VERSION);
        assert_eq!(track["clientMinorVersion"], CLIENT_MINOR_VERSION);
        assert_eq!(track["model"], "Google_Pixel 8");
        assert_eq!(track["deviceType"], "android");
        assert_eq!(track["appId"], "com.linkedin.android");
        assert_eq!(track["deviceId"], device_id);
        assert_eq!(track["dpi"], "xxhdpi");
        assert_eq!(track["displayWidth"], 1080);
        assert_eq!(track["displayHeight"], 2400);
        assert_eq!(track["storeId"], "com.linkedin.android");
        assert_eq!(track["isAdTrackingLimited"], true);
        assert_eq!(track["mpName"], "voyager-android");
        assert_eq!(track["mpVersion"], CLIENT_VERSION);

        // timezoneOffset and timezone must be present
        assert!(
            track["timezoneOffset"].is_number(),
            "timezoneOffset must be a number"
        );
        assert!(track["timezone"].is_string(), "timezone must be a string");
    }

    #[test]
    fn x_li_track_device_id_matches_udid() {
        // The RE docs require X-UDID == X-LI-Track.deviceId
        let device_id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let track_json = build_x_li_track(device_id);
        let track: Value = serde_json::from_str(&track_json).unwrap();
        assert_eq!(track["deviceId"], device_id);
    }

    #[test]
    fn client_construction() {
        // Verify the client can be constructed without panicking.
        let client = LinkedInClient::new().expect("client creation must succeed");
        assert!(client.jsessionid().starts_with("ajax:"));
        assert!(!client.device_id().is_empty());
        assert!(!client.x_li_track().is_empty());
        assert_eq!(client.base_url(), "https://www.linkedin.com");
    }

    #[test]
    fn restli_encode_empty_string() {
        assert_eq!(restli_encode_string(""), "''");
    }

    #[test]
    fn restli_encode_plain_string() {
        // No special chars -- should pass through unchanged.
        assert_eq!(restli_encode_string("john-doe-123"), "john-doe-123");
    }

    #[test]
    fn restli_encode_reserved_chars() {
        // Each Rest.li reserved character should be AsciiHex-encoded.
        assert_eq!(restli_encode_string("("), "%28");
        assert_eq!(restli_encode_string(")"), "%29");
        assert_eq!(restli_encode_string(","), "%2C");
        assert_eq!(restli_encode_string("'"), "%27");
        assert_eq!(restli_encode_string(":"), "%3A");
        assert_eq!(restli_encode_string("%"), "%25");
    }

    #[test]
    fn restli_encode_urn() {
        // URNs contain colons which are Rest.li reserved.
        // Should NOT double-encode (no %253A).
        let encoded = restli_encode_string("urn:li:fsd_profile:abc123");
        assert_eq!(encoded, "urn%3Ali%3Afsd_profile%3Aabc123");
        // Verify no double-encoding: should not contain %25.
        assert!(
            !encoded.contains("%253A"),
            "must not double-encode: {encoded}"
        );
    }

    #[test]
    fn restli_encode_spaces() {
        // Spaces should be percent-encoded for URL safety.
        let encoded = restli_encode_string("hello world");
        assert_eq!(encoded, "hello%20world");
    }

    #[test]
    fn restli_encode_mixed() {
        // Mixed content: keyword with apostrophe.
        let encoded = restli_encode_string("O'Brien");
        assert_eq!(encoded, "O%27Brien");
    }

    #[test]
    fn client_with_identity() {
        let client = LinkedInClient::with_identity(
            "my-device-id".to_string(),
            "ajax:0000000000000000001".to_string(),
        )
        .expect("client creation must succeed");
        assert_eq!(client.device_id(), "my-device-id");
        assert_eq!(client.jsessionid(), "ajax:0000000000000000001");
    }
}
