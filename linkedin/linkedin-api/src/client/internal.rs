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
        })
}

/// Check a GraphQL JSON response for a top-level `errors` array.
pub(super) fn check_graphql_errors(json: &Value) -> Result<(), Error> {
    let Some(errors) = json.get("errors").and_then(|e| e.as_array()) else {
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
    })
}

/// Check an HTTP response for error status codes and parse the body as JSON.
/// On 401, returns [`Error::Auth`]. On other non-success status codes,
/// returns [`Error::Api`].
pub(super) async fn check_response(resp: reqwest::Response) -> Result<Value, Error> {
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
