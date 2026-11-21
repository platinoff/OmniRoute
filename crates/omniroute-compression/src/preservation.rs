//! Preservation helpers ported from `open-sse/services/compression/preservation.ts`.
//! Wave 3 scope: fenced code block extraction helpers used by validation.

/// Extract fenced code blocks, calling `add_block` for each complete block and
/// returning the transformed text (block contents replaced by callback result).
pub fn extract_fenced_code_blocks<F>(text: &str, mut add_block: F) -> String
where
    F: FnMut(&str) -> String,
{
    // Equivalent to JS: text.match(/[^\n]*(?:\n|$)/g) ?? [text]
    let mut lines: Vec<&str> = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            lines.push(&text[start..=i]);
            start = i + 1;
        }
        i += 1;
    }
    if start <= text.len() {
        lines.push(&text[start..]);
    }
    if lines.is_empty() {
        lines.push(text);
    }

    let mut output = String::with_capacity(text.len());
    let mut idx = 0usize;

    while idx < lines.len() {
        let line = lines[idx];
        if line.is_empty() && idx == lines.len() - 1 {
            break;
        }

        if let Some((fence_char, min_len)) = opening_fence(line) {
            let mut block = String::new();
            block.push_str(line);
            let mut j = idx + 1;
            let mut closed = false;
            while j < lines.len() {
                let candidate = lines[j];
                block.push_str(candidate);
                if let Some((close_char, close_len)) = closing_fence(candidate) {
                    if close_char == fence_char && close_len >= min_len {
                        closed = true;
                        break;
                    }
                }
                j += 1;
            }

            if !closed {
                output.push_str(line);
                idx += 1;
                continue;
            }

            output.push_str(&add_block(&block));
            idx = j + 1;
            continue;
        }

        output.push_str(line);
        idx += 1;
    }

    output
}

fn is_indent_ws(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Match opening fence: /^([ \t]{0,3})(`{3,}|~{3,})[^\n]*(?:\n|$)/
/// Returns (fence_char, fence_len).
fn opening_fence(line: &str) -> Option<(u8, usize)> {
    let b = line.as_bytes();
    let mut i = 0usize;
    // up to 3 indent spaces/tabs
    let mut indent = 0;
    while i < b.len() && indent < 3 && is_indent_ws(b[i]) {
        i += 1;
        indent += 1;
    }
    if i >= b.len() {
        return None;
    }
    let ch = b[i];
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let mut len = 0usize;
    while i < b.len() && b[i] == ch {
        len += 1;
        i += 1;
    }
    if len >= 3 {
        Some((ch, len))
    } else {
        None
    }
}

/// Match closing fence: /^([ \t]{0,3})(`{3,}|~{3,})\s*(?:\n|$)/
fn closing_fence(line: &str) -> Option<(u8, usize)> {
    let b = line.as_bytes();
    let mut i = 0usize;
    let mut indent = 0;
    while i < b.len() && indent < 3 && is_indent_ws(b[i]) {
        i += 1;
        indent += 1;
    }
    if i >= b.len() {
        return None;
    }
    let ch = b[i];
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let mut len = 0usize;
    while i < b.len() && b[i] == ch {
        len += 1;
        i += 1;
    }
    if len < 3 {
        return None;
    }
    // remaining must be whitespace only until optional \n/end
    while i < b.len() {
        let c = b[i];
        if c == b'\n' || c == b'\r' {
            break;
        }
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }
        return None;
    }
    Some((ch, len))
}

/// Return the complete fenced code block strings found in `text`.
pub fn find_fenced_code_blocks(text: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    extract_fenced_code_blocks(text, |content| {
        blocks.push(content.to_string());
        content.to_string()
    });
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_backtick_and_tilde_fences() {
        let input = "Before\n````markdown\n```js\nconst theValue = 1;\n```\n````\n~~~ts\nconst x = `the`;\n~~~\nAfter";
        let blocks = find_fenced_code_blocks(input);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("````markdown"));
        assert!(blocks[1].contains("~~~ts"));
    }

    #[test]
    fn no_fences() {
        assert!(find_fenced_code_blocks("just text\nlines").is_empty());
    }
}
