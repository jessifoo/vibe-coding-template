//! Shared API logging helpers.

/// Truncate string for logging preview (character-safe for UTF-8).
#[must_use]
pub fn truncate_for_log(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}
