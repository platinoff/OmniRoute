//! Lite compression text primitives ported from `open-sse/services/compression/lite.ts`.
//! Pure, deterministic whitespace and truncation helpers.

pub const TOOL_TRUNCATION_LOOKBACK: usize = 80;

/// Trim trailing horizontal whitespace (space/tab) from a line.
pub fn trim_trailing_horizontal_whitespace(line: &str) -> &str {
    line.trim_end_matches([' ', '\t'])
}

/// Collapse runs of newlines to at most two consecutive newlines, preserving other chars.
pub fn collapse_newline_runs(content: &str) -> String {
    let mut normalized = String::with_capacity(content.len());
    let mut newline_run: usize = 0;
    for ch in content.chars() {
        if ch == '\n' {
            newline_run += 1;
            if newline_run <= 2 {
                normalized.push(ch);
            }
            continue;
        }
        newline_run = 0;
        normalized.push(ch);
    }
    normalized
}

/// Normalize message whitespace: collapse newline runs and trim trailing horizontal whitespace per line.
pub fn normalize_message_whitespace(content: &str) -> String {
    collapse_newline_runs(content)
        .split('\n')
        .map(trim_trailing_horizontal_whitespace)
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_word_char_at(s: &str, byte_idx: usize) -> bool {
    // byte_idx is expected at char boundary; treat out-of-bounds as non-word.
    if byte_idx >= s.len() {
        return false;
    }
    s[byte_idx..]
        .chars()
        .next()
        .map(|c| !c.is_whitespace())
        .unwrap_or(false)
}

fn find_whitespace_backward(content: &str, cut_byte: usize) -> Option<usize> {
    let window_start = cut_byte.saturating_sub(TOOL_TRUNCATION_LOOKBACK);
    let mut i = cut_byte;
    while i > window_start {
        // move to previous char boundary
        let prev = content[..i]
            .char_indices()
            .next_back()
            .map(|(idx, ch)| (idx, ch.len_utf8()))?;
        if !is_word_char_at(content, prev.0) {
            // return slice end after whitespace char (TS returns whitespace index as exclusive end)
            return Some(prev.0 + prev.1);
        }
        if prev.0 == 0 {
            break;
        }
        i = prev.0;
    }
    None
}

fn find_whitespace_forward(content: &str, cut_byte: usize) -> Option<usize> {
    let window_end = (cut_byte + TOOL_TRUNCATION_LOOKBACK).min(content.len());
    let mut i = cut_byte;
    while i < window_end && i < content.len() {
        if !content.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let ch = content[i..].chars().next()?;
        if !is_word_char_at(content, i) {
            // return slice end after whitespace char
            return Some(i + ch.len_utf8());
        }
        i += ch.len_utf8();
    }
    None
}

/// Adjust a hard cut byte index to a nearby whitespace boundary to avoid garbling words.
/// Prefers backing off (<= limit); if none found within lookback, looks forward.
/// Falls back to original cut. `cut_byte` is clamped to char boundary/len.
/// Returned index is a slice end (exclusive), matching TS `slice(0, cutIndex)` semantics.
pub fn back_off_to_word_boundary(content: &str, cut_byte: usize) -> usize {
    let mut cut = cut_byte.min(content.len());
    while cut > 0 && !content.is_char_boundary(cut) {
        cut -= 1;
    }
    if cut == 0 || cut >= content.len() {
        return cut;
    }
    let prev_char_start = content[..cut]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let prev_is_word = is_word_char_at(content, prev_char_start);
    let curr_is_word = is_word_char_at(content, cut);
    let on_word_boundary = !prev_is_word || !curr_is_word;
    if on_word_boundary {
        return cut;
    }
    if let Some(b) = find_whitespace_backward(content, cut) {
        return b;
    }
    if let Some(f) = find_whitespace_forward(content, cut) {
        return f;
    }
    cut
}

/// Truncate tool result content to `max_len` bytes/chars-boundary-adjusted with word-boundary backoff
/// and `\n...[truncated]` suffix when truncation occurs. Mirrors `compressToolResults` string behavior.
pub fn truncate_tool_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        return content.to_string();
    }
    let cut = back_off_to_word_boundary(content, max_len);
    format!("{}\n...[truncated]", &content[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_primitives() {
        assert_eq!(trim_trailing_horizontal_whitespace("a \t"), "a");
        assert_eq!(collapse_newline_runs("a\n\n\nb\nc"), "a\n\nb\nc");
        assert_eq!(normalize_message_whitespace("a \n\n\nb\t"), "a\n\nb");
    }

    #[test]
    fn word_boundary_and_truncation() {
        let s = "hello wonderful world";
        // cut in middle of "wonderful" -> back off to end of previous word boundary (keeps space)
        let cut = 9;
        let b = back_off_to_word_boundary(s, cut);
        assert!(b <= cut);
        assert_eq!(&s[..b], "hello ");

        let t = truncate_tool_content("short", 100);
        assert_eq!(t, "short");
        let t = truncate_tool_content(s, 9);
        assert!(t.ends_with("\n...[truncated]"));
        assert!(t.starts_with("hello "));

        // forward fallback when long unbroken run before cut window has no whitespace
        let long = "supercalifragilisticexpialidocious end";
        let b = back_off_to_word_boundary(long, 5);
        assert!(b >= 5);
        assert!(b <= long.len());
        assert!(long[..b].ends_with(' ') || b == 5 || b == long.len());
    }
}
