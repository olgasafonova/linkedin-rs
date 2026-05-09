//! On-disk feed cache used by `feed read` / `feed react` / `feed comment` /
//! `feed reactions --from-list`.

use serde_json::Value;

fn feed_cache_path() -> Result<std::path::PathBuf, String> {
    let data_dir =
        dirs::data_dir().ok_or_else(|| "could not determine data directory".to_string())?;
    Ok(data_dir.join("linkedin").join("last_feed.json"))
}

pub(super) fn save_feed_cache(value: &Value) -> Result<(), String> {
    let path = feed_cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create cache dir: {e}"))?;
    }
    let json =
        serde_json::to_string(value).map_err(|e| format!("failed to serialize feed cache: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write feed cache: {e}"))?;
    Ok(())
}

pub(super) fn load_feed_cache() -> Result<Value, String> {
    let path = feed_cache_path()?;
    let data = std::fs::read_to_string(&path)
        .map_err(|_| "no cached feed. Run `feed list` or `feed my-posts` first.".to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("failed to parse feed cache: {e}"))
}

/// Load a 1-based feed-cache element by index. Used by reactions + comments.
pub(super) fn cached_feed_element(index: usize) -> Result<Value, String> {
    if index == 0 {
        return Err("index must be >= 1".to_string());
    }
    let cache = load_feed_cache()?;
    let elements = cache
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "cached feed has no elements array".to_string())?;
    elements.get(index - 1).cloned().ok_or_else(|| {
        format!(
            "index {} out of range (cached feed has {} items)",
            index,
            elements.len()
        )
    })
}
