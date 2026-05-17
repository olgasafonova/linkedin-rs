use std::fs;

use linkedin_api::auth::Session;
use linkedin_api::client::LinkedInClient;

use crate::error::{CliError, CliResult};
use crate::util::truncate;

/// Handle `auth login --li-at <value>`.
///
/// Resolves the li_at value from the CLI flag or the `LINKEDIN_LI_AT`
/// environment variable. Generates a fresh JSESSIONID, creates a Session,
/// and saves it to the default path.
pub async fn cmd_auth_login(li_at_flag: Option<String>) -> CliResult<()> {
    let li_at = li_at_flag
        .or_else(|| std::env::var("LINKEDIN_LI_AT").ok())
        .ok_or_else(|| {
            "li_at cookie value required: use --li-at <value> or set LINKEDIN_LI_AT env var"
                .to_string()
        })?;

    if li_at.trim().is_empty() {
        return Err(CliError::Other(
            "li_at cookie value must not be empty".to_string(),
        ));
    }

    // Generate a fresh JSESSIONID for this session.
    let client = LinkedInClient::new()
        .map_err(|e| CliError::Other(format!("failed to create client: {e}")))?;
    let jsessionid = client.jsessionid().to_string();

    let session = Session::new(li_at, jsessionid);
    let path = Session::default_path().map_err(|e| CliError::Other(format!("{e}")))?;
    session
        .save(&path)
        .map_err(|e| CliError::Other(format!("{e}")))?;

    println!("Session saved to {}", path.display());
    println!("JSESSIONID: {}...", truncate(&session.jsessionid, 10));
    println!("li_at: {}...", truncate(&session.li_at, 10));
    Ok(())
}

/// Handle `auth status [--local]`.
///
/// Without `--local`, loads the session and calls GET /voyager/api/me to verify
/// the session is still valid server-side. With `--local`, only checks the
/// session file on disk (no network request).
pub async fn cmd_auth_status(local_only: bool) -> CliResult<()> {
    let path = Session::default_path().map_err(|e| CliError::Other(format!("{e}")))?;

    if !path.exists() {
        println!("No session found at {}", path.display());
        println!("Status: not logged in");
        return Ok(());
    }

    let session = Session::load(&path).map_err(|e| CliError::Other(format!("{e}")))?;

    println!("Session file: {}", path.display());
    println!("Created at: {}", session.created_at);
    println!("JSESSIONID: {}...", truncate(&session.jsessionid, 10));
    println!("li_at: {}...", truncate(&session.li_at, 10));

    if !session.is_valid() {
        println!("Status: invalid (empty li_at cookie)");
        return Ok(());
    }

    if local_only {
        println!("Status: valid (local check only -- session may be expired server-side)");
        return Ok(());
    }

    // Hit the live API to verify the session is actually valid.
    println!("Checking session against LinkedIn API...");
    let client = LinkedInClient::with_session(&session)
        .map_err(|e| CliError::Other(format!("client error: {e}")))?;

    match client.get_me().await {
        Ok(me) => {
            println!("Status: authenticated");
            // Try to extract a display name from the response.
            if let Some(mini) = me.get("miniProfile") {
                let first = mini.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
                let last = mini.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
                if !first.is_empty() || !last.is_empty() {
                    println!("Logged in as: {} {}", first, last);
                }
            }
            if let Some(id) = me.get("plainId").and_then(|v| v.as_i64()) {
                println!("Member ID: {}", id);
            }
            Ok(())
        }
        Err(e) => {
            println!("Status: session invalid or expired");
            println!("API error: {e}");
            Ok(())
        }
    }
}

/// Handle `auth logout`.
///
/// Deletes the session file from disk.
pub fn cmd_auth_logout() -> CliResult<()> {
    let path = Session::default_path().map_err(|e| CliError::Other(format!("{e}")))?;

    if !path.exists() {
        println!("No session file found at {}", path.display());
        return Ok(());
    }

    fs::remove_file(&path)
        .map_err(|e| CliError::Other(format!("failed to remove session file: {e}")))?;
    println!("Session removed: {}", path.display());
    Ok(())
}
