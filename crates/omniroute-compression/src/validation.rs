//! Compression validation — safety/fidelity checks.
//! Port of `open-sse/services/compression/validation.ts`.

use crate::preservation::find_fenced_code_blocks;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub fallback_applied: bool,
}

fn is_preview_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0C' | '\x0B')
}

fn is_horizontal_whitespace(ch: char) -> bool {
    ch == ' ' || ch == '\t'
}

fn is_ascii_digit_opt(ch: Option<char>) -> bool {
    matches!(ch, Some(c) if c.is_ascii_digit())
}

fn is_ascii_upper_opt(ch: Option<char>) -> bool {
    matches!(ch, Some(c) if c.is_ascii_uppercase())
}

fn is_word_char_opt(ch: Option<char>) -> bool {
    matches!(ch, Some(c) if c.is_ascii_alphanumeric() || c == '_')
}

fn is_url_terminator(ch: char) -> bool {
    is_preview_whitespace(ch) || matches!(ch, ')' | ']' | '"' | '\'' | '>')
}

fn collapse_whitespace_for_preview(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut previous_was_whitespace = false;
    for ch in text.chars() {
        if is_preview_whitespace(ch) {
            if !previous_was_whitespace {
                output.push(' ');
            }
            previous_was_whitespace = true;
            continue;
        }
        output.push(ch);
        previous_was_whitespace = false;
    }
    output
}

fn require_exact_presence(
    label: &str,
    original_items: &[String],
    compressed: &str,
    errors: &mut Vec<String>,
) {
    for item in original_items {
        if !compressed.contains(item) {
            let preview: String = collapse_whitespace_for_preview(item)
                .chars()
                .take(80)
                .collect();
            errors.push(format!("{} changed or missing: {}", label, preview));
        }
    }
}

pub fn validate_compression(original: &str, compressed: &str) -> ValidationResult {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // TS checks typeof string, but Rust signature enforces &str.

    if !original.is_empty() && compressed.trim().is_empty() {
        errors.push("compressed text is empty".to_string());
    }

    let orig_fenced = find_fenced_code_blocks(original);
    require_exact_presence("fenced code block", &orig_fenced, compressed, &mut errors);
    require_exact_presence(
        "inline code",
        &collect_inline_code(original),
        compressed,
        &mut errors,
    );
    require_exact_presence("URL", &collect_urls(original), compressed, &mut errors);
    require_exact_presence(
        "markdown link",
        &collect_markdown_links(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "frontmatter",
        &collect_frontmatter(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "heading",
        &collect_headings(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "table row",
        &collect_table_rows(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "math block",
        &collect_math_blocks(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "inline math",
        &collect_inline_math(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "LaTeX block",
        &collect_latex_blocks(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "version",
        &collect_versions(original),
        compressed,
        &mut errors,
    );
    require_exact_presence(
        "CONST_CASE",
        &collect_const_case(original),
        compressed,
        &mut errors,
    );

    let original_fence_count = orig_fenced.len();
    let compressed_fence_count = find_fenced_code_blocks(compressed).len();
    if compressed_fence_count < original_fence_count {
        errors.push(format!(
            "fenced code block count dropped: {} -> {}",
            original_fence_count, compressed_fence_count
        ));
    }

    if compressed.len() > original.len() {
        warnings.push("compressed text is longer than original".to_string());
    }

    let valid = errors.is_empty();
    ValidationResult {
        fallback_applied: !valid,
        valid,
        errors,
        warnings,
    }
}

fn collect_inline_code(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] != '`' {
            index += 1;
            continue;
        }
        // find next backtick
        let mut end = None;
        let mut j = index + 1;
        while j < chars.len() {
            if chars[j] == '`' {
                end = Some(j);
                break;
            }
            j += 1;
        }
        let Some(end) = end else { break };
        let content: String = chars[index + 1..end].iter().collect();
        if !content.is_empty() && !content.contains('\n') {
            let m: String = chars[index..=end].iter().collect();
            matches.push(m);
            index = end + 1;
        } else {
            index += 1;
        }
    }
    matches
}

fn collect_urls(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let starts_http = starts_with_at(&chars, index, "http://");
        let starts_https = starts_with_at(&chars, index, "https://");
        if !starts_http && !starts_https {
            index += 1;
            continue;
        }
        if index > 0 && is_word_char_opt(chars.get(index - 1).copied()) {
            index += 1;
            continue;
        }
        let mut end = index
            + if starts_https {
                "https://".len()
            } else {
                "http://".len()
            };
        while end < chars.len() && !is_url_terminator(chars[end]) {
            end += 1;
        }
        if end > index {
            matches.push(chars[index..end].iter().collect());
            if end == 0 {
                index += 1;
            } else {
                index = end; // will be incremented? TS does index = end - 1 then loop index++
            }
            continue;
        }
        index += 1;
    }
    matches
}

fn starts_with_at(chars: &[char], index: usize, s: &str) -> bool {
    let pat: Vec<char> = s.chars().collect();
    if index + pat.len() > chars.len() {
        return false;
    }
    chars[index..index + pat.len()] == pat[..]
}

fn collect_markdown_links(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] != '[' {
            index += 1;
            continue;
        }
        // find label end ']'
        let mut label_end = None;
        let mut j = index + 1;
        while j < chars.len() {
            if chars[j] == ']' {
                label_end = Some(j);
                break;
            }
            j += 1;
        }
        let Some(label_end) = label_end else {
            index += 1;
            continue;
        };
        if label_end - index - 1 > 1000 {
            index += 1;
            continue;
        }
        let label_slice = &chars[index + 1..label_end];
        if label_slice.contains(&'\n') {
            index += 1;
            continue;
        }
        if label_end + 1 >= chars.len() || chars[label_end + 1] != '(' {
            index += 1;
            continue;
        }
        let target_start = label_end + 2;
        let max_target_end = std::cmp::min(chars.len(), target_start + 2000);
        let mut target_end: Option<usize> = None;
        let mut cursor = target_start;
        while cursor < max_target_end {
            if chars[cursor] == '\n' {
                break;
            }
            if chars[cursor] == ')' {
                target_end = Some(cursor);
                break;
            }
            cursor += 1;
        }
        if let Some(target_end) = target_end {
            matches.push(chars[index..=target_end].iter().collect());
            index = target_end + 1;
            continue;
        }
        index += 1;
    }
    matches
}

fn collect_frontmatter(text: &str) -> Vec<String> {
    if !text.starts_with("---\n") {
        return Vec::new();
    }
    // find "\n---" starting at 4
    if let Some(rel_close) = text[4..].find("\n---") {
        let close = 4 + rel_close; // index of '\n' before closing ---
        let after_close = close + 4; // after "\n---"
        let close_end = text[after_close..].find('\n').map(|p| after_close + p);
        let end = match close_end {
            Some(e) => e + 1,
            None => text.len(),
        };
        return vec![text[0..end].to_string()];
    }
    Vec::new()
}

fn collect_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if bytes[idx] == b'\n' {
            lines.push(text[start..idx].to_string());
            start = idx + 1;
        }
        idx += 1;
    }
    lines.push(text[start..].to_string());
    lines
}

fn collect_headings(text: &str) -> Vec<String> {
    collect_lines(text)
        .into_iter()
        .filter(|line| {
            let chars: Vec<char> = line.chars().collect();
            let mut marker_count = 0usize;
            while marker_count < chars.len() && chars[marker_count] == '#' {
                marker_count += 1;
            }
            if !(1..=6).contains(&marker_count) {
                return false;
            }
            if marker_count >= chars.len() || !is_preview_whitespace(chars[marker_count]) {
                return false;
            }
            let mut i = marker_count + 1;
            while i < chars.len() {
                if !is_preview_whitespace(chars[i]) {
                    return true;
                }
                i += 1;
            }
            false
        })
        .collect()
}

fn collect_table_rows(text: &str) -> Vec<String> {
    collect_lines(text)
        .into_iter()
        .filter(|line| {
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0usize;
            while i < chars.len() && is_horizontal_whitespace(chars[i]) {
                i += 1;
            }
            if i >= chars.len() || chars[i] != '|' {
                return false;
            }
            let pipe_count = chars.iter().filter(|&&c| c == '|').count();
            pipe_count >= 2
        })
        .collect()
}

fn collect_math_blocks(text: &str) -> Vec<String> {
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < text.len() {
        let Some(rel_start) = text[index..].find("$$") else {
            break;
        };
        let start = index + rel_start;
        let search_from = start + 2;
        if search_from > text.len() {
            break;
        }
        let Some(rel_end) = text[search_from..].find("$$") else {
            break;
        };
        let end = search_from + rel_end;
        if end >= start + 2 && end - start - 2 <= 10000 {
            matches.push(text[start..end + 2].to_string());
            index = end + 2;
        } else {
            index = start + 2;
        }
    }
    matches
}

fn collect_inline_math(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] != '$' {
            index += 1;
            continue;
        }
        if index > 0 && chars[index - 1] == '$' {
            index += 1;
            continue;
        }
        let next = chars.get(index + 1).copied();
        if next.is_none() {
            index += 1;
            continue;
        }
        let next = next.unwrap();
        if is_preview_whitespace(next) || next == '$' || next.is_ascii_digit() {
            index += 1;
            continue;
        }
        let max_end = std::cmp::min(chars.len(), index + 1 + 160);
        let mut cursor = index + 1;
        let mut found = false;
        while cursor < max_end {
            let ch = chars[cursor];
            if ch == '\n' {
                break;
            }
            if ch == '\\' {
                cursor += 2;
                continue;
            }
            if ch != '$' {
                cursor += 1;
                continue;
            }
            if cursor > 0 && is_preview_whitespace(chars[cursor - 1]) {
                cursor += 1;
                continue;
            }
            if chars.get(cursor + 1).copied() == Some('$') {
                cursor += 1;
                continue;
            }
            matches.push(chars[index..=cursor].iter().collect());
            index = cursor + 1;
            found = true;
            break;
        }
        if !found {
            index += 1;
        }
    }
    matches
}

fn collect_latex_blocks(text: &str) -> Vec<String> {
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < text.len() {
        let Some(rel_start) = text[index..].find("\\begin{") else {
            break;
        };
        let start = index + rel_start;
        let env_start = start + "\\begin{".len();
        if env_start > text.len() {
            break;
        }
        let Some(rel_env_end) = text[env_start..].find('}') else {
            index = env_start;
            continue;
        };
        let env_end = env_start + rel_env_end;
        let env = &text[env_start..env_end];
        if env.is_empty()
            || env.len() > 50
            || !env.chars().all(|c| c.is_ascii_alphabetic() || c == '*')
        {
            index = env_start;
            continue;
        }
        let close_token = format!("\\end{{{}}}", env);
        let search_close_from = env_end + 1;
        if search_close_from > text.len() {
            break;
        }
        let Some(rel_close) = text[search_close_from..].find(&close_token) else {
            index = env_end + 1;
            continue;
        };
        let close_start = search_close_from + rel_close;
        if close_start > env_end && close_start - env_end - 1 <= 10000 {
            let end = close_start + close_token.len();
            matches.push(text[start..end].to_string());
            index = end;
        } else {
            index = env_end + 1;
        }
    }
    matches
}

fn read_digits(chars: &[char], start: usize) -> usize {
    let mut cursor = start;
    while cursor < chars.len() && chars[cursor].is_ascii_digit() {
        cursor += 1;
    }
    cursor
}

fn is_version_suffix_char_opt(ch: Option<char>) -> bool {
    matches!(ch, Some(c) if c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn collect_versions(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        if !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        if index > 0 && is_word_char_opt(chars.get(index - 1).copied()) {
            index += 1;
            continue;
        }
        let mut cursor = read_digits(&chars, index);
        let mut dot_groups = 0;
        while dot_groups < 3
            && chars.get(cursor).copied() == Some('.')
            && is_ascii_digit_opt(chars.get(cursor + 1).copied())
        {
            cursor = read_digits(&chars, cursor + 1);
            dot_groups += 1;
        }
        if dot_groups < 1 {
            index += 1;
            continue;
        }
        if matches!(chars.get(cursor).copied(), Some('-') | Some('+'))
            && is_version_suffix_char_opt(chars.get(cursor + 1).copied())
        {
            cursor += 1;
            while cursor < chars.len() && is_version_suffix_char_opt(chars.get(cursor).copied()) {
                cursor += 1;
            }
        }
        if is_word_char_opt(chars.get(cursor).copied()) {
            index += 1;
            continue;
        }
        matches.push(chars[index..cursor].iter().collect());
        index = cursor;
    }
    matches
}

fn collect_const_case(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        if !chars[index].is_ascii_uppercase() {
            index += 1;
            continue;
        }
        if index > 0 && is_word_char_opt(chars.get(index - 1).copied()) {
            index += 1;
            continue;
        }
        let mut cursor = index + 1;
        let mut has_underscore = false;
        while cursor < chars.len() {
            let ch = chars[cursor];
            if ch == '_' {
                let next = chars.get(cursor + 1).copied();
                if !is_ascii_upper_opt(next) && !is_ascii_digit_opt(next) {
                    break;
                }
                has_underscore = true;
                cursor += 1;
                continue;
            }
            if !ch.is_ascii_uppercase() && !ch.is_ascii_digit() {
                break;
            }
            cursor += 1;
        }
        if !has_underscore || is_word_char_opt(chars.get(cursor).copied()) {
            index += 1;
            continue;
        }
        matches.push(chars[index..cursor].iter().collect());
        index = cursor;
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_validation_failure_when_protected_content_lost() {
        let original = "Use `exact_token` and https://example.com/the/path.";
        let validation = validate_compression(original, "Use token.");
        assert!(!validation.valid);
        assert!(validation.fallback_applied);
        assert!(validation.errors.len() >= 2);
    }

    #[test]
    fn formats_validation_previews_with_linear_whitespace_collapsing() {
        let original = format!("Use `{}exact_token` in the request.", "\t".repeat(5000));
        let validation = validate_compression(&original, "Use token.");
        assert!(!validation.valid);
        let joined = validation.errors.join("\n");
        assert!(
            joined.contains("inline code changed or missing: ` exact_token`"),
            "actual errors: {joined:?}"
        );
    }

    #[test]
    fn warns_when_compressed_longer() {
        let original = "short";
        let compressed = "shorter but actually longer text here";
        let v = validate_compression(original, compressed);
        assert!(v
            .warnings
            .iter()
            .any(|w| w.contains("longer than original")));
    }

    #[test]
    fn preserves_structural_elements_presence_checks() {
        let original = "See [link](https://a.test/x)\n# Heading\n| a | b |\n\n```rs\nfn x(){}\n```\nCONST_CASE $$x$$ \\begin{align}a\\end{align} 1.2.3";
        let v = validate_compression(original, original);
        assert!(v.valid, "errors: {:?}", v.errors);
    }
}
