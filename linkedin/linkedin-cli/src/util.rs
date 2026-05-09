use std::process;

use linkedin_api::models::Paging;

/// Exit codes for structured error reporting.
///
/// Scripts can check `$?` to distinguish error types:
/// - 0: success
/// - 1: general/unknown error
/// - 2: authentication error (expired session, invalid cookie)
/// - 3: API error (rate limited, server error, bad response)
/// - 4: input error (invalid arguments, missing required flags)
pub mod exit_code {
    pub const AUTH: i32 = 2;
    pub const API: i32 = 3;
    pub const INPUT: i32 = 4;
}

/// Substring patterns that map an error message to an exit code.
///
/// Order matters: the first matching category wins. Tightly-scoped
/// auth markers come first so a "session expired … HTTP 401" message
/// classifies as auth, not API.
const CLASSIFY_PATTERNS: &[(i32, &[&str])] = &[
    (
        exit_code::AUTH,
        &["session", "auth", "li_at", "401", "not logged in"],
    ),
    (
        exit_code::API,
        &["API", "HTTP", "429", "500", "failed to send"],
    ),
    (
        exit_code::INPUT,
        &[
            "index must be",
            "invalid",
            "must not be empty",
            "out of range",
            "not confirmed",
        ],
    ),
];

/// Classify an error string into an exit code.
///
/// Inspects the error message for known patterns to determine the
/// appropriate exit code.
pub fn classify_error(msg: &str) -> i32 {
    for (code, keywords) in CLASSIFY_PATTERNS {
        if keywords.iter().any(|k| msg.contains(k)) {
            return *code;
        }
    }
    1
}

/// Hint substring patterns mapped to actionable suggestions.
///
/// Order matters: the first matching group wins. Higher-signal markers
/// (e.g. `HTTP 401`) come before broader ones so a stale-session message
/// gets the auth hint rather than a generic API one.
const HINT_PATTERNS: &[(&[&str], &str)] = &[
    (
        &["HTTP 401", "session expired", "li_at"],
        "Hint: session looks stale. Refresh with: li auth login <li_at_cookie>",
    ),
    (
        &["HTTP 301"],
        "Hint: HTTP 301 from a Voyager endpoint usually means LinkedIn retired it. Check re/ docs for the modern path; if the path is still listed there, file an issue.",
    ),
    (
        &["HTTP 429"],
        "Hint: rate limited. Wait a few minutes; consider lowering --pacing-ms below 2000 only if you've checked your quota.",
    ),
    (
        &[
            "Internal error fetching data from downstream",
            "Failed to get response from server",
        ],
        "Hint: transient GraphQL error. The client already retried up to MAX_RETRIES times — if you see this often, LinkedIn's mesh is degraded. Try again in a minute.",
    ),
    (
        &["failed to resolve profile"],
        "Hint: the slug→URN resolver flaked. Pass the fsd_profile URN directly if you have it, or use 'li search invite N' once the person is in your last search results.",
    ),
    (
        &["HTTP 403"],
        "Hint: LinkedIn blocked the request. This often means a captcha challenge — open the site in a browser and complete any pending verification.",
    ),
    (
        &["out of range"],
        "Hint: rerun the parent command (e.g. 'li search people \"…\"' or 'li feed list') first to populate the cache.",
    ),
];

/// Suggest a one-line next step the user can take, based on the shape of
/// an error message. Returns `None` if no specific hint applies.
///
/// Pattern matching is intentionally string-based so it works on errors
/// surfaced through `format!("…: {e}")` chains where the original
/// `Error` variant is already lost.
pub fn error_hint(msg: &str) -> Option<&'static str> {
    HINT_PATTERNS
        .iter()
        .find(|(keywords, _)| keywords.iter().any(|k| msg.contains(k)))
        .map(|(_, hint)| *hint)
}

/// Run a command and exit with a classified error code on failure.
pub fn exit_on_err(result: Result<(), String>) {
    if let Err(e) = result {
        let code = classify_error(&e);
        eprintln!("error: {e}");
        if let Some(hint) = error_hint(&e) {
            eprintln!("{hint}");
        }
        process::exit(code);
    }
}

/// Truncate a string to at most `max_chars` characters, safely handling
/// multi-byte UTF-8. Returns the original string if it is shorter than
/// `max_chars`.
pub fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Truncate a string and append `...` if it was truncated.
/// Returns the original string unchanged if it fits within `max_chars`.
pub fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let truncated = truncate(s, max_chars);
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

/// Print a paging header line in the format:
/// `{label} (offset {start}, showing {count}, total {total})`
pub fn print_paging_header(label: &str, paging: &Paging) {
    let total_str = paging
        .total
        .map(|t| t.to_string())
        .unwrap_or_else(|| "?".to_string());
    println!(
        "{} (offset {}, showing {}, total {})",
        label, paging.start, paging.count, total_str
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_for_401_suggests_relogin() {
        let hint = error_hint("API call failed: API error (HTTP 401): session expired");
        assert!(hint.unwrap().contains("li auth login"));
    }

    #[test]
    fn hint_for_301_suggests_re_doc_check() {
        let hint = error_hint("API call failed: API error (HTTP 301): {\"status\":301}");
        assert!(hint.unwrap().contains("retired"));
    }

    #[test]
    fn hint_for_429_suggests_backoff() {
        let hint = error_hint("API call failed: API error (HTTP 429): too many requests");
        assert!(hint.unwrap().contains("rate limited"));
    }

    #[test]
    fn hint_for_resolver_failure_suggests_alternatives() {
        let hint =
            error_hint("failed to resolve profile: API error (HTTP 200): GraphQL errors: ...");
        assert!(hint.unwrap().contains("URN") || hint.unwrap().contains("search invite"));
    }

    #[test]
    fn hint_for_index_out_of_range_suggests_repopulating_cache() {
        let hint = error_hint("index 5 out of range (search has 3 results)");
        assert!(hint.unwrap().contains("cache"));
    }

    #[test]
    fn hint_for_unknown_error_returns_none() {
        assert!(error_hint("some random unrecognised error").is_none());
    }
}
