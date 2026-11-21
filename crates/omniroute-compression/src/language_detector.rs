//! Compression language detection ported from `open-sse/services/compression/languageDetector.ts`.
//! Deterministic keyword/Unicode heuristics.

use regex::Regex;
use std::sync::OnceLock;

struct LangHint {
    lang: &'static str,
    patterns: &'static [&'static str],
}

fn hints() -> &'static [LangHint] {
    // Ordered to preserve tie-breaking behavior (first max wins by strict >).
    static H: OnceLock<Vec<LangHint>> = OnceLock::new();
    H.get_or_init(|| {
        vec![
            LangHint {
                lang: "pt-BR",
                patterns: &[
                    r"(?i)\b(?:voce|você|preciso|arquivo|codigo|código|erro|falha|obrigado)\b",
                ],
            },
            LangHint {
                lang: "es",
                patterns: &[r"(?i)\b(?:necesito|archivo|codigo|código|fallo|gracias|puedes)\b"],
            },
            LangHint {
                lang: "de",
                patterns: &[r"(?i)\b(?:ich|datei|fehler|bitte|kannst|konfiguration|danke)\b"],
            },
            LangHint {
                lang: "fr",
                patterns: &[r"(?i)\b(?:fichier|erreur|merci|peux|besoin)\b"],
            },
            LangHint {
                lang: "ja",
                patterns: &[r"[\u3040-\u30ff]"],
            },
            LangHint {
                lang: "id",
                patterns: &[
                    r"(?i)\b(?:saya|kamu|anda|dengan|untuk|yang|tidak|bisa|terima\s+kasih|dari)\b",
                ],
            },
        ]
    })
}

fn compiled_hints() -> &'static Vec<(String, Vec<Regex>)> {
    static C: OnceLock<Vec<(String, Vec<Regex>)>> = OnceLock::new();
    C.get_or_init(|| {
        hints()
            .iter()
            .map(|h| {
                (
                    h.lang.to_string(),
                    h.patterns
                        .iter()
                        .map(|p| Regex::new(p).expect("valid language hint regex"))
                        .collect(),
                )
            })
            .collect()
    })
}

fn has_han(text: &str) -> bool {
    text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

fn has_kana(text: &str) -> bool {
    text.chars().any(|c| ('\u{3040}'..='\u{30FF}').contains(&c))
}

/// Detect compression language using deterministic heuristics.
/// CJK: Han without kana => `zh`; kana => Japanese hint path.
/// Keyword hints scored by total match count; highest wins, ties keep earlier; zero => `en`.
pub fn detect_compression_language(text: &str) -> String {
    if has_han(text) && !has_kana(text) {
        return "zh".to_string();
    }

    let mut best = "en".to_string();
    let mut best_score: usize = 0;
    for (lang, patterns) in compiled_hints() {
        let mut score: usize = 0;
        for re in patterns {
            score += re.find_iter(text).count();
        }
        if score > best_score {
            best_score = score;
            best = lang.clone();
        }
    }
    best
}

/// Supported compression language codes.
pub fn list_supported_compression_languages() -> Vec<String> {
    let mut out = vec!["en".to_string(), "zh".to_string()];
    for h in hints() {
        out.push(h.lang.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cjk_variants() {
        assert_eq!(detect_compression_language("这是中文内容"), "zh");
        assert_eq!(detect_compression_language("これは日本語です"), "ja");
    }

    #[test]
    fn detects_keyword_languages() {
        assert_eq!(
            detect_compression_language("voce pode corrigir o arquivo codigo erro"),
            "pt-BR"
        );
        assert_eq!(
            detect_compression_language("necesito archivo codigo fallo gracias"),
            "es"
        );
        assert_eq!(
            detect_compression_language("ich brauche datei fehler bitte danke"),
            "de"
        );
        assert_eq!(
            detect_compression_language("merci fichier erreur besoin peux"),
            "fr"
        );
        assert_eq!(
            detect_compression_language("saya tidak bisa dengan file ini terima kasih"),
            "id"
        );
    }

    #[test]
    fn defaults_to_english() {
        assert_eq!(
            detect_compression_language("please fix this error now"),
            "en"
        );
        assert_eq!(detect_compression_language(""), "en");
    }

    #[test]
    fn lists_supported_languages() {
        assert_eq!(
            list_supported_compression_languages(),
            vec!["en", "zh", "pt-BR", "es", "de", "fr", "ja", "id"]
        );
    }
}
