//! Encoding + GraphQL envelope helpers shared across client submodules.
//!
//! These functions are not tied to `LinkedInClient`'s state, so keeping
//! them next to the resource modules that use them (rather than in
//! `client.rs`) keeps the parent module focused on the type itself and
//! keeps each per-resource submodule's import list short.

use serde_json::Value;

use crate::error::Error;

/// Build a GraphQL query parameter string for the Voyager GraphQL endpoint.
pub(super) fn graphql_params(variables: &str, query_id: &str, query_name: &str) -> String {
    format!(
        "variables={}&queryId={}&queryName={}",
        variables, query_id, query_name
    )
}

/// Encode a string using Rest.li's AsciiHex encoding for use in Rest.li
/// parenthesized record variables within a URL query parameter.
///
/// Source: `DataEncoder.processString()` and `AsciiHexEncoding` in the
/// decompiled LinkedIn Android app.
pub(super) fn restli_encode_string(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut buf = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'(' | b')' | b',' | b'\'' | b':' | b'%' => {
                buf.push('%');
                buf.push_str(&format!("{:02X}", byte));
            }
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
            _ => {
                buf.push('%');
                buf.push_str(&format!("{:02X}", byte));
            }
        }
    }
    buf
}

/// Unwrap the standard GraphQL response envelope: `data.{data_key}`.
pub(super) fn unwrap_graphql(raw: &Value, data_key: &str) -> Result<Value, Error> {
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
            correlation_id: None,
        })
}

/// Check a GraphQL JSON response for an `errors` array.
///
/// GraphQL can return HTTP 200 with logical errors in the response body.
/// Mutations may wrap errors under `value.errors` instead of the top-level
/// `errors` key, so both paths are checked.
pub(super) fn check_graphql_errors(json: &Value) -> Result<(), Error> {
    let errors = json.get("errors").and_then(|e| e.as_array()).or_else(|| {
        json.get("value")
            .and_then(|v| v.get("errors"))
            .and_then(|e| e.as_array())
    });

    let Some(errors) = errors else {
        return Ok(());
    };
    if errors.is_empty() {
        return Ok(());
    }
    let messages: Vec<&str> = errors
        .iter()
        .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
        .collect();
    Err(Error::Api {
        status: 200,
        body: format!("GraphQL errors: {}", messages.join("; ")),
        correlation_id: None,
    })
}

/// Check an HTTP response for error status codes and parse the body as JSON.
/// On 401, returns [`Error::Auth`]. On other non-success status codes,
/// returns [`Error::Api`] with the LinkedIn correlation ID attached when
/// the response carried one.
pub(super) async fn check_response(resp: reqwest::Response) -> Result<Value, Error> {
    let status = resp.status();
    if status.is_success() {
        let json = resp.json::<Value>().await?;
        return Ok(json);
    }
    let status_code = status.as_u16();
    let correlation_id = crate::error::extract_correlation_id(resp.headers());
    let body = resp.text().await.unwrap_or_default();
    if status_code == 401 {
        let suffix = correlation_id
            .as_deref()
            .map(|id| format!(" (request_id={id})"))
            .unwrap_or_default();
        return Err(Error::Auth(format!(
            "session expired or invalid (HTTP 401{suffix}): {body}"
        )));
    }
    Err(Error::Api {
        status: status_code,
        body,
        correlation_id,
    })
}

/// Normalize a LinkedIn social thread URN to a canonical format.
///
/// LinkedIn uses two different URN formats for comment threads:
///
/// - `urn:li:fsd_comment:(<comment_id>,urn:li:activity:<activity_id>)` —
///   the "social detail" format returned by some endpoints.
/// - `urn:li:comment:(activity:<activity_id>,<comment_id>)` — the canonical
///   comment URN format used by most API endpoints.
///
/// This function converts the former to the latter. If the input doesn't
/// match the `fsd_comment` pattern, it passes through unchanged (or wraps
/// bare IDs as `urn:li:activity:<id>`).
pub(super) fn normalize_social_thread_urn(thread_urn: &str) -> String {
    if let Some(rest) = thread_urn.strip_prefix("urn:li:fsd_comment:(") {
        if let Some(inner) = rest.strip_suffix(')') {
            if let Some((comment_id, activity_urn)) = inner.split_once(',') {
                if let Some(activity_id) = activity_urn.strip_prefix("urn:li:activity:") {
                    return format!("urn:li:comment:(activity:{activity_id},{comment_id})");
                }
            }
        }
    }

    if thread_urn.starts_with("urn:li:") {
        thread_urn.to_string()
    } else {
        format!("urn:li:activity:{thread_urn}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- normalize_social_thread_urn ----

    #[test]
    fn normalize_fsd_comment_urn_to_comment_urn() {
        let input = "urn:li:fsd_comment:(abc123,urn:li:activity:789012)";
        let out = normalize_social_thread_urn(input);
        assert_eq!(out, "urn:li:comment:(activity:789012,abc123)");
    }

    #[test]
    fn normalize_urn_passthrough() {
        let urn = "urn:li:activity:12345";
        assert_eq!(normalize_social_thread_urn(urn), urn);
    }

    #[test]
    fn normalize_bare_id_wraps_as_activity() {
        assert_eq!(
            normalize_social_thread_urn("12345"),
            "urn:li:activity:12345"
        );
    }

    // ---- restli encoding ----

    #[test]
    fn restli_encode_empty_string() {
        assert_eq!(restli_encode_string(""), "''");
    }

    #[test]
    fn restli_encode_plain_string() {
        assert_eq!(restli_encode_string("john-doe-123"), "john-doe-123");
    }

    #[test]
    fn restli_encode_reserved_chars() {
        let cases = [
            ("(", "%28"),
            (")", "%29"),
            (",", "%2C"),
            ("'", "%27"),
            (":", "%3A"),
            ("%", "%25"),
        ];
        for (input, expected) in cases {
            assert_eq!(restli_encode_string(input), expected, "input={input}");
        }
    }

    #[test]
    fn restli_encode_urn() {
        let encoded = restli_encode_string("urn:li:fsd_profile:abc123");
        assert_eq!(encoded, "urn%3Ali%3Afsd_profile%3Aabc123");
        assert!(!encoded.contains("%253A"), "must not double-encode");
    }

    #[test]
    fn restli_encode_spaces() {
        assert_eq!(restli_encode_string("hello world"), "hello%20world");
    }

    #[test]
    fn restli_encode_mixed() {
        assert_eq!(restli_encode_string("O'Brien"), "O%27Brien");
    }
}
