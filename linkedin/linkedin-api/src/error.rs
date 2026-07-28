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

    /// Every slug → URN resolver endpoint failed.
    ///
    /// Carries a [`ProfileResolutionFailure`] classification because the
    /// failure modes have opposite recoveries (back off / fix the slug /
    /// stop using this resolver), plus the per-endpoint outcomes that
    /// produced the classification so a reader can check the reasoning.
    #[error("could not resolve profile '{public_id}': {reason} (attempts: {})", AttemptsDisplay(.attempts))]
    ProfileResolution {
        /// The vanity slug that failed to resolve.
        public_id: String,
        /// Which recovery applies.
        reason: ProfileResolutionFailure,
        /// Per-endpoint outcomes, in the order they were tried.
        attempts: Vec<ResolutionAttempt>,
    },
}

/// Which of the mutually-exclusive recoveries applies when
/// [`Error::ProfileResolution`] fires.
///
/// The three named failures need opposite responses: wait it out, fix the
/// slug, or stop asking these endpoints entirely. Collapsing them into one
/// opaque message costs the caller the ability to react at all.
/// `Inconclusive` is the honest fallback for when the evidence singles out
/// none of the three; it is not a synonym for "not found".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileResolutionFailure {
    /// At least one endpoint returned HTTP 429 after exhausting its retries.
    RateLimited,
    /// The requested slug is the authenticated user's own vanity slug.
    SelfSlugUnsupported,
    /// Every endpoint answered, and none of them knows this slug.
    NotFound,
    /// The outcomes point at none of the above.
    Inconclusive,
}

impl fmt::Display for ProfileResolutionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::RateLimited => {
                "rate limited: an endpoint returned HTTP 429 after exhausting its retries. \
                 Recovery: back off and retry later; the slug itself is fine."
            }
            Self::SelfSlugUnsupported => {
                "self-slug unsupported: this is the authenticated user's own vanity slug, and \
                 the resolver endpoints answer for other members only. \
                 Recovery: use the `/me` profile URN instead; retrying can never succeed."
            }
            Self::NotFound => {
                "profile not found: every endpoint answered and none of them knows this slug. \
                 Recovery: check the spelling, or the member renamed or removed the profile."
            }
            Self::Inconclusive => {
                "inconclusive: the endpoint outcomes match neither rate limiting, nor a missing \
                 profile, nor a self-slug. Recovery: read the per-endpoint outcomes."
            }
        };
        f.write_str(text)
    }
}

/// What one resolver endpoint reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointOutcome {
    /// The endpoint returned this HTTP status.
    Status(u16),
    /// The endpoint answered successfully but carried no URN for the slug.
    /// Also covers this crate's synthesised `Error::Api { status: 0, .. }`
    /// shape errors, which mean the same thing: a 2xx with nothing in it.
    Empty,
    /// The request never produced a status (connection reset, TLS, JSON).
    Transport(String),
}

impl EndpointOutcome {
    /// Classify one endpoint's [`Error`] into an outcome.
    ///
    /// `status: 0` is this crate's convention for a synthesised (non-HTTP)
    /// error, so it maps to [`EndpointOutcome::Empty`] rather than to a
    /// nonsense HTTP status.
    pub fn from_error(err: &Error) -> Self {
        match err {
            Error::Api { status: 0, .. } => Self::Empty,
            Error::Api { status, .. } => Self::Status(*status),
            Error::Http(e) => Self::Transport(e.to_string()),
            Error::Json(e) => Self::Transport(e.to_string()),
            Error::Auth(msg) | Error::InvalidInput(msg) => Self::Transport(msg.clone()),
            // A nested resolution failure can't happen (no resolver calls
            // another resolver), but the match has to be total.
            Error::ProfileResolution { reason, .. } => Self::Transport(reason.to_string()),
        }
    }
}

impl fmt::Display for EndpointOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(status) => write!(f, "HTTP {status}"),
            Self::Empty => f.write_str("empty"),
            Self::Transport(msg) => write!(f, "transport: {}", sanitize_body(msg)),
        }
    }
}

/// One endpoint's contribution to a failed slug → URN resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionAttempt {
    /// Short endpoint label, e.g. `identity/miniprofiles`.
    pub endpoint: &'static str,
    /// What that endpoint reported.
    pub outcome: EndpointOutcome,
}

impl ResolutionAttempt {
    /// Build an attempt record from an endpoint label and its error.
    pub fn from_error(endpoint: &'static str, err: &Error) -> Self {
        Self {
            endpoint,
            outcome: EndpointOutcome::from_error(err),
        }
    }
}

impl fmt::Display for ResolutionAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.endpoint, self.outcome)
    }
}

/// Display wrapper that renders the attempt list as a comma-separated run.
struct AttemptsDisplay<'a>(&'a [ResolutionAttempt]);

impl fmt::Display for AttemptsDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, attempt) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{attempt}")?;
        }
        Ok(())
    }
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
    CORRELATION_HEADER_CANDIDATES
        .iter()
        .find_map(|name| header_nonempty_value(headers, name))
}

/// Read a single header as a trimmed, non-empty string. Returns `None` when the
/// header is absent, not valid UTF-8, or blank after trimming.
fn header_nonempty_value(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?;
    let trimmed = value.to_str().ok()?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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

const URN_PREFIX: &str = "urn:li:";

/// Replace LinkedIn URN opaque IDs with `[…]`.
fn mask_urns(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(idx) = rest.find(URN_PREFIX) {
        out.push_str(&rest[..idx]);
        let after_prefix = &rest[idx + URN_PREFIX.len()..];
        rest = mask_one_urn(after_prefix, &mut out);
    }
    out.push_str(rest);
    out
}

/// Handle a single `urn:li:` occurrence. `after_prefix` is the text immediately
/// following the prefix. Pushes the rendered output onto `out` and returns the
/// remaining text to continue scanning from.
fn mask_one_urn<'a>(after_prefix: &'a str, out: &mut String) -> &'a str {
    // Read the entity type (alphanumeric + underscore + period) up to ':'.
    let type_end = after_prefix.find(':').unwrap_or(after_prefix.len());
    let entity_type = &after_prefix[..type_end];
    if !is_valid_urn_type(entity_type) || type_end == after_prefix.len() {
        // Not a recognizable URN — emit the prefix literally and advance.
        out.push_str(URN_PREFIX);
        return after_prefix;
    }
    // Skip the opaque ID up to the next URN-terminator character.
    let id_part = &after_prefix[type_end + 1..];
    let id_end = id_part
        .find(|c: char| {
            !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':')
        })
        .unwrap_or(id_part.len());
    out.push_str(URN_PREFIX);
    out.push_str(entity_type);
    out.push_str(":[…]");
    &id_part[id_end..]
}

fn is_valid_urn_type(entity_type: &str) -> bool {
    !entity_type.is_empty()
        && entity_type
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
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
        match email_span_at(bytes, body, i) {
            Some((local_start, domain_end)) => {
                // Emit everything from cursor to local_start verbatim, then [email].
                out.push_str(&body[cursor..local_start]);
                out.push_str("[email]");
                cursor = domain_end;
                i = domain_end;
            }
            None => i += 1,
        }
    }
    out.push_str(&body[cursor..]);
    out
}

/// Given an `@` at byte index `at`, return the `(local_start, domain_end)` byte
/// span of a valid email-shaped substring, or `None` when the surrounding text
/// doesn't look like an email (no local part, or domain without a dot).
fn email_span_at(bytes: &[u8], body: &str, at: usize) -> Option<(usize, usize)> {
    // Expand back through local-part bytes.
    let mut local_start = at;
    while local_start > 0 && is_email_local_byte(bytes[local_start - 1]) {
        local_start -= 1;
    }
    // Expand forward through domain bytes.
    let mut domain_end = at + 1;
    while domain_end < bytes.len() && is_email_domain_byte(bytes[domain_end]) {
        domain_end += 1;
    }
    let has_local = local_start < at;
    let domain = &body[at + 1..domain_end];
    if has_local && domain.contains('.') {
        Some((local_start, domain_end))
    } else {
        None
    }
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
    fn profile_resolution_display_names_reason_and_attempts() {
        let err = Error::ProfileResolution {
            public_id: "eric-vyacheslav".to_string(),
            reason: ProfileResolutionFailure::RateLimited,
            attempts: vec![
                ResolutionAttempt {
                    endpoint: "identity/miniprofiles",
                    outcome: EndpointOutcome::Status(429),
                },
                ResolutionAttempt {
                    endpoint: "graphql:ProfilesByMemberIdentity",
                    outcome: EndpointOutcome::Empty,
                },
            ],
        };
        let rendered = format!("{err}");
        assert!(rendered.contains("eric-vyacheslav"), "got: {rendered}");
        assert!(rendered.contains("rate limited"), "got: {rendered}");
        assert!(
            rendered.contains("identity/miniprofiles=HTTP 429"),
            "got: {rendered}"
        );
        assert!(
            rendered.contains("graphql:ProfilesByMemberIdentity=empty"),
            "got: {rendered}"
        );
    }

    #[test]
    fn profile_resolution_reasons_carry_distinct_recoveries() {
        // The whole point of the variant: three failures, three different
        // things the caller should do next.
        let rendered: Vec<String> = [
            ProfileResolutionFailure::RateLimited,
            ProfileResolutionFailure::SelfSlugUnsupported,
            ProfileResolutionFailure::NotFound,
            ProfileResolutionFailure::Inconclusive,
        ]
        .iter()
        .map(|r| r.to_string())
        .collect();
        for text in &rendered {
            assert!(text.contains("Recovery:"), "missing recovery: {text}");
        }
        assert!(rendered[0].contains("back off"));
        assert!(rendered[1].contains("never succeed"));
        assert!(rendered[2].contains("spelling"));
        // All four messages must be distinguishable from each other.
        let unique: std::collections::HashSet<&String> = rendered.iter().collect();
        assert_eq!(unique.len(), rendered.len());
    }

    #[test]
    fn transport_outcome_display_is_sanitized() {
        let outcome = EndpointOutcome::Transport(
            "error sending request for urn:li:fsd_profile:ACoAAAJqB-cBb1234".to_string(),
        );
        let rendered = outcome.to_string();
        assert!(!rendered.contains("ACoAAAJqB-cBb1234"), "got: {rendered}");
        assert!(rendered.contains("[…]"), "got: {rendered}");
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
