//! Ultra compression heuristic helpers ported from `open-sse/services/compression/ultraHeuristic.ts`.
//! Deterministic token scoring and pruning.

use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

fn stopwords() -> &'static HashSet<&'static str> {
    static S: OnceLock<HashSet<&'static str>> = OnceLock::new();
    S.get_or_init(|| {
        [
            "a",
            "an",
            "the",
            "is",
            "are",
            "was",
            "were",
            "be",
            "been",
            "being",
            "have",
            "has",
            "had",
            "do",
            "does",
            "did",
            "will",
            "would",
            "could",
            "should",
            "may",
            "might",
            "shall",
            "can",
            "need",
            "dare",
            "ought",
            "used",
            "i",
            "we",
            "you",
            "he",
            "she",
            "it",
            "they",
            "me",
            "us",
            "him",
            "her",
            "them",
            "my",
            "our",
            "your",
            "his",
            "its",
            "their",
            "this",
            "that",
            "these",
            "those",
            "and",
            "but",
            "or",
            "nor",
            "for",
            "yet",
            "so",
            "as",
            "at",
            "by",
            "in",
            "of",
            "on",
            "to",
            "up",
            "via",
            "with",
            "from",
            "into",
            "onto",
            "upon",
            "about",
            "just",
            "very",
            "really",
            "quite",
            "rather",
            "also",
            "too",
            "even",
            "still",
            "already",
            "always",
            "never",
            "often",
            "usually",
            "sometimes",
            "here",
            "there",
        ]
        .into_iter()
        .collect()
    })
}

fn force_preserve_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\d|https?://|[._/\\]|Error:|Exception:|```")
            .expect("valid force preserve regex")
    })
}

/// Score a token for information value (0.0 prune-candidate .. 1.0 keep).
pub fn score_token(token: &str) -> f64 {
    if force_preserve_re().is_match(token) {
        return 1.0;
    }
    let lower = token.to_ascii_lowercase();
    if stopwords().contains(lower.as_str()) {
        return 0.1;
    }
    if token.chars().count() <= 2 {
        return 0.2;
    }
    if token.chars().next().is_some_and(|c| c.is_uppercase()) {
        return 0.8;
    }
    if token.chars().count() >= 6 {
        return 0.7;
    }
    0.5
}

fn split_preserving_whitespace(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_is_ws: Option<bool> = None;

    for ch in text.chars() {
        let is_ws = ch.is_whitespace();
        match current_is_ws {
            None => {
                current_is_ws = Some(is_ws);
                current.push(ch);
            }
            Some(was_ws) if was_ws == is_ws => {
                current.push(ch);
            }
            Some(_) => {
                out.push(std::mem::take(&mut current));
                current_is_ws = Some(is_ws);
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Prune tokens to achieve target keep rate.
/// `keep_rate` in [0,1], `min_score` threshold for pruning candidates.
pub fn prune_by_score(text: &str, keep_rate: f64, min_score: f64) -> String {
    if text.is_empty() || keep_rate >= 1.0 {
        return text.to_string();
    }

    let tokens = split_preserving_whitespace(text);
    let mut word_tokens: Vec<String> = Vec::new();
    for t in &tokens {
        if !t.chars().all(char::is_whitespace) {
            word_tokens.push(t.clone());
        }
    }
    if word_tokens.is_empty() {
        return text.to_string();
    }

    let target_keep = ((word_tokens.len() as f64) * keep_rate).ceil() as usize;
    let target_keep = target_keep.min(word_tokens.len());

    let mut scored: Vec<(usize, f64)> = word_tokens
        .iter()
        .enumerate()
        .map(|(i, t)| (i, score_token(t)))
        .collect();
    // Sort by score ascending; stable by index for determinism.
    scored.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let mut to_prune = std::collections::HashSet::new();
    let max_prune = word_tokens.len().saturating_sub(target_keep);
    let mut pruned = 0usize;
    for (i, score) in scored {
        if pruned >= max_prune {
            break;
        }
        if score < min_score {
            to_prune.insert(i);
            pruned += 1;
        }
    }

    let mut word_idx = 0usize;
    let mut rebuilt = String::with_capacity(text.len());
    for t in &tokens {
        if t.chars().all(char::is_whitespace) {
            rebuilt.push_str(t);
        } else {
            if !to_prune.contains(&word_idx) {
                rebuilt.push_str(t);
            }
            word_idx += 1;
        }
    }

    collapse_multi_spaces_and_trim(&rebuilt)
}

fn collapse_multi_spaces_and_trim(s: &str) -> String {
    let re = Regex::new(r"\s{2,}").expect("valid whitespace collapse regex");
    re.replace_all(s, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_tokens() {
        assert_eq!(score_token("123"), 1.0);
        assert_eq!(score_token("https://x"), 1.0);
        assert_eq!(score_token("path/to"), 1.0);
        assert_eq!(score_token("Error:"), 1.0);
        assert_eq!(score_token("```"), 1.0);
        assert_eq!(score_token("the"), 0.1);
        assert_eq!(score_token("to"), 0.1); // stopword takes precedence
        assert_eq!(score_token("go"), 0.2); // short, non-stopword
        assert_eq!(score_token("Rust"), 0.8);
        assert_eq!(score_token("compression"), 0.7);
        assert_eq!(score_token("keep"), 0.5);
    }

    #[test]
    fn prunes_by_score_deterministically() {
        let text = "This is a very simple test for compression";
        let out = prune_by_score(text, 1.0, 0.3);
        assert_eq!(out, text);

        let out = prune_by_score(text, 0.5, 0.3);
        assert!(!out.is_empty());
        // Low-value stopwords/short tokens should be pruned preferentially.
        assert!(!out.split_whitespace().any(|w| w == "a"));
        assert!(out.split_whitespace().count() <= text.split_whitespace().count());

        let empty = prune_by_score("", 0.5, 0.3);
        assert_eq!(empty, "");
    }

    #[test]
    fn preserves_force_preserve_tokens() {
        let text = "see Error: code 42 at path/to/file";
        let out = prune_by_score(text, 0.4, 0.3);
        assert!(out.contains("Error:"));
        assert!(out.contains("42"));
        assert!(out.contains("path/to/file"));
    }
}
