//! CLI-side error type.
//!
//! Replaces the historical `Result<_, String>` returns. Callers either get a
//! `CliError` from the library (`linkedin_api::Error` → `CliError::Api` via
//! `From`) or build one of the ad-hoc CLI variants (`Session`, `Cache`,
//! `Input`, `Other`). The variants drive both the exit code and the
//! actionable hint without substring-matching the error message.

use linkedin_api::error::{Error as ApiError, ProfileResolutionFailure};

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
            // A slug that will never resolve is bad input, not a failing
            // API. Scripts can branch on this without parsing the message.
            CliError::Api(ApiError::ProfileResolution { reason, .. }) => match reason {
                ProfileResolutionFailure::NotFound
                | ProfileResolutionFailure::SelfSlugUnsupported => exit_code::INPUT,
                ProfileResolutionFailure::RateLimited | ProfileResolutionFailure::Inconclusive => {
                    exit_code::API
                }
            },
            CliError::Api(_) => exit_code::API,
            CliError::Input(_) => exit_code::INPUT,
            CliError::Cache(_) | CliError::Other(_) => 1,
        }
    }

    /// Optional actionable next step. Matched on the typed variant rather
    /// than substring-matched against the formatted message.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            CliError::Api(ApiError::Auth(_)) | CliError::Session(_) => Some(STALE_SESSION_HINT),
            CliError::Api(ApiError::ProfileResolution { reason, .. }) => {
                Some(resolution_hint(*reason))
            }
            CliError::Api(ApiError::Api { status, body, .. }) => api_status_hint(*status, body),
            CliError::Input(msg) if msg.contains("out of range") => Some(
                "Hint: rerun the parent command (e.g. 'li search people \"…\"' or 'li feed list') first \
                 to populate the cache.",
            ),
            _ => None,
        }
    }
}

const STALE_SESSION_HINT: &str =
    "Hint: session looks stale. Refresh with: li auth login <li_at_cookie>";

/// Next step for an `ApiError::Api` response, keyed on the HTTP status.
fn api_status_hint(status: u16, body: &str) -> Option<&'static str> {
    match status {
        401 => Some(STALE_SESSION_HINT),
        301 => Some(
            "Hint: HTTP 301 from a Voyager endpoint usually means LinkedIn retired it. \
             Check re/ docs for the modern path; if the path is still listed there, file an issue.",
        ),
        403 => Some(
            "Hint: LinkedIn blocked the request. This often means a captcha challenge — \
             open the site in a browser and complete any pending verification.",
        ),
        429 => Some(
            "Hint: rate limited. Wait a few minutes; consider lowering --pacing-ms below 2000 \
             only if you've checked your quota.",
        ),
        200 if is_transient_graphql_error(body) => Some(
            "Hint: transient GraphQL error. The client already retried up to MAX_RETRIES \
             times — if you see this often, LinkedIn's mesh is degraded. \
             Try again in a minute.",
        ),
        _ => None,
    }
}

/// True when a 200-status GraphQL body carries a known transient mesh error.
fn is_transient_graphql_error(body: &str) -> bool {
    body.contains("Internal error fetching data from downstream")
        || body.contains("Failed to get response from server")
}

/// Next step for each slug → URN resolution failure.
///
/// One hint per classification, because the recoveries are opposites:
/// waiting helps exactly one of these and wastes time on the rest.
fn resolution_hint(reason: ProfileResolutionFailure) -> &'static str {
    match reason {
        ProfileResolutionFailure::RateLimited => {
            "Hint: the resolver was rate limited (HTTP 429, or LinkedIn's 999 challenge status). \
             Wait a few minutes and retry the same slug; do not re-run it in a loop, which is \
             what earned the block."
        }
        ProfileResolutionFailure::SelfSlugUnsupported => {
            "Hint: that is your own vanity slug. The slug resolvers answer for other members only, \
             so retrying will never work. Use 'li profile me' for your own profile, or pass your \
             fsd_profile URN directly."
        }
        ProfileResolutionFailure::NotFound => {
            "Hint: no endpoint knows that slug. Check the spelling against the profile URL, or the \
             member renamed or removed the profile."
        }
        ProfileResolutionFailure::Inconclusive => {
            "Hint: the per-endpoint outcomes in the message say what each endpoint returned. A 403 \
             usually means a captcha challenge (open LinkedIn in a browser); a 5xx or a transport \
             error means retry later."
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
            correlation_id: None,
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

    fn resolution_err(reason: ProfileResolutionFailure) -> CliError {
        CliError::Api(ApiError::ProfileResolution {
            public_id: "olgasafonova".to_string(),
            reason,
            attempts: Vec::new(),
        })
    }

    #[test]
    fn exit_code_splits_resolution_failures() {
        assert_eq!(
            resolution_err(ProfileResolutionFailure::NotFound).exit_code(),
            exit_code::INPUT
        );
        assert_eq!(
            resolution_err(ProfileResolutionFailure::SelfSlugUnsupported).exit_code(),
            exit_code::INPUT
        );
        assert_eq!(
            resolution_err(ProfileResolutionFailure::RateLimited).exit_code(),
            exit_code::API
        );
        assert_eq!(
            resolution_err(ProfileResolutionFailure::Inconclusive).exit_code(),
            exit_code::API
        );
    }

    #[test]
    fn resolution_hints_are_distinct_and_actionable() {
        let hints = [
            ProfileResolutionFailure::RateLimited,
            ProfileResolutionFailure::SelfSlugUnsupported,
            ProfileResolutionFailure::NotFound,
            ProfileResolutionFailure::Inconclusive,
        ]
        .map(|r| resolution_err(r).hint().expect("every reason has a hint"));

        assert!(hints[0].contains("Wait a few minutes"));
        assert!(hints[1].contains("li profile me"));
        assert!(hints[2].contains("spelling"));
        assert!(hints[3].contains("captcha"));

        let unique: std::collections::HashSet<&&str> = hints.iter().collect();
        assert_eq!(unique.len(), hints.len(), "hints must not be duplicates");
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
            correlation_id: None,
        };
        let cli_err: CliError = lib_err.into();
        assert_eq!(cli_err.exit_code(), exit_code::API);
    }
}
