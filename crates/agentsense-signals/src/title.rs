// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Strip a single recognized agent activity glyph from a terminal title.
//!
//! Whitespace bounds the glyph, so ordinary symbols in task titles survive.

const CLAUDE_ACTIVITY_GLYPHS: &str = "·✢✳✶✻✽◐◓◑◒";

pub fn stripped_terminal_title(title: &str) -> Option<String> {
    stripped_terminal_title_with_elevation_prefix(title, cfg!(windows))
}

/// Strip a Windows elevation prefix when parsing a remote Windows terminal.
/// The caller selects the source platform independently of the host platform.
pub fn stripped_terminal_title_with_elevation_prefix(
    title: &str,
    strip_elevation_prefix: bool,
) -> Option<String> {
    let title = if strip_elevation_prefix {
        title.strip_prefix("Administrator: ").unwrap_or(title)
    } else {
        title
    }
    .trim();
    if title.is_empty() {
        return None;
    }

    let mut chars = title.char_indices();
    let (_, first) = chars.next()?;
    let after_first = &title[first.len_utf8()..];
    let recognized =
        matches!(first, '\u{2800}'..='\u{28ff}') || CLAUDE_ACTIVITY_GLYPHS.contains(first);
    let stripped = if recognized
        && (after_first.is_empty() || after_first.chars().next().is_some_and(char::is_whitespace))
    {
        after_first.trim()
    } else {
        title
    };

    (!stripped.is_empty()).then(|| stripped.to_string())
}

#[cfg(test)]
mod tests {
    use super::stripped_terminal_title;

    #[test]
    fn strips_one_recognized_leading_activity_glyph() {
        for title in [
            "⠋ task",
            "✳ task",
            "  ⠙   task  ",
            "✢ task",
            "✻ task",
            "◐ task",
            "◓ task",
            "◑ task",
            "◒ task",
        ] {
            assert_eq!(stripped_terminal_title(title).as_deref(), Some("task"));
        }
        assert_eq!(
            stripped_terminal_title("⠋ ⠙ task").as_deref(),
            Some("⠙ task")
        );
    }

    #[test]
    fn preserves_unrecognized_or_unbounded_symbols() {
        for (title, expected) in [
            ("★task", "★task"),
            ("★ production", "★ production"),
            ("✨ task", "✨ task"),
            ("☼ status", "☼ status"),
            ("@ task", "@ task"),
            ("task ⠋ detail", "task ⠋ detail"),
            ("[prod] task", "[prod] task"),
        ] {
            assert_eq!(stripped_terminal_title(title).as_deref(), Some(expected));
        }
    }

    #[test]
    fn preserves_unicode_text_and_elides_empty_results() {
        assert_eq!(
            stripped_terminal_title(" ⠋ 修复🙂标题 ").as_deref(),
            Some("修复🙂标题")
        );
        assert_eq!(stripped_terminal_title("  "), None);
        assert_eq!(stripped_terminal_title("⠋   "), None);
    }

    #[test]
    fn strips_one_windows_elevation_decoration_before_activity_glyph() {
        assert_eq!(
            super::stripped_terminal_title_with_elevation_prefix("Administrator:   ⠋ task", true)
                .as_deref(),
            Some("task")
        );
        assert_eq!(
            super::stripped_terminal_title_with_elevation_prefix(
                "Administrator: Administrator: task",
                true
            )
            .as_deref(),
            Some("Administrator: task")
        );
        assert_eq!(
            super::stripped_terminal_title_with_elevation_prefix("Administrator: ", true),
            None
        );
    }
}
