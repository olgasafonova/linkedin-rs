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

/// Load the stored session and create an authenticated client.
pub fn load_session_client() -> Result<(LinkedInClient, std::path::PathBuf), String> {
    // Check for browser cookies file first (enables write operations).
    let cookies_path = std::path::Path::new("secrets/browser_cookies.json");
    if cookies_path.exists() {
        let data = std::fs::read_to_string(cookies_path)
            .map_err(|e| format!("failed to read browser cookies: {e}"))?;
        let cookies: std::collections::HashMap<String, String> = serde_json::from_str(&data)
            .map_err(|e| format!("failed to parse browser cookies: {e}"))?;
        let client = LinkedInClient::with_browser_cookies(&cookies)
            .map_err(|e| format!("client error: {e}"))?;
        let (_, path) = load_session()?;
        eprintln!("Using browser cookies from secrets/browser_cookies.json");
        return Ok((client, path));
    }

    let (session, path) = load_session()?;
    let client =
        LinkedInClient::with_session(&session).map_err(|e| format!("client error: {e}"))?;
    Ok((client, path))
}
