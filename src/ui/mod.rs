pub mod active_window;
#[cfg(feature = "gui")]
pub mod overlay;

/// Truncate a string to at most `max` characters, appending `…` if truncated.
/// UTF-8 safe: operates on char boundaries, not byte offsets.
pub fn truncate_label(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{}…", truncated)
    }
}
