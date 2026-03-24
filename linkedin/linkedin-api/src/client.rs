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
        let x_li_track = build_x_li_track(&device_id);

        // Build default headers applied to every request.
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            "X-RestLi-Protocol-Version",
            HeaderValue::from_static("2.0.0"),
        );
        default_headers.insert("X-LI-Lang", HeaderValue::from_static("en_US"));
        default_headers.insert("Accept-Language", HeaderValue::from_static("en-US"));
        // Accept JSON to avoid needing protobuf deserialization.
        // The real app uses protobuf, but the server supports JSON.
        default_headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        // User-Agent: the main app relies on Cronet's UA (Chrome-like).
        // The auth library uses "ANDROID OS" for its direct HTTP calls.
        // We use "ANDROID OS" as a safe baseline; Cronet-level UA spoofing
        // would require a different TLS backend anyway.
        default_headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static("ANDROID OS"),
        );

        // X-UDID and X-LI-Track are set per-request via the header injection
        // below, but since they're constant for the client lifetime we could
        // also set them as defaults. We set them here for simplicity.
        default_headers.insert(
            "X-UDID",
            HeaderValue::from_str(&device_id)
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

        // Seed the JSESSIONID cookie. The real app generates this client-side
        // via CsrfCookieHelper.generateJsessionId() before the first request.
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

        let http = reqwest::Client::builder()
            .cookie_provider(jar)
            .default_headers(default_headers)
            .build()?;

        Ok(Self {
            http,
            jsessionid,
            device_id,
            x_li_track,
        })
    }

    /// Create a client with a specific device ID and JSESSIONID (for testing or persistence).
    ///
    /// Use this when you want to restore a previous session's device identity
    /// rather than generating a new one.
    pub fn with_identity(device_id: String, jsessionid: String) -> Result<Self, Error> {
        let x_li_track = build_x_li_track(&device_id);

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
            HeaderValue::from_str(&device_id)
                .map_err(|e| Error::Auth(format!("invalid device_id header value: {e}")))?,
        );
        default_headers.insert(
            "X-LI-Track",
            HeaderValue::from_str(&x_li_track)
                .map_err(|e| Error::Auth(format!("invalid X-LI-Track header value: {e}")))?,
        );

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

        let http = reqwest::Client::builder()
            .cookie_provider(jar)
            .default_headers(default_headers)
            .build()?;

        Ok(Self {
            http,
            jsessionid,
            device_id,
            x_li_track,
        })
    }

    /// Create a client from a persisted [`Session`].
    ///
    /// Uses the session's JSESSIONID and injects the `li_at` cookie into the
    /// cookie jar on the `.linkedin.com` domain. A new device ID is generated
    /// (the session doesn't persist device identity -- that's a separate concern).
    pub fn with_session(session: &Session) -> Result<Self, Error> {
        let device_id = uuid::Uuid::new_v4().to_string();
        let x_li_track = build_x_li_track(&device_id);

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
            HeaderValue::from_str(&device_id)
                .map_err(|e| Error::Auth(format!("invalid device_id header value: {e}")))?,
        );
        default_headers.insert(
            "X-LI-Track",
            HeaderValue::from_str(&x_li_track)
                .map_err(|e| Error::Auth(format!("invalid X-LI-Track header value: {e}")))?,
        );

        let jar = Arc::new(reqwest::cookie::Jar::default());
        let base_url: url::Url = BASE_URL
            .parse()
            .map_err(|e| Error::Auth(format!("invalid base URL: {e}")))?;

        // Set JSESSIONID cookie (CSRF token).
        jar.add_cookie_str(
            &format!(
                "JSESSIONID=\"{}\"; Domain=.linkedin.com; Path=/; Secure",
                session.jsessionid
            ),
            &base_url,
        );

        // Set li_at cookie (session authentication).
        jar.add_cookie_str(
            &format!(
                "li_at={}; Domain=.linkedin.com; Path=/; Secure",
                session.li_at
            ),
            &base_url,
        );

        let http = reqwest::Client::builder()
            .cookie_provider(jar)
            .default_headers(default_headers)
            .build()?;

        Ok(Self {
            http,
            jsessionid: session.jsessionid.clone(),
            device_id,
            x_li_track,
        })
    }

    /// Send a GET request to a Voyager API endpoint.
    ///
    /// The `path` is relative to `/voyager/api/` -- do not include the prefix.
    /// For example, `client.get("me")` requests `https://www.linkedin.com/voyager/api/me`.
    ///
    /// The CSRF token header is added automatically.
    pub async fn get(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}{}", BASE_URL, API_PREFIX, path);
        let resp = self
            .http
            .get(&url)
            .header("Csrf-Token", &self.jsessionid)
            .send()
            .await?;
        let json = resp.json::<Value>().await?;
        Ok(json)
    }

    /// Send a POST request to a Voyager API endpoint with a JSON body.
    ///
    /// The `path` is relative to `/voyager/api/` -- do not include the prefix.
    /// The `body` is serialized as JSON in the request body.
    ///
    /// The CSRF token header is added automatically.
    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, Error> {
        let url = format!("{}{}{}", BASE_URL, API_PREFIX, path);
        let resp = self
            .http
            .post(&url)
            .header("Csrf-Token", &self.jsessionid)
            .json(body)
            .send()
            .await?;
        let json = resp.json::<Value>().await?;
        Ok(json)
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
}

impl Default for LinkedInClient {
    fn default() -> Self {
        Self::new().expect("failed to create default LinkedInClient")
    }
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
