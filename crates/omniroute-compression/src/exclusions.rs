//! Compression exclusions — per-model/endpoint exclusion filter.
//! Port of `open-sse/services/compression/exclusions.ts`.

use regex::Regex;

pub type CompressionExclusions = Vec<String>;

const MAX_EXCLUSIONS: usize = 200;

/// Escape every regex metacharacter except `*`, which is re-inserted as `.*`.
fn escape_except_wildcard(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    for ch in pattern.chars() {
        match ch {
            '.' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '*' => out.push_str(".*"),
            _ => out.push(ch),
        }
    }
    out
}

/// Compile a single normalized (lowercased/trimmed) pattern into a bounded regex.
fn compile_pattern(pattern: &str) -> Option<Regex> {
    let escaped = escape_except_wildcard(pattern);
    Regex::new(&format!("^{}$", escaped)).ok()
}

/// Normalize a raw settings value (as iterated strings) into a bounded list of
/// lowercase, trimmed, deduped patterns.
pub fn normalize_compression_exclusions(raw: &[Option<String>]) -> CompressionExclusions {
    let mut seen = std::collections::HashSet::new();
    let mut result: CompressionExclusions = Vec::new();
    for entry in raw {
        let Some(s) = entry else { continue };
        let trimmed = s.trim().to_lowercase();
        if trimmed.is_empty() || seen.contains(&trimmed) {
            continue;
        }
        seen.insert(trimmed.clone());
        result.push(trimmed);
        if result.len() >= MAX_EXCLUSIONS {
            break;
        }
    }
    result
}

/// Normalize from an iterator of string-like values, skipping non-string/empty entries.
/// Convenience for interop where callers can map values.
pub fn normalize_compression_exclusions_strings<'a, I>(raw: I) -> CompressionExclusions
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen = std::collections::HashSet::new();
    let mut result: CompressionExclusions = Vec::new();
    for entry in raw {
        let trimmed = entry.trim().to_lowercase();
        if trimmed.is_empty() || seen.contains(&trimmed) {
            continue;
        }
        seen.insert(trimmed.clone());
        result.push(trimmed);
        if result.len() >= MAX_EXCLUSIONS {
            break;
        }
    }
    result
}

/// True when the given target matches any configured exclusion pattern.
/// Matches case-insensitively against both the bare model id and the
/// `provider/model` composite.
pub fn is_compression_excluded(
    provider: Option<&str>,
    model: Option<&str>,
    exclusions: &CompressionExclusions,
) -> bool {
    if exclusions.is_empty() {
        return false;
    }

    let model_lc = model.unwrap_or("").trim().to_lowercase();
    let provider_lc = provider.unwrap_or("").trim().to_lowercase();
    if model_lc.is_empty() && provider_lc.is_empty() {
        return false;
    }

    let composite = if !provider_lc.is_empty() && !model_lc.is_empty() {
        Some(format!("{}/{}", provider_lc, model_lc))
    } else {
        None
    };

    for pattern in exclusions {
        if pattern.is_empty() {
            continue;
        }
        if let Some(regex) = compile_pattern(pattern) {
            if !model_lc.is_empty() && regex.is_match(&model_lc) {
                return true;
            }
            if let Some(ref c) = composite {
                if regex.is_match(c) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_model_true_non_excluded_false() {
        let exclusions = normalize_compression_exclusions_strings(["text-embedding-3-large"]);
        assert!(is_compression_excluded(
            None,
            Some("text-embedding-3-large"),
            &exclusions
        ));
        assert!(!is_compression_excluded(None, Some("gpt-5-6"), &exclusions));
    }

    #[test]
    fn composite_and_bare_patterns() {
        let composite = normalize_compression_exclusions_strings(["openai/text-embedding-3-large"]);
        assert!(is_compression_excluded(
            Some("openai"),
            Some("text-embedding-3-large"),
            &composite
        ));
        assert!(!is_compression_excluded(
            None,
            Some("text-embedding-3-large"),
            &composite
        ));

        let bare = normalize_compression_exclusions_strings(["text-embedding-3-large"]);
        assert!(is_compression_excluded(
            Some("openai"),
            Some("text-embedding-3-large"),
            &bare
        ));
    }

    #[test]
    fn wildcards() {
        let provider_wildcard = normalize_compression_exclusions_strings(["openai/*"]);
        assert!(is_compression_excluded(
            Some("openai"),
            Some("gpt-5-6"),
            &provider_wildcard
        ));
        assert!(!is_compression_excluded(
            Some("anthropic"),
            Some("claude-opus"),
            &provider_wildcard
        ));

        let everything = normalize_compression_exclusions_strings(["*"]);
        assert!(is_compression_excluded(
            Some("anthropic"),
            Some("claude-opus"),
            &everything
        ));
        assert!(is_compression_excluded(None, Some("anything"), &everything));
    }

    #[test]
    fn case_insensitive_match() {
        let exclusions = normalize_compression_exclusions_strings(["OpenAI/GPT-5-6"]);
        assert!(is_compression_excluded(
            Some("openai"),
            Some("gpt-5-6"),
            &exclusions
        ));
        assert!(is_compression_excluded(
            Some("OPENAI"),
            Some("GPT-5-6"),
            &exclusions
        ));
    }

    #[test]
    fn empty_absent_exclusions_false() {
        assert!(!is_compression_excluded(None, Some("gpt-5-6"), &Vec::new()));
    }

    #[test]
    fn regex_metacharacters_escaped() {
        let exclusions = normalize_compression_exclusions_strings(["gpt-5.6"]);
        assert!(is_compression_excluded(None, Some("gpt-5.6"), &exclusions));
        assert!(!is_compression_excluded(None, Some("gpt-5x6"), &exclusions));
    }

    #[test]
    fn normalize_drops_non_strings_dedupes_and_trims() {
        // Simulate mixed raw input by pre-filtering strings as callers would.
        let raw = vec![
            Some("gpt-5-6".to_string()),
            Some("GPT-5-6".to_string()),
            Some(" gpt-5-6 ".to_string()),
            None,
            Some("claude-opus".to_string()),
        ];
        let result = normalize_compression_exclusions(&raw);
        assert_eq!(result, vec!["gpt-5-6", "claude-opus"]);

        let result2 = normalize_compression_exclusions_strings([
            "gpt-5-6",
            "GPT-5-6",
            " gpt-5-6 ",
            "claude-opus",
        ]);
        assert_eq!(result2, vec!["gpt-5-6", "claude-opus"]);
    }
}
