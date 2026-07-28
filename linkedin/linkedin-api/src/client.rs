//! HTTP client wrapper for LinkedIn API requests.
//!
//! Implements the LinkedIn Android app's HTTP client behavior, including:
//! - Cookie jar with JSESSIONID (CSRF token) auto-generation
//! - X-LI-Track device telemetry header (main-app format)
//! - X-UDID persistent device identifier
//! - X-RestLi-Protocol-Version 2.0.0
//! - Csrf-Token header echoing the JSESSIONID cookie value
//!
//! Per-resource API methods live in submodules:
//! `feed`, `profile`, `messaging`, `connections`, `search`,
//! `notifications`, `company`, `events`. They each `impl LinkedInClient`
//! to add resource-specific methods to the type defined here.
//!
//! Reference: `re/architecture_overview.md`, `re/device_fingerprinting.md`,
//! `re/restli_protocol.md`, `re/auth_flow.md`.

mod company;
mod connections;
mod events;
mod feed;
mod internal;
mod messaging;
mod notifications;
mod profile;
mod search;

use internal::{check_graphql_errors, check_response};

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use tokio::sync::{Mutex, OnceCell};

use crate::auth::Session;
use crate::error::Error;

/// Maximum number of automatic retries for retriable errors (429, 5xx).
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (milliseconds).
const BASE_BACKOFF_MS: u64 = 1000;

/// Maximum backoff delay cap (milliseconds).
const MAX_BACKOFF_MS: u64 = 30_000;

/// Default minimum interval between API requests (milliseconds).
/// 1 request per second keeps well under LinkedIn's rate limits.
const DEFAULT_MIN_REQUEST_INTERVAL_MS: u64 = 1000;

/// Base URL for all LinkedIn API requests.
pub(super) const BASE_URL: &str = "https://www.linkedin.com";

/// API path prefix for the international variant (Voyager).
pub(super) const API_PREFIX: &str = "/voyager/api/";

/// LinkedIn app version to impersonate.
const CLIENT_VERSION: &str = "4.2.1058";

/// Numeric build/version code corresponding to CLIENT_VERSION.
const CLIENT_MINOR_VERSION: i64 = 562100;

/// HTTP client configured to impersonate the LinkedIn Android app.
///
/// Holds a `reqwest::Client` with cookie jar, persistent device identity,
/// and pre-built headers that match the Android app's networking stack.
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
    jsessionid: String,

    /// Persistent device identifier (UUID v4).
    device_id: String,

    /// Pre-serialized X-LI-Track JSON string.
    x_li_track: String,

    /// Cached `fsd_profile` URN for the authenticated user, lazily fetched
    /// from the `/me` endpoint on first use.
    pub(super) profile_urn: OnceCell<String>,

    /// Cached vanity slug (`publicIdentifier`) for the authenticated user.
    ///
    /// Filled for free whenever `/me` is fetched for any other reason, and
    /// seedable offline by the caller (see
    /// [`LinkedInClient::set_self_public_id`]) so that a self-slug can be
    /// recognised without spending a request on it.
    pub(super) self_public_id: OnceCell<String>,

    /// Timestamp of the last API request, used for rate limiting.
    last_request: Mutex<Instant>,

    /// Minimum interval between requests (milliseconds).
    min_request_interval: Duration,
}

impl LinkedInClient {
    /// Create a new `LinkedInClient` with auto-generated device identity and
    /// CSRF token.
    pub fn new() -> Result<Self, Error> {
        let device_id = uuid::Uuid::new_v4().to_string();
        let jsessionid = generate_jsessionid();
        Self::build(&device_id, &jsessionid, None)
    }

    /// Create a client with a specific device ID and JSESSIONID.
    pub fn with_identity(device_id: String, jsessionid: String) -> Result<Self, Error> {
        Self::build(&device_id, &jsessionid, None)
    }

    /// Create a client from a persisted [`Session`].
    ///
    /// Seeds the self-slug cache from the session file when it carries one,
    /// so a self-slug is recognisable without a `/me` round trip.
    pub fn with_session(session: &Session) -> Result<Self, Error> {
        let device_id = uuid::Uuid::new_v4().to_string();
        let client = Self::build(&device_id, &session.jsessionid, Some(&session.li_at))?;
        if let Some(public_id) = session.self_public_id.as_deref() {
            client.set_self_public_id(public_id);
        }
        Ok(client)
    }

    /// Create a client using full browser cookies (from a cookies JSON file).
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

        let base_url: url::Url = BASE_URL.parse().unwrap();
        for (name, value) in cookies {
            if name == "JSESSIONID" || name == "li_at" {
                continue;
            }
            if !is_valid_cookie_name(name) {
                // Reject names with control chars, whitespace, or cookie-attribute
                // separators (`;`, `=`, `,`, etc.). A malformed name in an
                // untrusted cookies file could otherwise inject extra
                // attributes into the Set-Cookie string.
                continue;
            }
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
    fn build(device_id: &str, jsessionid: &str, li_at: Option<&str>) -> Result<Self, Error> {
        let x_li_track = build_x_li_track(device_id);
        let default_headers = build_default_headers(device_id, &x_li_track)?;

        let jar = Arc::new(reqwest::cookie::Jar::default());
        let base_url: url::Url = BASE_URL
            .parse()
            .map_err(|e| Error::Auth(format!("invalid base URL: {e}")))?;

        jar.add_cookie_str(
            &format!(
                "JSESSIONID=\"{}\"; Domain=.linkedin.com; Path=/; Secure",
                jsessionid
            ),
            &base_url,
        );
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
            self_public_id: OnceCell::new(),
            last_request: Mutex::new(Instant::now() - Duration::from_secs(10)),
            min_request_interval: Duration::from_millis(DEFAULT_MIN_REQUEST_INTERVAL_MS),
        })
    }

    /// Wait until the minimum interval since the last request has elapsed.
    async fn throttle(&self) {
        let mut last = self.last_request.lock().await;
        let elapsed = last.elapsed();
        if elapsed < self.min_request_interval {
            tokio::time::sleep(self.min_request_interval - elapsed).await;
        }
        *last = Instant::now();
    }

    /// Send a GET request to a Voyager API endpoint with automatic retry on
    /// 429 / 5xx, respecting `Retry-After` when present.
    pub async fn get(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}{}", BASE_URL, API_PREFIX, path);
        self.send_with_retry(|| self.csrf_get(&url)).await
    }

    /// Send a POST request to a Voyager API endpoint with a JSON body and
    /// automatic retry on 429 / 5xx.
    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, Error> {
        let url = format!("{}{}{}", BASE_URL, API_PREFIX, path);
        self.send_with_retry(|| self.csrf_post(&url).json(body))
            .await
    }

    /// Build a GET RequestBuilder with the standard `Csrf-Token` header.
    fn csrf_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http.get(url).header("Csrf-Token", &self.jsessionid)
    }

    /// Build a POST RequestBuilder with the standard `Csrf-Token` header.
    fn csrf_post(&self, url: &str) -> reqwest::RequestBuilder {
        self.http.post(url).header("Csrf-Token", &self.jsessionid)
    }

    /// Run a request through the throttle + retry loop. The closure is
    /// re-invoked on each retry attempt to build a fresh `RequestBuilder`
    /// (the prior one is consumed by `.send()`).
    async fn send_with_retry<F>(&self, build: F) -> Result<Value, Error>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut attempt = 0u32;
        loop {
            self.throttle().await;
            let resp = build().send().await?;
            match check_response_retryable(resp, attempt).await {
                RetryResult::Ok(val) => return Ok(val),
                RetryResult::Err(e) => return Err(e),
                RetryResult::Retry(delay) => {
                    attempt += 1;
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Send a GET request to the Voyager GraphQL endpoint.
    pub async fn graphql_get(&self, query_params: &str) -> Result<Value, Error> {
        let url = format!("{}{}graphql?{}", BASE_URL, API_PREFIX, query_params);
        let mut attempt = 0u32;
        loop {
            self.throttle().await;
            let resp = self
                .http
                .get(&url)
                .header("Csrf-Token", &self.jsessionid)
                .header("x-li-graphql-pegasus-client", "true")
                .send()
                .await?;
            let json = check_response(resp).await?;
            match check_graphql_errors(&json) {
                Ok(()) => return Ok(json),
                Err(e) if is_retriable_graphql_error(&e) && attempt < MAX_RETRIES => {
                    let delay = graphql_retry_delay(attempt);
                    eprintln!(
                        "GraphQL transient error -- retrying in {:.1}s (attempt {}/{})",
                        delay.as_secs_f64(),
                        attempt + 1,
                        MAX_RETRIES
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Send a POST request to the Voyager GraphQL endpoint (mutation).
    pub async fn graphql_post(
        &self,
        variables: &Value,
        query_id: &str,
        query_name: &str,
    ) -> Result<Value, Error> {
        let url = format!(
            "{}{}graphql?action=execute&queryId={}&queryName={}",
            BASE_URL, API_PREFIX, query_id, query_name
        );
        let body = build_graphql_post_body(variables, query_id, query_name);

        let mut attempt = 0u32;
        loop {
            self.throttle().await;
            let resp = self
                .http
                .post(&url)
                .header("Csrf-Token", &self.jsessionid)
                .header("x-li-graphql-pegasus-client", "true")
                .json(&body)
                .send()
                .await?;
            match handle_graphql_post_response(resp).await {
                GraphqlPostResult::Ok(val) => return Ok(val),
                GraphqlPostResult::Retry(e) if attempt < MAX_RETRIES => {
                    let delay = graphql_retry_delay(attempt);
                    eprintln!(
                        "GraphQL transient error -- retrying in {:.1}s (attempt {}/{}): {:?}",
                        delay.as_secs_f64(),
                        attempt + 1,
                        MAX_RETRIES,
                        e
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                GraphqlPostResult::Retry(e) | GraphqlPostResult::Err(e) => return Err(e),
            }
        }
    }

    /// Send a GET request to an arbitrary path on the LinkedIn host (NOT
    /// prefixed with `/voyager/api/`). No retry.
    pub async fn api_get(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}", BASE_URL, path);
        let resp = self.csrf_get(&url).send().await?;
        check_response(resp).await
    }

    /// Send a POST request to an arbitrary path on `www.linkedin.com`. No retry.
    pub async fn api_post(&self, path: &str, body: &Value) -> Result<Value, Error> {
        let url = format!("{}{}", BASE_URL, path);
        let resp = self.csrf_post(&url).json(body).send().await?;
        check_response(resp).await
    }

    /// Returns the raw reqwest client for advanced use (e.g., auth endpoints
    /// outside the `/voyager/api/` prefix).
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub fn jsessionid(&self) -> &str {
        &self.jsessionid
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn x_li_track(&self) -> &str {
        &self.x_li_track
    }

    pub fn base_url(&self) -> &str {
        BASE_URL
    }
}

// ---------------------------------------------------------------------------
// Private helpers (used only inside this file)
// ---------------------------------------------------------------------------

fn build_default_headers(device_id: &str, x_li_track: &str) -> Result<HeaderMap, Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-RestLi-Protocol-Version",
        HeaderValue::from_static("2.0.0"),
    );
    headers.insert("X-LI-Lang", HeaderValue::from_static("en_US"));
    headers.insert("Accept-Language", HeaderValue::from_static("en-US"));
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        HeaderValue::from_static("ANDROID OS"),
    );
    headers.insert(
        "X-UDID",
        HeaderValue::from_str(device_id)
            .map_err(|e| Error::Auth(format!("invalid device_id header value: {e}")))?,
    );
    headers.insert(
        "X-LI-Track",
        HeaderValue::from_str(x_li_track)
            .map_err(|e| Error::Auth(format!("invalid X-LI-Track header value: {e}")))?,
    );
    Ok(headers)
}

fn build_graphql_post_body(variables: &Value, query_id: &str, query_name: &str) -> Value {
    let mut body = serde_json::json!({
        "queryId": query_id,
        "queryName": query_name,
    });
    if !variables.is_null() {
        body["variables"] = variables.clone();
    }
    body
}

/// Outcome of a `graphql_post` HTTP exchange.
enum GraphqlPostResult {
    Ok(Value),
    Retry(Error),
    Err(Error),
}

/// Decode the response from a `graphql_post` call. State-returning so the
/// loop in `graphql_post` stays flat.
async fn handle_graphql_post_response(resp: reqwest::Response) -> GraphqlPostResult {
    if resp.status().is_success() {
        decode_graphql_post_success(resp).await
    } else {
        decode_graphql_post_error(resp).await
    }
}

async fn decode_graphql_post_success(resp: reqwest::Response) -> GraphqlPostResult {
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => return GraphqlPostResult::Err(Error::Http(e)),
    };
    if text.is_empty() {
        return GraphqlPostResult::Ok(Value::Null);
    }
    let json: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return GraphqlPostResult::Err(Error::Json(e)),
    };
    match check_graphql_errors(&json) {
        Ok(()) => GraphqlPostResult::Ok(json),
        Err(e) if is_retriable_graphql_error(&e) => GraphqlPostResult::Retry(e),
        Err(e) => GraphqlPostResult::Err(e),
    }
}

async fn decode_graphql_post_error(resp: reqwest::Response) -> GraphqlPostResult {
    let status_code = resp.status().as_u16();
    let correlation_id = crate::error::extract_correlation_id(resp.headers());
    let body_text = resp.text().await.unwrap_or_default();
    if status_code == 401 {
        let suffix = correlation_id
            .as_deref()
            .map(|id| format!(" (request_id={id})"))
            .unwrap_or_default();
        return GraphqlPostResult::Err(Error::Auth(format!(
            "session expired or invalid (HTTP 401{suffix}): {body_text}"
        )));
    }
    GraphqlPostResult::Err(Error::Api {
        status: status_code,
        body: body_text,
        correlation_id,
    })
}

/// Decide whether a GraphQL-shaped error is worth retrying.
fn is_retriable_graphql_error(err: &Error) -> bool {
    match err {
        Error::Api {
            status: 200, body, ..
        } => {
            body.contains("Internal error fetching data from downstream")
                || body.contains("Failed to get response from server")
        }
        _ => false,
    }
}

fn graphql_retry_delay(attempt: u32) -> Duration {
    let ms = BASE_BACKOFF_MS * 2u64.pow(attempt);
    Duration::from_millis(ms.min(MAX_BACKOFF_MS))
}

/// Validate a cookie name against a conservative whitelist.
///
/// Accepts only `[A-Za-z0-9_-]+`. Real LinkedIn cookies use this character
/// set; the stricter rule than RFC 6265 keeps us from accepting names with
/// punctuation that could change meaning if the cookies file is ever sourced
/// from a less trusted location (semicolons split attributes, equals signs
/// split name/value, control characters could inject CRLF).
fn is_valid_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn generate_jsessionid() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let raw: i64 = rng.random();
    let value = if raw == i64::MIN {
        i64::MAX
    } else {
        raw.unsigned_abs() as i64
    };
    format!("ajax:{:019}", value)
}

fn build_x_li_track(device_id: &str) -> String {
    let tz_offset_hours: f64 = {
        let now = chrono::Local::now();
        let offset_secs = now.offset().local_minus_utc();
        offset_secs as f64 / 3600.0
    };
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

/// Result of checking an HTTP response with retry support.
enum RetryResult {
    Ok(Value),
    Err(Error),
    Retry(Duration),
}

/// Check an HTTP response for error status codes with retry awareness.
async fn check_response_retryable(resp: reqwest::Response, attempt: u32) -> RetryResult {
    let status = resp.status();
    if status.is_success() {
        return match resp.json::<Value>().await {
            Ok(json) => RetryResult::Ok(json),
            Err(e) => RetryResult::Err(Error::Http(e)),
        };
    }
    let status_code = status.as_u16();
    let retry_after = parse_retry_after(&resp);
    let correlation_id = crate::error::extract_correlation_id(resp.headers());
    let body = resp.text().await.unwrap_or_default();

    if status_code == 401 {
        let suffix = correlation_id
            .as_deref()
            .map(|id| format!(" (request_id={id})"))
            .unwrap_or_default();
        return RetryResult::Err(Error::Auth(format!(
            "session expired or invalid (HTTP 401{suffix}): {body}"
        )));
    }

    let is_retriable = status_code == 429 || (500..=599).contains(&status_code);
    if !is_retriable || attempt >= MAX_RETRIES {
        return RetryResult::Err(Error::Api {
            status: status_code,
            body,
            correlation_id,
        });
    }

    let delay = retry_after.unwrap_or_else(|| {
        let ms = BASE_BACKOFF_MS * 2u64.pow(attempt);
        Duration::from_millis(ms.min(MAX_BACKOFF_MS))
    });
    eprintln!(
        "HTTP {} -- retrying in {:.1}s (attempt {}/{})",
        status_code,
        delay.as_secs_f64(),
        attempt + 1,
        MAX_RETRIES
    );
    RetryResult::Retry(delay)
}

/// Parse the `Retry-After` header from an HTTP response.
fn parse_retry_after(resp: &reqwest::Response) -> Option<Duration> {
    let header = resp.headers().get("retry-after")?.to_str().ok()?;
    parse_retry_after_str(header)
}

fn parse_retry_after_str(header: &str) -> Option<Duration> {
    if let Some(secs) = parse_retry_after_seconds(header) {
        return Some(secs);
    }
    parse_retry_after_http_date(header)
}

fn parse_retry_after_seconds(header: &str) -> Option<Duration> {
    let secs = header.parse::<f64>().ok()?;
    if secs > 0.0 {
        Some(Duration::from_secs_f64(secs))
    } else {
        None
    }
}

fn parse_retry_after_http_date(header: &str) -> Option<Duration> {
    let dt = chrono::DateTime::parse_from_rfc2822(header).ok()?;
    let now = chrono::Utc::now();
    let delta = dt.signed_duration_since(now);
    if delta.num_seconds() > 0 {
        Some(Duration::from_secs(delta.num_seconds() as u64))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsessionid_format() {
        let id = generate_jsessionid();
        assert!(id.starts_with("ajax:"), "must start with 'ajax:': {id}");
        let numeric_part = &id["ajax:".len()..];
        assert_eq!(numeric_part.len(), 19);
        assert!(numeric_part.chars().all(|c| c.is_ascii_digit()));
        let parsed: i64 = numeric_part.parse().expect("must parse as i64");
        assert!(parsed >= 0);
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
        assert_required_x_li_track_fields(&track, device_id);
    }

    fn assert_required_x_li_track_fields(track: &Value, device_id: &str) {
        let exact_string_fields = [
            ("osName", "Android OS"),
            ("osVersion", "34"),
            ("model", "Google_Pixel 8"),
            ("deviceType", "android"),
            ("appId", "com.linkedin.android"),
            ("dpi", "xxhdpi"),
            ("storeId", "com.linkedin.android"),
            ("mpName", "voyager-android"),
        ];
        for (key, expected) in exact_string_fields {
            assert_eq!(track[key], expected, "{key}");
        }
        assert_eq!(track["clientVersion"], CLIENT_VERSION);
        assert_eq!(track["clientMinorVersion"], CLIENT_MINOR_VERSION);
        assert_eq!(track["mpVersion"], CLIENT_VERSION);
        assert_eq!(track["deviceId"], device_id);
        assert_eq!(track["displayWidth"], 1080);
        assert_eq!(track["displayHeight"], 2400);
        assert_eq!(track["isAdTrackingLimited"], true);
        assert!(track["timezoneOffset"].is_number());
        assert!(track["timezone"].is_string());
    }

    #[test]
    fn x_li_track_device_id_matches_udid() {
        let device_id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let track_json = build_x_li_track(device_id);
        let track: Value = serde_json::from_str(&track_json).unwrap();
        assert_eq!(track["deviceId"], device_id);
    }

    #[test]
    fn client_construction() {
        let client = LinkedInClient::new().expect("client creation must succeed");
        assert_constructed_client(&client);
    }

    fn assert_constructed_client(client: &LinkedInClient) {
        assert!(client.jsessionid().starts_with("ajax:"));
        assert!(!client.device_id().is_empty());
        assert!(!client.x_li_track().is_empty());
        assert_eq!(client.base_url(), "https://www.linkedin.com");
    }

    #[test]
    fn graphql_retry_classifies_internal_downstream_error() {
        let err = Error::Api {
            status: 200,
            body: "GraphQL errors: Internal error fetching data from downstream.".to_string(),
            correlation_id: None,
        };
        assert!(is_retriable_graphql_error(&err));
    }

    #[test]
    fn graphql_retry_classifies_failed_to_get_response() {
        let err = Error::Api {
            status: 200,
            body: "GraphQL errors: Failed to get response from server for URI https://[2a04:f547:93:21b::86e1]:5485/voyager/api/voyagerStoriesDashProfileVideoPreviews".to_string(),
        correlation_id: None,
        };
        assert!(is_retriable_graphql_error(&err));
    }

    #[test]
    fn graphql_retry_does_not_classify_field_error() {
        let err = Error::Api {
            status: 200,
            body: "GraphQL errors: Cannot query field 'foo' on type 'Bar'".to_string(),
            correlation_id: None,
        };
        assert!(!is_retriable_graphql_error(&err));
    }

    #[test]
    fn graphql_retry_does_not_classify_http_500() {
        let err = Error::Api {
            status: 500,
            body: "Internal Server Error".to_string(),
            correlation_id: None,
        };
        assert!(!is_retriable_graphql_error(&err));
    }

    #[test]
    fn graphql_retry_does_not_classify_auth_error() {
        let err = Error::Auth("session expired".to_string());
        assert!(!is_retriable_graphql_error(&err));
    }

    #[test]
    fn graphql_retry_delay_is_bounded() {
        let cases = [
            (0, Duration::from_millis(1000)),
            (1, Duration::from_millis(2000)),
            (2, Duration::from_millis(4000)),
        ];
        for (attempt, expected) in cases {
            assert_eq!(graphql_retry_delay(attempt), expected);
        }
        assert!(graphql_retry_delay(20) <= Duration::from_millis(MAX_BACKOFF_MS));
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

    #[test]
    fn cookie_name_accepts_real_linkedin_names() {
        for name in [
            "bcookie",
            "bscookie",
            "lidc",
            "li_gc",
            "li_mc",
            "li_sugr",
            "JSESSIONID",
            "UserMatchHistory",
            "li-x-li-token-12",
        ] {
            assert!(is_valid_cookie_name(name), "must accept: {name}");
        }
    }

    #[test]
    fn cookie_name_rejects_injection_attempts() {
        let rejected = [
            "",
            "bad name",
            "bad;Path=/",
            "bad=value",
            "bad,more",
            "bad\nname",
            "bad\rname",
            "bad\tname",
            "li_at\0",
            "name.with.dots",
            "name/slash",
            "name\"quote",
        ];
        for name in rejected {
            assert!(
                !is_valid_cookie_name(name),
                "must reject: {:?}",
                name.escape_debug().to_string()
            );
        }
    }
}
