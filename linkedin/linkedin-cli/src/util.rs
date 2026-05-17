use std::process;

use linkedin_api::models::Paging;

use crate::error::CliResult;

/// Run a command and exit with a classified error code on failure.
///
/// Exit codes come from [`CliError::exit_code`] (matched on the variant,
/// not the formatted message). Optional one-line hints from
/// [`CliError::hint`] are printed after the error line.
pub fn exit_on_err(result: CliResult<()>) {
    if let Err(e) = result {
        let hint = e.hint();
        let code = e.exit_code();
        eprintln!("error: {e}");
        if let Some(hint) = hint {
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
