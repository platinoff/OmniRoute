//! Caveman output-mode primitives ported from `open-sse/services/compression/outputMode.ts`.
//! Portable, deterministic helpers for instruction building and request-body injection.

use crate::message_content::extract_text_content;
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

pub const CAVEMAN_OUTPUT_MARKER: &str = "[OmniRoute Caveman Output Mode]";

/// Shared boundary clause — appended to every intensity level.
pub const SHARED_BOUNDARIES: &str = "Code blocks, file paths, commands, errors, URLs: keep exact. Security warnings, irreversible action confirmations, multi-step ordered sequences: write normal. Resume terse style after. Active every response until user asks for normal mode.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CavemanOutputIntensity {
    Lite,
    Full,
    Ultra,
}

impl CavemanOutputIntensity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lite => "lite",
            Self::Full => "full",
            Self::Ultra => "ultra",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CavemanOutputModeConfig {
    pub enabled: bool,
    pub intensity: CavemanOutputIntensity,
    pub auto_clarity: bool,
}

impl Default for CavemanOutputModeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: CavemanOutputIntensity::Lite,
            auto_clarity: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CavemanOutputModeResult {
    pub body: Value,
    pub applied: bool,
    pub skipped_reason: Option<String>,
}

fn en_lite() -> String {
    format!(
        "Respond concise. Drop filler, pleasantries, hedging. Keep full sentences, technical terms, code, errors, URLs, and identifiers exact. {SHARED_BOUNDARIES}"
    )
}
fn en_full() -> String {
    format!(
        "Respond terse like smart caveman. Drop articles (a/an/the), filler (just/really/basically/actually/simply), pleasantries, hedging. Fragments OK. Short synonyms (big not extensive, fix not implement). Keep all technical substance, code, errors, URLs, identifiers exact. {SHARED_BOUNDARIES}"
    )
}
fn en_ultra() -> String {
    format!(
        "Respond ultra terse. Maximum compression. Telegraphic. Abbreviate (DB/auth/config/req/res/fn/impl), strip conjunctions, arrows for causality (X → Y). One word when one word enough. Never abbreviate code symbols, API names, error strings, URLs, or identifiers. {SHARED_BOUNDARIES}"
    )
}
fn pt_lite() -> String {
    format!(
        "Responda conciso. Remova enrolacao, cortesias e incerteza. Preserve termos tecnicos, codigo, erros, URLs e identificadores exatamente. {SHARED_BOUNDARIES}"
    )
}
fn pt_full() -> String {
    format!(
        "Responda seco e compacto. Frases curtas OK. Preserve todo conteudo tecnico, codigo, erros, URLs e identificadores exatamente. {SHARED_BOUNDARIES}"
    )
}
fn pt_ultra() -> String {
    format!(
        "Responda ultra compacto. Use prosa tecnica curta e abreviacoes comuns como DB/auth/config/req/res/fn. Nunca abrevie simbolos de codigo, APIs, erros, URLs ou identificadores. {SHARED_BOUNDARIES}"
    )
}
fn es_lite() -> String {
    format!(
        "Responde conciso. Quita relleno, cortesias y dudas. Conserva terminos tecnicos, codigo, errores, URLs e identificadores exactos. {SHARED_BOUNDARIES}"
    )
}
fn es_full() -> String {
    format!(
        "Responde seco y compacto. Fragmentos OK. Conserva todo el contenido tecnico, codigo, errores, URLs e identificadores exactos. {SHARED_BOUNDARIES}"
    )
}
fn es_ultra() -> String {
    format!(
        "Responde ultra compacto. Usa prosa tecnica corta y abreviaturas comunes como DB/auth/config/req/res/fn. Nunca abrevies simbolos de codigo, APIs, errores, URLs o identificadores. {SHARED_BOUNDARIES}"
    )
}
fn de_lite() -> String {
    format!(
        "Antworte knapp. Entferne Fuellwoerter, Hoeflichkeit und Unsicherheit. Bewahre Fachbegriffe, Code, Fehler, URLs und Bezeichner exakt. {SHARED_BOUNDARIES}"
    )
}
fn de_full() -> String {
    format!(
        "Antworte sehr knapp. Fragmente OK. Bewahre alle technischen Inhalte, Code, Fehler, URLs und Bezeichner exakt. {SHARED_BOUNDARIES}"
    )
}
fn de_ultra() -> String {
    format!(
        "Antworte ultra knapp. Nutze kurze technische Prosa und uebliche Abkuerzungen wie DB/auth/config/req/res/fn. Code-Symbole, APIs, Fehler, URLs und Bezeichner nie abkuerzen. {SHARED_BOUNDARIES}"
    )
}
fn fr_lite() -> String {
    format!(
        "Reponds concis. Retire remplissage, politesses et hesitations. Garde termes techniques, code, erreurs, URLs et identifiants exacts. {SHARED_BOUNDARIES}"
    )
}
fn fr_full() -> String {
    format!(
        "Reponds tres compact. Fragments OK. Garde tout le contenu technique, code, erreurs, URLs et identifiants exacts. {SHARED_BOUNDARIES}"
    )
}
fn fr_ultra() -> String {
    format!(
        "Reponds ultra compact. Utilise une prose technique courte et des abreviations communes comme DB/auth/config/req/res/fn. N'abrege jamais symboles de code, APIs, erreurs, URLs ou identifiants. {SHARED_BOUNDARIES}"
    )
}
fn ja_lite() -> String {
    format!(
        "簡潔に回答。冗長表現、挨拶、曖昧表現を削る。技術用語、コード、エラー、URL、識別子は正確に保持。{SHARED_BOUNDARIES}"
    )
}
fn ja_full() -> String {
    format!(
        "短く圧縮して回答。断片文可。技術内容、コード、エラー、URL、識別子は正確に保持。{SHARED_BOUNDARIES}"
    )
}
fn ja_ultra() -> String {
    format!(
        "超短く回答。DB/auth/config/req/res/fn など一般的な略語は可。コード記号、API名、エラー文字列、URL、識別子は省略しない。{SHARED_BOUNDARIES}"
    )
}
fn id_lite() -> String {
    format!(
        "Jawab ringkas. Hapus pengisi, salam sopan santun, keraguan. Pertahankan istilah teknis, kode, error, URL, & identifier secara persis. {SHARED_BOUNDARIES}"
    )
}
fn id_full() -> String {
    format!(
        "Jawab sangat singkat ala caveman pintar. Hapus kata pengisi (hanya/sangat/sebenarnya), salam sopan santun. Kalimat pendek/tidak lengkap OK. Gunakan sinonim pendek. Pertahankan semua substansi teknis, kode, error, URL, & identifier secara persis. {SHARED_BOUNDARIES}"
    )
}
fn id_ultra() -> String {
    format!(
        "Jawab ultra singkat. Kompresi maksimal. Gunakan singkatan umum (DB/auth/config/req/res/fn/impl), hilangkan kata hubung, gunakan panah untuk kausalitas (X → Y). Satu kata jika cukup. Jangan singkat simbol kode, nama API, string error, URL, atau identifier. {SHARED_BOUNDARIES}"
    )
}

fn instruction_for(language: &str, intensity: CavemanOutputIntensity) -> String {
    match language {
        "pt-BR" => match intensity {
            CavemanOutputIntensity::Lite => pt_lite(),
            CavemanOutputIntensity::Full => pt_full(),
            CavemanOutputIntensity::Ultra => pt_ultra(),
        },
        "es" => match intensity {
            CavemanOutputIntensity::Lite => es_lite(),
            CavemanOutputIntensity::Full => es_full(),
            CavemanOutputIntensity::Ultra => es_ultra(),
        },
        "de" => match intensity {
            CavemanOutputIntensity::Lite => de_lite(),
            CavemanOutputIntensity::Full => de_full(),
            CavemanOutputIntensity::Ultra => de_ultra(),
        },
        "fr" => match intensity {
            CavemanOutputIntensity::Lite => fr_lite(),
            CavemanOutputIntensity::Full => fr_full(),
            CavemanOutputIntensity::Ultra => fr_ultra(),
        },
        "ja" => match intensity {
            CavemanOutputIntensity::Lite => ja_lite(),
            CavemanOutputIntensity::Full => ja_full(),
            CavemanOutputIntensity::Ultra => ja_ultra(),
        },
        "id" => match intensity {
            CavemanOutputIntensity::Lite => id_lite(),
            CavemanOutputIntensity::Full => id_full(),
            CavemanOutputIntensity::Ultra => id_ultra(),
        },
        _ => match intensity {
            CavemanOutputIntensity::Lite => en_lite(),
            CavemanOutputIntensity::Full => en_full(),
            CavemanOutputIntensity::Ultra => en_ultra(),
        },
    }
}

/// Build caveman output instruction string including marker and intensity/language text.
pub fn build_caveman_output_instruction(
    config: &CavemanOutputModeConfig,
    language: &str,
) -> String {
    format!(
        "{CAVEMAN_OUTPUT_MARKER}\n{}",
        instruction_for(language, config.intensity)
    )
}

fn re_security() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(security|vulnerability|exploit|credential leak|secret leak|malware|phishing)\b")
            .expect("valid security bypass regex")
    })
}
fn re_irreversible() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(delete|drop table|truncate|destroy|wipe|irreversible|permanently remove)\b",
        )
        .expect("valid irreversible bypass regex")
    })
}
fn re_clarify() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(clarify|explain in detail|more detail|step by step|why exactly|what do you mean)\b",
        )
        .expect("valid clarify bypass regex")
    })
}
fn re_order_sensitive() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?is)\b(first|then|after that|before|rollback|backup)\b[\s\S]{0,240}\b(delete|drop|migrate|deploy|release)\b",
        )
        .expect("valid order-sensitive bypass regex")
    })
}

/// Return bypass reason for caveman output mode based on last up to 3 messages, or None.
pub fn should_bypass_caveman_output_mode(messages: &[Value]) -> Option<String> {
    let start = messages.len().saturating_sub(3);
    let mut parts: Vec<String> = Vec::new();
    for msg in &messages[start..] {
        if let Some(obj) = msg.as_object() {
            let content = obj.get("content");
            parts.push(extract_text_content(content).to_lowercase());
        }
    }
    let text = parts.join("\n");
    if text.trim().is_empty() {
        return None;
    }
    if re_security().is_match(&text) {
        return Some("security_warning".to_string());
    }
    if re_irreversible().is_match(&text) {
        return Some("irreversible_action".to_string());
    }
    if re_clarify().is_match(&text) {
        return Some("clarification_requested".to_string());
    }
    if re_order_sensitive().is_match(&text) {
        return Some("order_sensitive_sequence".to_string());
    }
    None
}

fn result(body: Value, applied: bool, skipped_reason: Option<&str>) -> CavemanOutputModeResult {
    CavemanOutputModeResult {
        body,
        applied,
        skipped_reason: skipped_reason.map(|s| s.to_string()),
    }
}

/// Apply caveman output mode injection to a chat/request body (JSON).
/// Mirrors `applyCavemanOutputMode` semantics: messages path, instructions/input fallback,
/// idempotency before bypass, optional auto-clarity bypass.
pub fn apply_caveman_output_mode(
    body: &Value,
    options: Option<CavemanOutputModeConfig>,
    language: &str,
) -> CavemanOutputModeResult {
    let config = options.unwrap_or_default();
    if !config.enabled {
        return result(body.clone(), false, Some("disabled"));
    }

    let Some(body_obj) = body.as_object() else {
        return result(body.clone(), false, Some("no_messages"));
    };

    let messages_opt = body_obj.get("messages").and_then(|v| v.as_array()).cloned();
    let instruction = build_caveman_output_instruction(&config, language);

    let Some(messages) = messages_opt else {
        // instructions / input fallback (e.g. Responses API style)
        if let Some(Value::String(instr)) = body_obj.get("instructions") {
            if instr.contains(CAVEMAN_OUTPUT_MARKER) {
                return result(body.clone(), false, Some("already_applied"));
            }
            let mut next = body_obj.clone();
            next.insert(
                "instructions".to_string(),
                Value::String(format!("{}\n\n{instruction}", instr.trim())),
            );
            return result(Value::Object(next), true, None);
        }
        let has_input = body_obj
            .get("input")
            .map(|v| v.is_string() || v.is_array())
            .unwrap_or(false);
        if has_input {
            let mut next = body_obj.clone();
            next.insert("instructions".to_string(), Value::String(instruction));
            return result(Value::Object(next), true, None);
        }
        return result(body.clone(), false, Some("no_messages"));
    };

    if messages.is_empty() {
        if let Some(Value::String(instr)) = body_obj.get("instructions") {
            if instr.contains(CAVEMAN_OUTPUT_MARKER) {
                return result(body.clone(), false, Some("already_applied"));
            }
            let mut next = body_obj.clone();
            next.insert(
                "instructions".to_string(),
                Value::String(format!("{}\n\n{instruction}", instr.trim())),
            );
            return result(Value::Object(next), true, None);
        }
        let has_input = body_obj
            .get("input")
            .map(|v| v.is_string() || v.is_array())
            .unwrap_or(false);
        if has_input {
            let mut next = body_obj.clone();
            next.insert("instructions".to_string(), Value::String(instruction));
            return result(Value::Object(next), true, None);
        }
        return result(body.clone(), false, Some("no_messages"));
    }

    // Idempotency before bypass
    let already_applied = messages.iter().any(|m| {
        m.as_object()
            .map(|o| {
                o.get("role").and_then(|r| r.as_str()) == Some("system")
                    && o.get("content")
                        .and_then(|c| c.as_str())
                        .map(|s| s.contains(CAVEMAN_OUTPUT_MARKER))
                        .unwrap_or(false)
            })
            .unwrap_or(false)
    });
    if already_applied {
        return result(body.clone(), false, Some("already_applied"));
    }

    if config.auto_clarity {
        if let Some(bypass) = should_bypass_caveman_output_mode(&messages) {
            return result(body.clone(), false, Some(&bypass));
        }
    }

    let mut next_messages = messages.clone();
    let first_is_system_string = next_messages
        .first()
        .and_then(|m| m.as_object())
        .map(|o| {
            o.get("role").and_then(|r| r.as_str()) == Some("system")
                && o.get("content").map(|c| c.is_string()).unwrap_or(false)
        })
        .unwrap_or(false);

    if first_is_system_string {
        if let Some(first_obj) = next_messages[0].as_object_mut() {
            let existing = first_obj
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            first_obj.insert(
                "content".to_string(),
                Value::String(format!("{existing}\n\n{instruction}")),
            );
        }
    } else {
        let mut sys = Map::new();
        sys.insert("role".to_string(), Value::String("system".to_string()));
        sys.insert("content".to_string(), Value::String(instruction));
        next_messages.insert(0, Value::Object(sys));
    }

    let mut next = body_obj.clone();
    next.insert("messages".to_string(), Value::Array(next_messages));
    result(Value::Object(next), true, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_instruction_with_marker() {
        let cfg = CavemanOutputModeConfig {
            enabled: true,
            intensity: CavemanOutputIntensity::Full,
            auto_clarity: true,
        };
        let s = build_caveman_output_instruction(&cfg, "en");
        assert!(s.starts_with(CAVEMAN_OUTPUT_MARKER));
        assert!(s.contains("smart caveman"));
        assert!(s.contains(SHARED_BOUNDARIES));

        let ja = build_caveman_output_instruction(&cfg, "ja");
        assert!(ja.contains("短く圧縮"));
    }

    #[test]
    fn bypass_reasons_detected() {
        let msgs = vec![json!({"role":"user","content":"please delete the database"})];
        assert_eq!(
            should_bypass_caveman_output_mode(&msgs).as_deref(),
            Some("irreversible_action")
        );
        let msgs = vec![json!({"role":"user","content":"explain in detail why"})];
        assert_eq!(
            should_bypass_caveman_output_mode(&msgs).as_deref(),
            Some("clarification_requested")
        );
        let msgs = vec![json!({"role":"user","content":"hello"})];
        assert!(should_bypass_caveman_output_mode(&msgs).is_none());
    }

    #[test]
    fn applies_to_messages_and_idempotent() {
        let body = json!({
            "messages": [
                {"role":"system","content":"sys"},
                {"role":"user","content":"hi"}
            ]
        });
        let cfg = CavemanOutputModeConfig {
            enabled: true,
            intensity: CavemanOutputIntensity::Lite,
            auto_clarity: false,
        };
        let r1 = apply_caveman_output_mode(&body, Some(cfg), "en");
        assert!(r1.applied);
        let c = r1.body["messages"][0]["content"].as_str().unwrap();
        assert!(c.contains("sys"));
        assert!(c.contains(CAVEMAN_OUTPUT_MARKER));

        let r2 = apply_caveman_output_mode(&r1.body, Some(cfg), "en");
        assert!(!r2.applied);
        assert_eq!(r2.skipped_reason.as_deref(), Some("already_applied"));
    }

    #[test]
    fn prepends_system_when_missing_and_handles_no_messages() {
        let body = json!({"messages": [{"role":"user","content":"hi"}]});
        let cfg = CavemanOutputModeConfig {
            enabled: true,
            intensity: CavemanOutputIntensity::Ultra,
            auto_clarity: false,
        };
        let r = apply_caveman_output_mode(&body, Some(cfg), "en");
        assert!(r.applied);
        assert_eq!(r.body["messages"][0]["role"], "system");

        let disabled = CavemanOutputModeConfig::default();
        let r = apply_caveman_output_mode(&body, Some(disabled), "en");
        assert_eq!(r.skipped_reason.as_deref(), Some("disabled"));

        let empty = json!({"model":"x"});
        let r = apply_caveman_output_mode(&empty, Some(cfg), "en");
        assert_eq!(r.skipped_reason.as_deref(), Some("no_messages"));
    }

    #[test]
    fn instructions_fallback_for_responses_style() {
        let body = json!({"instructions":"base", "input":"hello"});
        let cfg = CavemanOutputModeConfig {
            enabled: true,
            intensity: CavemanOutputIntensity::Lite,
            auto_clarity: false,
        };
        let r = apply_caveman_output_mode(&body, Some(cfg), "en");
        assert!(r.applied);
        assert!(r.body["instructions"]
            .as_str()
            .unwrap()
            .contains(CAVEMAN_OUTPUT_MARKER));
    }
}
