use linkedin_api::auth::Session;
use linkedin_api::client::LinkedInClient;

/// Load the stored session or return a descriptive error.
///
/// Checks for session validity and prints a warning to stderr if the
/// session is old enough to be potentially expired.
pub fn load_session() -> Result<(Session, std::path::PathBuf), String> {
    let path = Session::default_path().map_err(|e| format!("{e}"))?;

    if !path.exists() {
        return Err(format!(
            "no session found at {} -- run `auth login` first",
            path.display()
        ));
    }

    let session = Session::load(&path).map_err(|e| format!("{e}"))?;

    if !session.is_valid() {
        return Err("session is invalid (empty li_at cookie)".to_string());
    }

    // Warn about potentially expired sessions.
    if let Some(warning) = session.expiry_warning() {
        eprintln!("warning: {}", warning);
    }

    Ok((session, path))
}

/// Resolve the canonical browser-cookies file path.
///
/// Resolution order:
/// 1. `LINKEDIN_COOKIES_FILE` environment variable (explicit override).
/// 2. `<session_dir>/browser_cookies.json` where `session_dir` is the parent
///    directory of [`Session::default_path`].
///
/// Never resolves relative to the current working directory. Running `li`
/// from a directory that happens to contain `secrets/browser_cookies.json`
/// must not silently swap in those cookies.
fn browser_cookies_path() -> Result<std::path::PathBuf, String> {
    if let Some(override_path) = std::env::var_os("LINKEDIN_COOKIES_FILE") {
        return Ok(std::path::PathBuf::from(override_path));
    }
    let session_path = Session::default_path().map_err(|e| format!("{e}"))?;
    let session_dir = session_path
        .parent()
        .ok_or_else(|| "session path has no parent directory".to_string())?;
    Ok(session_dir.join("browser_cookies.json"))
}

/// Load the stored session and create an authenticated client.
pub fn load_session_client() -> Result<(LinkedInClient, std::path::PathBuf), String> {
    // Check for browser cookies file first (enables write operations).
    let cookies_path = browser_cookies_path()?;
    if cookies_path.exists() {
        let data = std::fs::read_to_string(&cookies_path)
            .map_err(|e| format!("failed to read browser cookies: {e}"))?;
        let cookies: std::collections::HashMap<String, String> = serde_json::from_str(&data)
            .map_err(|e| format!("failed to parse browser cookies: {e}"))?;
        let client = LinkedInClient::with_browser_cookies(&cookies)
            .map_err(|e| format!("client error: {e}"))?;
        let (_, path) = load_session()?;
        let display = std::fs::canonicalize(&cookies_path).unwrap_or(cookies_path);
        eprintln!("Using browser cookies from {}", display.display());
        return Ok((client, path));
    }

    let (session, path) = load_session()?;
    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;
    Ok((client, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-var mutations across tests in this module — the process
    // environment is shared state, and Rust runs tests in parallel by default.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn cookies_path_uses_env_var_when_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("LINKEDIN_COOKIES_FILE", "/tmp/explicit-cookies.json");
        let path = browser_cookies_path().expect("resolution must succeed");
        std::env::remove_var("LINKEDIN_COOKIES_FILE");
        assert_eq!(path, std::path::PathBuf::from("/tmp/explicit-cookies.json"));
    }

    #[test]
    fn cookies_path_falls_back_to_session_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("LINKEDIN_COOKIES_FILE");
        let path = browser_cookies_path().expect("resolution must succeed");
        let session_dir = Session::default_path()
            .expect("default_path must succeed")
            .parent()
            .expect("session path must have a parent")
            .to_path_buf();
        assert_eq!(path, session_dir.join("browser_cookies.json"));
    }

    #[test]
    fn cookies_path_never_resolves_to_cwd() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("LINKEDIN_COOKIES_FILE");
        let path = browser_cookies_path().expect("resolution must succeed");
        assert!(
            path.is_absolute(),
            "cookies path must be absolute, got: {}",
            path.display()
        );
        let cwd_relative = std::path::Path::new("secrets/browser_cookies.json");
        assert_ne!(
            path, cwd_relative,
            "cookies path must not be the legacy CWD-relative path"
        );
    }
}
