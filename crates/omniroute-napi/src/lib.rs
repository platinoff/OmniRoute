//! N-API bindings for OmniRoute Rust crates (CNV-1 wave 5+).
//! Additive, opt-in native bindings. Does not affect existing npm/Node runtime flows.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use omniroute_compression::{
    is_compression_excluded, validate_compression, validate_rule_pack, CompressionExclusions,
};
use omniroute_gcf::{decode_generic, encode_generic, GenericOptions};
use omniroute_sse::{
    extract_sse_data_line, format_sse, has_openai_finish_reason,
    is_known_non_claude_stream_payload, is_openai_choices_payload, normalize_sse_data_lines,
    parse_sse_data_lines, parse_sse_data_payload, parse_sse_line, strip_ansi_codes,
    unwrap_gemini_chunk,
};
use serde_json::Value;

fn err_msg(prefix: &str, msg: impl std::fmt::Display) -> napi::Error {
    napi::Error::from_reason(format!("{prefix}: {msg}"))
}

/// Encode a JSON value (stringified) to GCF generic profile text.
#[napi(js_name = "gcfEncodeJson")]
pub fn gcf_encode_json(json_input: String, no_flatten: Option<bool>) -> Result<String> {
    let value: Value =
        serde_json::from_str(&json_input).map_err(|e| err_msg("gcfEncodeJson parse", e))?;
    let opts = GenericOptions {
        no_flatten: no_flatten.unwrap_or(false),
    };
    Ok(encode_generic(&value, Some(&opts)))
}

/// Decode GCF generic profile text to a JSON string.
#[napi(js_name = "gcfDecodeToJson")]
pub fn gcf_decode_to_json(gcf_text: String) -> Result<String> {
    let value = decode_generic(&gcf_text).map_err(|e| err_msg("gcfDecodeToJson", e))?;
    serde_json::to_string(&value).map_err(|e| err_msg("gcfDecodeToJson stringify", e))
}

/// Validate a compression rule pack JSON value (structural + best-effort pattern checks).
/// Returns a JSON string: `{ "valid": boolean, "errors": string[] }`.
#[napi(js_name = "validateRulePackJson")]
pub fn validate_rule_pack_json(json: String) -> Result<String> {
    let value: Value =
        serde_json::from_str(&json).map_err(|e| err_msg("validateRulePackJson parse", e))?;
    let res = validate_rule_pack(&value);
    let out = serde_json::json!({
        "valid": res.valid,
        "errors": res.errors,
    });
    serde_json::to_string(&out).map_err(|e| err_msg("validateRulePackJson stringify", e))
}

/// Validate compression fidelity/invariants.
/// Returns a JSON string: `{ "valid", "errors", "warnings", "fallbackApplied" }`.
#[napi(js_name = "validateCompressionJson")]
pub fn validate_compression_json(original: String, compressed: String) -> Result<String> {
    let res = validate_compression(&original, &compressed);
    let out = serde_json::json!({
        "valid": res.valid,
        "errors": res.errors,
        "warnings": res.warnings,
        "fallbackApplied": res.fallback_applied,
    });
    serde_json::to_string(&out).map_err(|e| err_msg("validateCompressionJson stringify", e))
}

/// Check if a provider/model target is excluded from compression.
/// `exclusions_json` must be a JSON array of strings (e.g. `["openai/*","gpt-4"]`).
#[napi(js_name = "isCompressionExcluded")]
pub fn is_compression_excluded_js(
    provider: Option<String>,
    model: Option<String>,
    exclusions_json: String,
) -> Result<bool> {
    let raw: Vec<Value> = serde_json::from_str(&exclusions_json)
        .map_err(|e| err_msg("isCompressionExcluded parse", e))?;
    let mut exclusions: CompressionExclusions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for v in raw {
        if let Value::String(s) = v {
            let trimmed = s.trim().to_lowercase();
            if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
                exclusions.push(trimmed);
            }
        }
    }
    Ok(is_compression_excluded(
        provider.as_deref(),
        model.as_deref(),
        &exclusions,
    ))
}

/// Strip ANSI/VT100 escapes and stray C0 controls (preserves \t, \n, \r).
#[napi(js_name = "sseStripAnsiCodes")]
pub fn sse_strip_ansi_codes_js(s: String) -> String {
    strip_ansi_codes(&s)
}

/// Parse SSE data payload. Returns JSON string of parsed value, or null if unparseable/empty.
#[napi(js_name = "sseParseDataPayload")]
pub fn sse_parse_data_payload_js(
    data: String,
    event_type: Option<String>,
) -> Result<Option<String>> {
    let parsed = parse_sse_data_payload(&data, event_type.as_deref());
    match parsed {
        Some(v) => serde_json::to_string(&v)
            .map(Some)
            .map_err(|e| err_msg("sseParseDataPayload stringify", e)),
        None => Ok(None),
    }
}

/// Parse joined SSE data lines. `lines_json` must be a JSON array of strings.
#[napi(js_name = "sseParseDataLines")]
pub fn sse_parse_data_lines_js(
    lines_json: String,
    event_type: Option<String>,
) -> Result<Option<String>> {
    let lines: Vec<String> =
        serde_json::from_str(&lines_json).map_err(|e| err_msg("sseParseDataLines parse", e))?;
    let parsed = parse_sse_data_lines(&lines, event_type.as_deref());
    match parsed {
        Some(v) => serde_json::to_string(&v)
            .map(Some)
            .map_err(|e| err_msg("sseParseDataLines stringify", e)),
        None => Ok(None),
    }
}

/// Extract payload text from an SSE `data:` line, or null if not a data line.
#[napi(js_name = "sseExtractDataLine")]
pub fn sse_extract_data_line_js(line: String) -> Option<String> {
    extract_sse_data_line(&line)
}

/// Parse a single SSE line. Returns JSON string of parsed value, or null.
#[napi(js_name = "sseParseLine")]
pub fn sse_parse_line_js(line: String) -> Result<Option<String>> {
    let parsed = parse_sse_line(&line);
    match parsed {
        Some(v) => serde_json::to_string(&v)
            .map(Some)
            .map_err(|e| err_msg("sseParseLine stringify", e)),
        None => Ok(None),
    }
}

/// Format an SSE frame. `data_json` is a JSON string value (or null/undefined for empty frame).
#[napi(js_name = "sseFormatSse")]
pub fn sse_format_sse_js(
    data_json: Option<String>,
    source_format: Option<String>,
) -> Result<String> {
    let value: Option<Value> = match data_json {
        None => None,
        Some(s) => Some(serde_json::from_str(&s).map_err(|e| err_msg("sseFormatSse parse", e))?),
    };
    Ok(format_sse(value.as_ref(), source_format.as_deref()))
}

/// True when payload JSON carries an OpenAI `choices` array.
#[napi(js_name = "sseIsOpenaiChoicesPayload")]
pub fn sse_is_openai_choices_payload_js(parsed_json: String) -> Result<bool> {
    let value: Value = serde_json::from_str(&parsed_json)
        .map_err(|e| err_msg("sseIsOpenaiChoicesPayload parse", e))?;
    Ok(is_openai_choices_payload(&value))
}

/// True when any OpenAI choice carries a non-null `finish_reason`.
#[napi(js_name = "sseHasOpenaiFinishReason")]
pub fn sse_has_openai_finish_reason_js(parsed_json: String) -> Result<bool> {
    let value: Value = serde_json::from_str(&parsed_json)
        .map_err(|e| err_msg("sseHasOpenaiFinishReason parse", e))?;
    Ok(has_openai_finish_reason(&value))
}

/// True when payload is a known non-Claude stream payload shape with relevant stream value.
#[napi(js_name = "sseIsKnownNonClaudeStreamPayload")]
pub fn sse_is_known_non_claude_stream_payload_js(
    parsed_json: String,
    event_type: Option<String>,
) -> Result<bool> {
    let value: Value = serde_json::from_str(&parsed_json)
        .map_err(|e| err_msg("sseIsKnownNonClaudeStreamPayload parse", e))?;
    Ok(is_known_non_claude_stream_payload(
        &value,
        event_type.as_deref().unwrap_or(""),
    ))
}

/// Unwrap Cloud Code API Gemini envelope when top-level `candidates` is absent.
/// Returns JSON string of the unwrapped (or original) value.
#[napi(js_name = "sseUnwrapGeminiChunk")]
pub fn sse_unwrap_gemini_chunk_js(parsed_json: String) -> Result<String> {
    let value: Value =
        serde_json::from_str(&parsed_json).map_err(|e| err_msg("sseUnwrapGeminiChunk parse", e))?;
    let unwrapped = unwrap_gemini_chunk(&value);
    serde_json::to_string(&unwrapped).map_err(|e| err_msg("sseUnwrapGeminiChunk stringify", e))
}

/// Normalize SSE lines in a single batch (fresh normalizer). `lines_json` is a JSON array of strings.
/// Returns JSON array of normalized output lines.
#[napi(js_name = "sseNormalizeLinesBatch")]
pub fn sse_normalize_lines_batch_js(lines_json: String) -> Result<String> {
    let lines: Vec<String> = serde_json::from_str(&lines_json)
        .map_err(|e| err_msg("sseNormalizeLinesBatch parse", e))?;
    let out = normalize_sse_data_lines(&lines);
    serde_json::to_string(&out).map_err(|e| err_msg("sseNormalizeLinesBatch stringify", e))
}
