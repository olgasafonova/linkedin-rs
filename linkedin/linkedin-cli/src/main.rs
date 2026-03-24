use std::fs;
use std::process;

use clap::{Parser, Subcommand};
use linkedin_api::auth::Session;
use linkedin_api::client::LinkedInClient;

#[derive(Parser)]
#[command(name = "linkedin-cli")]
#[command(about = "CLI for LinkedIn API", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with LinkedIn
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// View profiles
    Profile {
        /// LinkedIn profile URN or vanity name
        id: Option<String>,
    },
    /// Messaging operations
    Messages,
    /// Feed and posts
    Feed,
    /// Connection management
    Connections,
}

#[derive(Subcommand)]
enum AuthAction {
    /// Log in by providing a li_at cookie value
    Login {
        /// li_at cookie value from browser dev tools.
        /// Also accepts LINKEDIN_LI_AT environment variable.
        #[arg(long = "li-at")]
        li_at: Option<String>,
    },
    /// Show current session status
    Status,
    /// Log out and clear stored session
    Logout,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { action } => match action {
            AuthAction::Login { li_at } => {
                if let Err(e) = cmd_auth_login(li_at).await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            AuthAction::Status => {
                if let Err(e) = cmd_auth_status().await {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            AuthAction::Logout => {
                if let Err(e) = cmd_auth_logout() {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        },
        Commands::Profile { id } => {
            let target = id.as_deref().unwrap_or("me");
            println!("Profile view for '{}': not yet implemented", target);
        }
        Commands::Messages => {
            println!("Messages: not yet implemented");
        }
        Commands::Feed => {
            println!("Feed: not yet implemented");
        }
        Commands::Connections => {
            println!("Connections: not yet implemented");
        }
    }
}

/// Handle `auth login --li-at <value>`.
///
/// Resolves the li_at value from the CLI flag or the `LINKEDIN_LI_AT`
/// environment variable. Generates a fresh JSESSIONID, creates a Session,
/// and saves it to the default path.
async fn cmd_auth_login(li_at_flag: Option<String>) -> Result<(), String> {
    let li_at = li_at_flag
        .or_else(|| std::env::var("LINKEDIN_LI_AT").ok())
        .ok_or_else(|| {
            "li_at cookie value required: use --li-at <value> or set LINKEDIN_LI_AT env var"
                .to_string()
        })?;

    if li_at.trim().is_empty() {
        return Err("li_at cookie value must not be empty".to_string());
    }

    // Generate a fresh JSESSIONID for this session.
    let client = LinkedInClient::new().map_err(|e| format!("failed to create client: {e}"))?;
    let jsessionid = client.jsessionid().to_string();

    let session = Session::new(li_at, jsessionid);
    let path = Session::default_path().map_err(|e| format!("{e}"))?;
    session.save(&path).map_err(|e| format!("{e}"))?;

    println!("Session saved to {}", path.display());
    println!("JSESSIONID: {}...", &session.jsessionid[..10]);
    println!(
        "li_at: {}...",
        &session.li_at[..session.li_at.len().min(10)]
    );
    Ok(())
}

/// Handle `auth status`.
///
/// Loads the persisted session and reports whether it looks valid.
/// Does NOT make an API call to verify -- that would require network access
/// and the session might be used offline to inspect state.
async fn cmd_auth_status() -> Result<(), String> {
    let path = Session::default_path().map_err(|e| format!("{e}"))?;

    if !path.exists() {
        println!("No session found at {}", path.display());
        println!("Status: not logged in");
        return Ok(());
    }

    let session = Session::load(&path).map_err(|e| format!("{e}"))?;

    println!("Session file: {}", path.display());
    println!("Created at: {}", session.created_at);
    println!(
        "JSESSIONID: {}...",
        &session.jsessionid[..session.jsessionid.len().min(10)]
    );
    println!(
        "li_at: {}...",
        &session.li_at[..session.li_at.len().min(10)]
    );

    if session.is_valid() {
        println!("Status: valid (local check only -- session may be expired server-side)");
    } else {
        println!("Status: invalid (empty li_at cookie)");
    }

    Ok(())
}

/// Handle `auth logout`.
///
/// Deletes the session file from disk.
fn cmd_auth_logout() -> Result<(), String> {
    let path = Session::default_path().map_err(|e| format!("{e}"))?;

    if !path.exists() {
        println!("No session file found at {}", path.display());
        return Ok(());
    }

    fs::remove_file(&path).map_err(|e| format!("failed to remove session file: {e}"))?;
    println!("Session removed: {}", path.display());
    Ok(())
}
