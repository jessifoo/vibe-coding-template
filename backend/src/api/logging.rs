//! Shared API logging helpers.

/// Default max characters to include for short request previews in logs.
pub const LOG_PREVIEW_CHARS: usize = 50;

/// Truncate string for logging preview (character-safe for UTF-8).
#[must_use]
pub fn truncate_for_log(s: &str, max_chars: usize) -> String {
    let mut iter = s.chars();
    let preview: String = iter.by_ref().take(max_chars).collect();

    if iter.next().is_some() {
        return format!("{preview}...");
    }

    preview
}

#[cfg(test)]
mod tests {
    use super::{LOG_PREVIEW_CHARS, truncate_for_log};

    #[test]
    fn does_not_append_ellipsis_when_within_char_limit() {
        assert_eq!(truncate_for_log("hello", 5), "hello");
        assert_eq!(truncate_for_log("héllo", 5), "héllo");
    }

    #[test]
    fn appends_ellipsis_when_over_char_limit() {
        assert_eq!(truncate_for_log("abcdef", 5), "abcde...");
        assert_eq!(truncate_for_log("éééééé", 5), "ééééé...");
    }

    #[test]
    fn exposes_named_default_preview_length() {
        assert_eq!(LOG_PREVIEW_CHARS, 50);
    }
}
