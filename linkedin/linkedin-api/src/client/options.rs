//! Runtime configuration for [`LinkedInClient`](super::LinkedInClient).

use std::collections::HashMap;

/// Environment variable names consulted for a proxy URL, in priority order.
/// The LinkedIn-specific override wins so it can differ from a shell-wide
/// `HTTPS_PROXY` set for other tools.
const PROXY_ENV_VARS: &[&str] = &["LINKEDIN_PROXY_URL", "HTTPS_PROXY", "HTTP_PROXY"];

/// Optional runtime configuration for the HTTP client.
///
/// Passes environment-derived settings (currently just a proxy URL) into
/// client construction without the caller assembling the client by hand.
///
/// ```ignore
/// use linkedin_api::client::{ClientOptions, LinkedInClient};
/// let client = LinkedInClient::with_options(ClientOptions::from_env())?;
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientOptions {
    /// HTTP(S)/SOCKS proxy URL, e.g. `http://127.0.0.1:8888`. When set, API
    /// requests route through this proxy; `None` leaves reqwest on a direct
    /// connection.
    pub proxy_url: Option<String>,
}

impl ClientOptions {
    /// Build options from the process environment. See [`PROXY_ENV_VARS`]
    /// for the variables consulted and their precedence.
    pub fn from_env() -> Self {
        Self::from_env_map(&std::env::vars().collect())
    }

    /// Build options from an explicit environment map. Kept separate from
    /// [`from_env`](Self::from_env) so tests exercise the precedence rules
    /// without mutating the real process environment. A whitespace-only
    /// value counts as unset.
    pub fn from_env_map(env: &HashMap<String, String>) -> Self {
        let proxy_url = PROXY_ENV_VARS
            .iter()
            .find_map(|name| env.get(*name))
            .filter(|value| !value.trim().is_empty())
            .cloned();
        Self { proxy_url }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn empty_env_yields_no_proxy() {
        assert_eq!(
            ClientOptions::from_env_map(&env(&[])),
            ClientOptions::default()
        );
    }

    #[test]
    fn linkedin_var_wins_over_shell_proxies() {
        let opts = ClientOptions::from_env_map(&env(&[
            ("LINKEDIN_PROXY_URL", "http://specific:8888"),
            ("HTTPS_PROXY", "http://shell:3128"),
        ]));
        assert_eq!(opts.proxy_url.as_deref(), Some("http://specific:8888"));
    }

    #[test]
    fn falls_back_through_https_then_http() {
        let https = ClientOptions::from_env_map(&env(&[
            ("HTTPS_PROXY", "http://s:3128"),
            ("HTTP_PROXY", "http://h:3128"),
        ]));
        assert_eq!(https.proxy_url.as_deref(), Some("http://s:3128"));
        let http = ClientOptions::from_env_map(&env(&[("HTTP_PROXY", "http://h:3128")]));
        assert_eq!(http.proxy_url.as_deref(), Some("http://h:3128"));
    }

    #[test]
    fn blank_value_is_treated_as_unset() {
        let opts = ClientOptions::from_env_map(&env(&[("LINKEDIN_PROXY_URL", "   ")]));
        assert_eq!(opts.proxy_url, None);
    }
}
