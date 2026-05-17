//! CLI-side error type.
//!
//! Replaces the historical `Result<_, String>` returns. Callers either get a
//! `CliError` from the library (`linkedin_api::Error` → `CliError::Api` via
//! `From`) or build one of the ad-hoc CLI variants (`Session`, `Cache`,
//! `Input`, `Other`). The variants drive both the exit code and the
//! actionable hint without substring-matching the error message.

use linkedin_api::error::Error as ApiError;

/// All errors a `cmd_*` function can produce.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// An error from the linkedin-api crate (HTTP, JSON, Auth, Api status).
    #[error("API call failed: {0}")]
    Api(#[from] ApiError),

    /// Session loading, expiry, or cookie-file I/O.
    #[error("{0}")]
    Session(String),

    /// Local cache read/parse failure.
    #[error("{0}")]
    Cache(String),

    /// Invalid input from the user (bad flag combination, out-of-range index,
    /// not-confirmed prompt, etc.).
    #[error("{0}")]
    Input(String),

    /// Anything that doesn't fit the other variants.
    #[error("{0}")]
    Other(String),
}

/// Shorthand for fallible CLI command return types.
pub type CliResult<T> = Result<T, CliError>;

/// Exit-code categories used by `exit_on_err`.
pub mod exit_code {
    pub const AUTH: i32 = 2;
    pub const API: i32 = 3;
    pub const INPUT: i32 = 4;
}

impl CliError {
    /// Classify the error into a script-friendly exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Session(_) => exit_code::AUTH,
            CliError::Api(ApiError::Auth(_)) => exit_code::AUTH,
            CliError::Api(ApiError::Api { status: 401, .. }) => exit_code::AUTH,
            CliError::Api(_) => exit_code::API,
            CliError::Input(_) => exit_code::INPUT,
            CliError::Cache(_) | CliError::Other(_) => 1,
        }
    }

    /// Optional actionable next step. Matched on the typed variant rather
    /// than substring-matched against the formatted message.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            CliError::Api(ApiError::Auth(_))
            | CliError::Api(ApiError::Api { status: 401, .. })
            | CliError::Session(_) => Some(
                "Hint: session looks stale. Refresh with: li auth login <li_at_cookie>",
            ),
            CliError::Api(ApiError::Api { status: 301, .. }) => Some(
                "Hint: HTTP 301 from a Voyager endpoint usually means LinkedIn retired it. \
                 Check re/ docs for the modern path; if the path is still listed there, file an issue.",
            ),
            CliError::Api(ApiError::Api { status: 403, .. }) => Some(
                "Hint: LinkedIn blocked the request. This often means a captcha challenge — \
                 open the site in a browser and complete any pending verification.",
            ),
            CliError::Api(ApiError::Api { status: 429, .. }) => Some(
                "Hint: rate limited. Wait a few minutes; consider lowering --pacing-ms below 2000 \
                 only if you've checked your quota.",
            ),
            CliError::Api(ApiError::Api { status: 200, body }) => {
                if body.contains("Internal error fetching data from downstream")
                    || body.contains("Failed to get response from server")
                {
                    Some(
                        "Hint: transient GraphQL error. The client already retried up to MAX_RETRIES \
                         times — if you see this often, LinkedIn's mesh is degraded. \
                         Try again in a minute.",
                    )
                } else {
                    None
                }
            }
            CliError::Other(msg) if msg.contains("failed to resolve profile") => Some(
                "Hint: the slug→URN resolver flaked. Pass the fsd_profile URN directly if you have it, \
                 or use 'li search invite N' once the person is in your last search results.",
            ),
            CliError::Input(msg) if msg.contains("out of range") => Some(
                "Hint: rerun the parent command (e.g. 'li search people \"…\"' or 'li feed list') first \
                 to populate the cache.",
            ),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError::Other(format!("JSON error: {e}"))
    }
}

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        CliError::Other(s.to_string())
    }
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError::Other(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_err(status: u16, body: &str) -> CliError {
        CliError::Api(ApiError::Api {
            status,
            body: body.to_string(),
        })
    }

    #[test]
    fn exit_code_routes_auth_session() {
        assert_eq!(
            CliError::Session("expired".into()).exit_code(),
            exit_code::AUTH
        );
        assert_eq!(
            CliError::Api(ApiError::Auth("…".into())).exit_code(),
            exit_code::AUTH
        );
        assert_eq!(api_err(401, "").exit_code(), exit_code::AUTH);
    }

    #[test]
    fn exit_code_routes_api_input() {
        assert_eq!(api_err(429, "").exit_code(), exit_code::API);
        assert_eq!(api_err(500, "").exit_code(), exit_code::API);
        assert_eq!(CliError::Input("…".into()).exit_code(), exit_code::INPUT);
    }

    #[test]
    fn exit_code_fallback_is_1() {
        assert_eq!(CliError::Cache("…".into()).exit_code(), 1);
        assert_eq!(CliError::Other("…".into()).exit_code(), 1);
    }

    #[test]
    fn hint_401_suggests_relogin() {
        let hint = api_err(401, "session expired").hint().unwrap();
        assert!(hint.contains("li auth login"));
    }

    #[test]
    fn hint_301_suggests_re_doc_check() {
        let hint = api_err(301, "{\"status\":301}").hint().unwrap();
        assert!(hint.contains("retired"));
    }

    #[test]
    fn hint_429_suggests_backoff() {
        let hint = api_err(429, "").hint().unwrap();
        assert!(hint.contains("rate limited"));
    }

    #[test]
    fn hint_403_suggests_captcha_check() {
        let hint = api_err(403, "blocked").hint().unwrap();
        assert!(hint.contains("captcha"));
    }

    #[test]
    fn hint_transient_graphql_error_suggests_retry() {
        let hint = api_err(200, "Internal error fetching data from downstream")
            .hint()
            .unwrap();
        assert!(hint.contains("transient"));
    }

    #[test]
    fn hint_index_out_of_range_points_to_cache_repopulation() {
        let hint = CliError::Input("index 5 out of range".into())
            .hint()
            .unwrap();
        assert!(hint.contains("cache"));
    }

    #[test]
    fn hint_unknown_returns_none() {
        assert!(CliError::Other("no idea".into()).hint().is_none());
    }

    #[test]
    fn api_error_from_lib_routes_through_from_impl() {
        let lib_err = ApiError::Api {
            status: 429,
            body: "rate limited".to_string(),
        };
        let cli_err: CliError = lib_err.into();
        assert_eq!(cli_err.exit_code(), exit_code::API);
    }
}
