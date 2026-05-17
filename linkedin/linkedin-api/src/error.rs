//! Error types for the LinkedIn API client.

use std::fmt;

/// Maximum number of bytes of the raw API response body shown in [`Error::Api`]
/// Display output. The full body is still available on the variant for
/// programmatic inspection (debug logs, internal retry classifiers); the
/// truncation only applies to the human-readable rendering that ends up in CLI
/// error messages.
const MAX_DISPLAY_BODY_LEN: usize = 200;

/// Top-level error type for the linkedin-api crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Authentication failed or session expired.
    ///
    /// The Display impl sanitizes URN/email shapes in the message body — some
    /// callers construct this variant from raw HTTP 401 response bodies which
    /// can carry connection names, conversation snippets, and member URNs.
    #[error("{}", AuthDisplay(.0))]
    Auth(String),

    /// LinkedIn API returned an error status code.
    ///
    /// The Display impl truncates the body to at most [`MAX_DISPLAY_BODY_LEN`]
    /// bytes and masks LinkedIn URNs and email-shaped substrings. The raw
    /// `body` field remains accessible for programmatic use (e.g. the GraphQL
    /// retry classifier substring-matches on it). When a correlation ID is
    /// known, it's surfaced as `request_id=<id>` in the rendered message —
    /// useful when reporting an issue against LinkedIn so they can trace the
    /// specific server-side request.
    #[error("API error (HTTP {status}{}): {}", ApiCorrelationDisplay(.correlation_id), ApiBodyDisplay(.body))]
    Api {
        /// HTTP status code.
        status: u16,
        /// Response body (may be JSON error or empty). Raw — not sanitized.
        body: String,
        /// Correlation ID from the LinkedIn response headers, when known.
        /// `None` for synthesised errors (validation, unexpected shape, etc.)
        /// or when LinkedIn didn't send a recognisable correlation header.
        correlation_id: Option<String>,
    },

    /// Invalid input provided by the caller.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

/// Display wrapper that sanitizes the body when an `Error::Api` is rendered.
struct ApiBodyDisplay<'a>(&'a str);

impl fmt::Display for ApiBodyDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", sanitize_body(self.0))
    }
}

/// Display wrapper that renders `, request_id=<id>` when the correlation ID
/// is present, and nothing at all when it isn't.
struct ApiCorrelationDisplay<'a>(&'a Option<String>);

impl fmt::Display for ApiCorrelationDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(id) = self.0 {
            write!(f, ", request_id={}", id)?;
        }
        Ok(())
    }
}

/// Candidate response-header names that LinkedIn uses to carry a
/// per-request correlation ID. The order matters — we return the first
/// match. The list isn't documented; it's based on observed behavior, so
/// adding more candidates is fine.
pub(crate) const CORRELATION_HEADER_CANDIDATES: &[&str] = &[
    "x-li-uuid",
    "x-li-tracker-uuid",
    "x-li-tracking-id",
    "x-li-fabric",
];

/// Pluck the first available correlation-ID-shaped header from a reqwest
/// response header map. Returns `None` when none of the known headers are
/// present.
pub(crate) fn extract_correlation_id(headers: &reqwest::header::HeaderMap) -> Option<String> {
    for name in CORRELATION_HEADER_CANDIDATES {
        if let Some(value) = headers.get(*name) {
            if let Ok(s) = value.to_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// Display wrapper that sanitizes the message when an `Error::Auth` is rendered.
struct AuthDisplay<'a>(&'a str);

impl fmt::Display for AuthDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Auth error: {}", sanitize_body(self.0))
    }
}

/// Sanitize a response-body string for human display.
///
/// 1. Mask LinkedIn URNs: `urn:li:<type>:<opaque-id>` → `urn:li:<type>:[…]`.
/// 2. Mask email-shaped substrings: `<local>@<domain>` → `[email]`.
/// 3. Truncate to [`MAX_DISPLAY_BODY_LEN`] bytes at a char boundary, append
///    ` …[<n> more bytes]` when truncated.
///
/// The function is intentionally simple (no regex, no full parser). Heuristics
/// are tuned for what LinkedIn 4xx/5xx bodies typically embed: connection
/// names, conversation snippets, member URNs. False negatives are acceptable;
/// false positives (over-masking) are preferable to PII leakage.
fn sanitize_body(body: &str) -> String {
    let masked = mask_emails(&mask_urns(body));
    truncate_display(&masked, MAX_DISPLAY_BODY_LEN)
}

/// Replace LinkedIn URN opaque IDs with `[…]`.
fn mask_urns(body: &str) -> String {
    const PREFIX: &str = "urn:li:";
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(idx) = rest.find(PREFIX) {
        out.push_str(&rest[..idx]);
        let after_prefix = &rest[idx + PREFIX.len()..];
        // Read the entity type (alphanumeric + underscore + period) up to ':'.
        let type_end = after_prefix.find(':').unwrap_or(after_prefix.len());
        let entity_type = &after_prefix[..type_end];
        let is_valid_type = !entity_type.is_empty()
            && entity_type
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.');
        if !is_valid_type || type_end == after_prefix.len() {
            // Not a recognizable URN — emit the prefix literally and advance.
            out.push_str(PREFIX);
            rest = after_prefix;
            continue;
        }
        // Skip the opaque ID up to the next URN-terminator character.
        let id_start = type_end + 1;
        let id_part = &after_prefix[id_start..];
        let id_end = id_part
            .find(|c: char| {
                !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':')
            })
            .unwrap_or(id_part.len());
        out.push_str(PREFIX);
        out.push_str(entity_type);
        out.push_str(":[…]");
        rest = &id_part[id_end..];
    }
    out.push_str(rest);
    out
}

/// Replace email-shaped substrings with `[email]`.
///
/// Scans for `@` (an ASCII byte, so byte-indexing is safe), then expands
/// outward through the contiguous run of email-character bytes. Non-ASCII
/// characters anywhere in the input are preserved verbatim — we only ever
/// slice the input on byte indices that fall on email-character runs, which
/// by construction are ASCII.
fn mask_emails(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut out = String::with_capacity(body.len());
    let mut cursor = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        // Expand back through local-part bytes.
        let mut local_start = i;
        while local_start > 0 && is_email_local_byte(bytes[local_start - 1]) {
            local_start -= 1;
        }
        // Expand forward through domain bytes.
        let mut domain_end = i + 1;
        while domain_end < bytes.len() && is_email_domain_byte(bytes[domain_end]) {
            domain_end += 1;
        }
        let has_local = local_start < i;
        let domain = &body[i + 1..domain_end];
        if has_local && domain.contains('.') {
            // Emit everything from cursor to local_start verbatim, then [email].
            out.push_str(&body[cursor..local_start]);
            out.push_str("[email]");
            cursor = domain_end;
            i = domain_end;
        } else {
            i += 1;
        }
    }
    out.push_str(&body[cursor..]);
    out
}

fn is_email_local_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-' | b'%')
}

fn is_email_domain_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-')
}

/// Truncate `s` to at most `max_bytes` bytes at a UTF-8 char boundary.
/// When truncated, append ` …[<n> more bytes]` to make the loss visible.
fn truncate_display(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut cut = max_bytes;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let remaining = s.len() - cut;
    format!("{} …[{} more bytes]", &s[..cut], remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- sanitize_body helpers below -----

    #[test]
    fn sanitize_strips_member_urn() {
        let raw = r#"{"message":"viewer urn:li:fsd_profile:ACoAAAJqB-cBb1234 not allowed"}"#;
        let out = sanitize_body(raw);
        assert!(!out.contains("ACoAAAJqB-cBb1234"), "got: {out}");
        assert!(out.contains("urn:li:fsd_profile:[…]"), "got: {out}");
    }

    #[test]
    fn sanitize_strips_multiple_urns() {
        let raw = "urn:li:fsd_profile:abc123 sent message to urn:li:fsd_profile:def456";
        let out = sanitize_body(raw);
        assert!(!out.contains("abc123"));
        assert!(!out.contains("def456"));
        assert_eq!(out.matches("[…]").count(), 2);
    }

    #[test]
    fn sanitize_strips_email() {
        let raw = r#"contact alice.smith+work@example.com about this"#;
        let out = sanitize_body(raw);
        assert!(!out.contains("alice.smith"), "got: {out}");
        assert!(!out.contains("example.com"), "got: {out}");
        assert!(out.contains("[email]"), "got: {out}");
    }

    #[test]
    fn sanitize_truncates_long_body() {
        let body = "x".repeat(500);
        let out = sanitize_body(&body);
        assert!(out.len() < body.len());
        assert!(out.contains("more bytes]"));
        assert!(out.starts_with(&"x".repeat(200)));
    }

    #[test]
    fn sanitize_preserves_short_safe_body() {
        let raw = r#"{"status":"too many requests","retry":60}"#;
        let out = sanitize_body(raw);
        assert_eq!(out, raw);
    }

    #[test]
    fn sanitize_truncates_at_char_boundary() {
        // Pad to just past the limit with multibyte chars to verify
        // truncation lands on a UTF-8 boundary.
        let mut body = "x".repeat(MAX_DISPLAY_BODY_LEN - 1);
        body.push('é'); // 2-byte; pushes total length past the limit
        body.push_str("rest");
        let out = sanitize_body(&body);
        // Must be valid UTF-8 (would panic on construction otherwise; this
        // guards against future regressions if truncate logic changes).
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn sanitize_leaves_partial_urn_alone() {
        // The prefix appears but no entity type follows — don't mask.
        let raw = "see urn:li: at line 5";
        let out = sanitize_body(raw);
        assert_eq!(out, raw);
    }

    #[test]
    fn display_for_api_uses_sanitized_body() {
        let err = Error::Api {
            status: 403,
            body: "blocked viewer urn:li:fsd_profile:ACoAAAJqB-cBb1234".to_string(),
            correlation_id: None,
        };
        let rendered = format!("{err}");
        assert!(rendered.starts_with("API error (HTTP 403)"));
        assert!(!rendered.contains("ACoAAAJqB-cBb1234"), "got: {rendered}");
        assert!(rendered.contains("[…]"), "got: {rendered}");
        // No correlation ID → no `request_id=…` segment.
        assert!(!rendered.contains("request_id"), "got: {rendered}");
    }

    #[test]
    fn display_for_api_includes_request_id_when_present() {
        let err = Error::Api {
            status: 500,
            body: "server error".to_string(),
            correlation_id: Some("AbCdEf1234567890".to_string()),
        };
        let rendered = format!("{err}");
        assert!(
            rendered.contains("request_id=AbCdEf1234567890"),
            "got: {rendered}"
        );
    }

    #[test]
    fn extract_correlation_id_returns_first_match() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-li-uuid",
            reqwest::header::HeaderValue::from_static("first-id"),
        );
        headers.insert(
            "x-li-tracker-uuid",
            reqwest::header::HeaderValue::from_static("second-id"),
        );
        assert_eq!(extract_correlation_id(&headers), Some("first-id".into()));
    }

    #[test]
    fn extract_correlation_id_falls_back_through_candidates() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-li-tracking-id",
            reqwest::header::HeaderValue::from_static("third-id"),
        );
        assert_eq!(extract_correlation_id(&headers), Some("third-id".into()));
    }

    #[test]
    fn extract_correlation_id_returns_none_when_absent() {
        let headers = reqwest::header::HeaderMap::new();
        assert!(extract_correlation_id(&headers).is_none());
    }

    #[test]
    fn extract_correlation_id_skips_empty_values() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-li-uuid",
            reqwest::header::HeaderValue::from_static("   "),
        );
        assert!(extract_correlation_id(&headers).is_none());
    }

    #[test]
    fn display_for_auth_uses_sanitized_body() {
        let err = Error::Auth(
            "session expired (HTTP 401): owner alice@example.com please re-login".to_string(),
        );
        let rendered = format!("{err}");
        assert!(rendered.starts_with("Auth error:"));
        assert!(!rendered.contains("alice@example.com"), "got: {rendered}");
        assert!(rendered.contains("[email]"), "got: {rendered}");
    }

    #[test]
    fn api_body_field_remains_raw_for_match() {
        // Confirms the retry classifier and other internal logic still see
        // the raw body even though Display sanitizes it.
        let err = Error::Api {
            status: 200,
            body: "GraphQL errors: Internal error fetching data from downstream urn:li:fs_post:abc"
                .to_string(),
            correlation_id: None,
        };
        match &err {
            Error::Api { body, .. } => {
                assert!(body.contains("Internal error fetching data from downstream"));
                assert!(body.contains("urn:li:fs_post:abc"));
            }
            _ => panic!("expected Api"),
        }
    }
}
