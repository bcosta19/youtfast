//! Changelog extraction and Markdown normalization for update release notes.

const BUNDLED_CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");

/// Extracts and cleans the changelog section for `version` from the bundled
/// `CHANGELOG.md`, falling back to `Unreleased` highlights if needed.
pub fn bundled_changelog_for(version: &str) -> String {
    extract_changelog_section(BUNDLED_CHANGELOG, version)
        .or_else(|| extract_changelog_section(BUNDLED_CHANGELOG, "Unreleased"))
        .unwrap_or_default()
}

pub fn extract_changelog_section(markdown: &str, version: &str) -> Option<String> {
    let target = format!("## [{version}]");
    let mut inside = false;
    let mut lines = Vec::new();
    for line in markdown.lines() {
        if line.starts_with("## [") {
            if inside {
                break;
            }
            if line.starts_with(&target) {
                inside = true;
            }
            continue;
        }
        if inside {
            if line.starts_with('[') && line.contains("]: http") {
                continue;
            }
            lines.push(line);
        }
    }
    let cleaned = clean_changelog(&lines.join("\n"));
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Normalizes Markdown release notes into clean, readable lines for the UI.
pub fn clean_changelog(markdown: &str) -> String {
    let mut out = Vec::new();
    let mut current_bullet = String::new();
    let flush_bullet = |bullet: &mut String, out: &mut Vec<String>| {
        if !bullet.is_empty() {
            out.push(format!("• {}", strip_markdown_links(bullet.trim())));
            bullet.clear();
        }
    };
    for raw_line in markdown.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            flush_bullet(&mut current_bullet, &mut out);
            continue;
        }
        if line.starts_with('[') && line.contains("]: http") {
            continue;
        }
        if let Some(heading) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
        {
            flush_bullet(&mut current_bullet, &mut out);
            if !out.is_empty() {
                out.push(String::new());
            }
            out.push(strip_markdown_links(heading.trim()));
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush_bullet(&mut current_bullet, &mut out);
            current_bullet.push_str(item.trim());
        } else if !current_bullet.is_empty() {
            current_bullet.push(' ');
            current_bullet.push_str(line);
        } else {
            out.push(strip_markdown_links(line));
        }
    }
    flush_bullet(&mut current_bullet, &mut out);
    out.join("\n").trim().to_owned()
}

fn strip_markdown_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        result.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        if let Some(close) = after_open.find(']') {
            let label = &after_open[..close];
            let after_close = &after_open[close + 1..];
            if !label.contains('[')
                && let Some(inside_paren) = after_close.strip_prefix('(')
                && let Some(end_paren) = inside_paren.find(')')
            {
                result.push_str(label);
                rest = &inside_paren[end_paren + 1..];
                continue;
            }
        }
        result.push('[');
        rest = after_open;
    }
    result.push_str(rest);
    result.replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::CURRENT_VERSION;

    #[test]
    fn bundled_changelog_extracts_current_version_notes() {
        let notes = bundled_changelog_for(CURRENT_VERSION);
        let current = extract_changelog_section(BUNDLED_CHANGELOG, CURRENT_VERSION)
            .expect("the compiled version has its own changelog section");
        assert!(!current.is_empty());
        assert_eq!(notes, current);
        let notes_030 = bundled_changelog_for("0.3.0");
        assert!(notes_030.contains("About section and built-in updater"));
        let notes_020 = bundled_changelog_for("0.2.0");
        assert!(notes_020.contains("Spanish, French, German and Italian"));
    }

    #[test]
    fn strip_markdown_links_preserves_bracketed_ui_labels() {
        let line = "- Added `[ save ]` button in [watch page](https://example.com/watch) and `[ load more ]`.";
        let cleaned = strip_markdown_links(line);
        assert_eq!(
            cleaned,
            "- Added [ save ] button in watch page and [ load more ]."
        );
    }
}
