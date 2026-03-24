//! Authentication and session management.
//!
//! Handles li_at cookie-based sessions for the LinkedIn API client.
//! The li_at cookie is the primary session artifact -- its presence means
//! the user is authenticated. There is no OAuth2 token refresh; if the
//! cookie expires, the user must re-authenticate.
//!
//! See `re/auth_flow.md` for the full reverse-engineered auth flow.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// A persisted LinkedIn session containing the cookies needed for
/// authenticated API requests.
///
/// # Fields
///
/// - `li_at`: The primary session cookie set by the server after login.
/// - `jsessionid`: CSRF token in `ajax:{19-digit}` format. Auto-generated
///   client-side per `CsrfCookieHelper.generateJsessionId()`.
/// - `created_at`: When this session was saved (local clock). We cannot
///   determine server-side expiry without making an API call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Primary session cookie value (from browser dev tools or server).
    pub li_at: String,

    /// CSRF token value. Format: `ajax:{19-digit zero-padded number}`.
    pub jsessionid: String,

    /// Timestamp when this session was created/saved.
    pub created_at: DateTime<Utc>,
}

impl Session {
    /// Create a new session with the given li_at cookie and a fresh JSESSIONID.
    pub fn new(li_at: String, jsessionid: String) -> Self {
        Self {
            li_at,
            jsessionid,
            created_at: Utc::now(),
        }
    }

    /// Serialize this session to a JSON file at `path`.
    ///
    /// Creates parent directories if they don't exist. The file is written
    /// with mode 0600 on Unix to protect the session cookie.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Error::Auth(format!(
                    "failed to create session directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let json = serde_json::to_string_pretty(self)?;

        // Write the file, then restrict permissions on Unix.
        fs::write(path, &json).map_err(|e| {
            Error::Auth(format!(
                "failed to write session file {}: {e}",
                path.display()
            ))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(path, perms).map_err(|e| {
                Error::Auth(format!(
                    "failed to set permissions on {}: {e}",
                    path.display()
                ))
            })?;
        }

        Ok(())
    }

    /// Deserialize a session from a JSON file at `path`.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let json = fs::read_to_string(path).map_err(|e| {
            Error::Auth(format!(
                "failed to read session file {}: {e}",
                path.display()
            ))
        })?;
        let session: Self = serde_json::from_str(&json)?;
        Ok(session)
    }

    /// Returns the default session file path: `~/.local/share/linkedin/session.json`.
    ///
    /// Uses the `dirs` crate to resolve the platform-appropriate data directory.
    /// Falls back to `~/.local/share` on Linux if `XDG_DATA_HOME` is unset.
    pub fn default_path() -> Result<PathBuf, Error> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| Error::Auth("could not determine data directory".to_string()))?;
        Ok(data_dir.join("linkedin").join("session.json"))
    }

    /// Basic validity check: the li_at cookie is non-empty.
    ///
    /// This does NOT verify the session against LinkedIn's servers. A session
    /// can pass `is_valid()` but still be expired server-side. Use an API call
    /// (e.g., GET /voyager/api/me) to confirm the session is still active.
    pub fn is_valid(&self) -> bool {
        !self.li_at.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn session_roundtrip() {
        let dir = std::env::temp_dir().join("linkedin-test-session-roundtrip");
        let path = dir.join("session.json");

        // Clean up from any previous run.
        let _ = fs::remove_dir_all(&dir);

        let session = Session::new(
            "AQEDATest123".to_string(),
            "ajax:0000000000000000042".to_string(),
        );
        session.save(&path).expect("save must succeed");

        let loaded = Session::load(&path).expect("load must succeed");
        assert_eq!(loaded.li_at, session.li_at);
        assert_eq!(loaded.jsessionid, session.jsessionid);
        assert_eq!(loaded.created_at, session.created_at);

        // Verify file permissions on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&path).unwrap();
            assert_eq!(
                meta.permissions().mode() & 0o777,
                0o600,
                "session file must be mode 0600"
            );
        }

        // Clean up.
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_is_valid() {
        let valid = Session::new(
            "AQEDATest123".to_string(),
            "ajax:0000000000000000042".to_string(),
        );
        assert!(valid.is_valid());

        let empty = Session::new(String::new(), "ajax:0000000000000000042".to_string());
        assert!(!empty.is_valid());

        let whitespace = Session::new("   ".to_string(), "ajax:0000000000000000042".to_string());
        assert!(!whitespace.is_valid());
    }

    #[test]
    fn default_path_is_reasonable() {
        let path = Session::default_path().expect("default_path must succeed");
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains("linkedin"),
            "default path must contain 'linkedin': {path_str}"
        );
        assert!(
            path_str.ends_with("session.json"),
            "default path must end with 'session.json': {path_str}"
        );
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let path = Path::new("/tmp/linkedin-test-nonexistent/session.json");
        let result = Session::load(path);
        assert!(result.is_err());
    }
}
