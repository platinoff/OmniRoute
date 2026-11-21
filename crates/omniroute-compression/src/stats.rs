//! Lightweight compression stats/token-estimation helpers.
//!
//! Ported from OmniRoute TS compression utility patterns (e.g.
//! `open-sse/services/compression/toolResultCompressor.ts` uses
//! `Math.ceil(text.length / 4)`). Bounded, dependency-light, no runtime wiring.

/// Estimate token count from UTF-16/JS string length semantics approximated by
/// Rust char count in bytes? TS uses `text.length` (UTF-16 code units).
/// For parity with JS behavior on ASCII/common text, use UTF-16 code unit count.
///
/// Returns 0 for empty text.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    let units = text.encode_utf16().count();
    units.div_ceil(4)
}

/// Estimated token savings: `max(0, estimate_tokens(original) - estimate_tokens(compressed))`.
#[must_use]
pub fn estimate_token_savings(original: &str, compressed: &str) -> usize {
    estimate_tokens(original).saturating_sub(estimate_tokens(compressed))
}

/// Compression ratio as `compressed/original` in [0.0, +inf) over UTF-16 units.
/// Returns 1.0 when original is empty.
#[must_use]
pub fn compression_ratio(original: &str, compressed: &str) -> f64 {
    let o = text_utf16_len(original);
    if o == 0 {
        return 1.0;
    }
    text_utf16_len(compressed) as f64 / o as f64
}

/// Byte/char savings summary for compression reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressionSavings {
    pub original_chars: usize,
    pub compressed_chars: usize,
    pub original_tokens_est: usize,
    pub compressed_tokens_est: usize,
    pub saved_tokens_est: usize,
}

/// Compute lightweight savings stats. Uses UTF-16 code-unit counts for parity
/// with JS `string.length`-based heuristics.
#[must_use]
pub fn compression_savings(original: &str, compressed: &str) -> CompressionSavings {
    let original_chars = text_utf16_len(original);
    let compressed_chars = text_utf16_len(compressed);
    let original_tokens_est = original_chars.div_ceil(4);
    let compressed_tokens_est = compressed_chars.div_ceil(4);
    CompressionSavings {
        original_chars,
        compressed_chars,
        original_tokens_est,
        compressed_tokens_est,
        saved_tokens_est: original_tokens_est.saturating_sub(compressed_tokens_est),
    }
}

fn text_utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_tokens_ascii_ceil_div_4() {
        assert_eq!(estimate_tokens("a"), 1);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[test]
    fn estimate_tokens_uses_utf16_units() {
        // emoji outside BMP = 2 UTF-16 code units in JS; Rust char is one scalar.
        let s = "🙂";
        assert_eq!(s.encode_utf16().count(), 2);
        assert_eq!(estimate_tokens(s), 1);
        let s2 = "🙂🙂🙂"; // 6 UTF-16 units
        assert_eq!(estimate_tokens(s2), 2);
    }

    #[test]
    fn savings_non_negative() {
        assert_eq!(estimate_token_savings("short", "much longer text"), 0);
        assert_eq!(estimate_token_savings("abcdefgh", "abcd"), 1);
    }

    #[test]
    fn compression_ratio_empty_original() {
        assert_eq!(compression_ratio("", ""), 1.0);
        assert_eq!(compression_ratio("", "abc"), 1.0);
    }

    #[test]
    fn compression_ratio_normal() {
        let r = compression_ratio("abcdefgh", "abcd");
        assert!((r - 0.5).abs() < 1e-9);
    }

    #[test]
    fn compression_savings_struct() {
        let s = compression_savings("abcdefgh", "abcd");
        assert_eq!(
            s,
            CompressionSavings {
                original_chars: 8,
                compressed_chars: 4,
                original_tokens_est: 2,
                compressed_tokens_est: 1,
                saved_tokens_est: 1,
            }
        );
    }
}
