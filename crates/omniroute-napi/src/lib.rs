//! N-API bindings for OmniRoute Rust crates (CNV-1 wave 5+).
//! Additive, opt-in native bindings. Does not affect existing npm/Node runtime flows.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use omniroute_compression::{
    apply_caveman_output_mode, back_off_to_word_boundary, build_caveman_output_instruction,
    collapse_newline_runs, detect_compression_language, extract_text_content,
    is_compression_excluded, is_text_block, list_supported_compression_languages,
    normalize_message_whitespace, prune_by_score, replace_text_content, score_token,
    should_bypass_caveman_output_mode, trim_trailing_horizontal_whitespace, truncate_tool_content,
    validate_compression, validate_rule_pack, CavemanOutputIntensity, CavemanOutputModeConfig,
    CompressionExclusions,
};
use omniroute_gcf::{decode_generic, encode_generic, GenericOptions};
use omniroute_sse::{
    append_bounded_text, build_synthetic_chat_chunk, extract_sse_data_line, fix_invalid_id,
    format_sse, has_active_delta_value, has_openai_finish_reason,
    is_known_non_claude_stream_payload, is_openai_choices_payload, normalize_sse_data_lines,
    parse_sse_data_lines, parse_sse_data_payload, parse_sse_line, strip_ansi_codes,
    unwrap_gemini_chunk, SseDataLineNormalizer, SseEventPrefixBuffer,
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

/// Append bounded streamed text (keeps tail up to 64 KiB).
#[napi(js_name = "sseAppendBoundedText")]
pub fn sse_append_bounded_text_js(current: String, next: String) -> String {
    append_bounded_text(&current, &next)
}

/// Recursively check whether a delta-like value contains meaningful content.
#[napi(js_name = "sseHasActiveDeltaValue")]
pub fn sse_has_active_delta_value_js(value_json: String) -> Result<bool> {
    let value: Value = serde_json::from_str(&value_json)
        .map_err(|e| err_msg("sseHasActiveDeltaValue parse", e))?;
    Ok(has_active_delta_value(&value))
}

/// Fix invalid OpenAI-style chunk id deterministically.
/// Returns JSON string `{ "value": <object>, "changed": boolean }`.
#[napi(js_name = "sseFixInvalidId")]
pub fn sse_fix_invalid_id_js(parsed_json: String, fallback_id: Option<String>) -> Result<String> {
    let value: Value =
        serde_json::from_str(&parsed_json).map_err(|e| err_msg("sseFixInvalidId parse", e))?;
    let (updated, changed) = fix_invalid_id(&value, fallback_id.as_deref());
    let out = serde_json::json!({ "value": updated, "changed": changed });
    serde_json::to_string(&out).map_err(|e| err_msg("sseFixInvalidId stringify", e))
}

/// Build a deterministic synthetic OpenAI-shaped chat completion chunk.
/// `delta_json` and optional `finish_reason_json` are JSON values.
#[napi(js_name = "sseBuildSyntheticChatChunk")]
pub fn sse_build_synthetic_chat_chunk_js(
    id: String,
    model: String,
    created: f64,
    delta_json: String,
    finish_reason_json: Option<String>,
) -> Result<String> {
    let delta: Value = serde_json::from_str(&delta_json)
        .map_err(|e| err_msg("sseBuildSyntheticChatChunk delta parse", e))?;
    let finish_reason: Option<Value> = match finish_reason_json {
        None => None,
        Some(s) => Some(
            serde_json::from_str(&s)
                .map_err(|e| err_msg("sseBuildSyntheticChatChunk finishReason parse", e))?,
        ),
    };
    let chunk =
        build_synthetic_chat_chunk(&id, &model, created as i64, &delta, finish_reason.as_ref());
    serde_json::to_string(&chunk).map_err(|e| err_msg("sseBuildSyntheticChatChunk stringify", e))
}

/// Stateful SSE data-line normalizer (opt-in wrapper).
#[napi]
pub struct SseDataLineNormalizerState {
    inner: SseDataLineNormalizer,
}

#[napi]
impl SseDataLineNormalizerState {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[napi]
    pub fn has_pending(&self) -> bool {
        self.inner.has_pending()
    }

    #[napi]
    pub fn normalize(&mut self, lines: Vec<String>) -> Vec<String> {
        self.inner.normalize(&lines)
    }
}

impl Default for SseDataLineNormalizerState {
    fn default() -> Self {
        Self {
            inner: SseDataLineNormalizer::new(),
        }
    }
}

// ---------- Compression core helpers (W17+, opt-in) ----------

/// True for text-like content blocks (`type` absent, `"text"`, or `"input_text"`).
/// `value_json` is a JSON string representing the block/value.
#[napi(js_name = "compressionIsTextBlock")]
pub fn compression_is_text_block(value_json: String) -> Result<bool> {
    let value: Value = serde_json::from_str(&value_json)
        .map_err(|e| err_msg("compressionIsTextBlock parse", e))?;
    Ok(is_text_block(&value))
}

/// Extract concatenated text content from message content (string/array/null JSON).
#[napi(js_name = "compressionExtractTextContent")]
pub fn compression_extract_text_content(content_json: Option<String>) -> Result<String> {
    let value: Option<Value> = match content_json {
        None => None,
        Some(s) => Some(
            serde_json::from_str(&s)
                .map_err(|e| err_msg("compressionExtractTextContent parse", e))?,
        ),
    };
    Ok(extract_text_content(value.as_ref()))
}

/// Replace text content in a message JSON while preserving non-text blocks.
/// Returns updated message JSON string.
#[napi(js_name = "compressionReplaceTextContent")]
pub fn compression_replace_text_content(msg_json: String, new_text: String) -> Result<String> {
    let msg: Value = serde_json::from_str(&msg_json)
        .map_err(|e| err_msg("compressionReplaceTextContent parse", e))?;
    let out = replace_text_content(&msg, &new_text);
    serde_json::to_string(&out).map_err(|e| err_msg("compressionReplaceTextContent stringify", e))
}

/// Detect compression language (deterministic heuristics). Returns language code.
#[napi(js_name = "compressionDetectLanguage")]
pub fn compression_detect_language(text: String) -> String {
    detect_compression_language(&text)
}

/// List supported compression language codes.
#[napi(js_name = "compressionListSupportedLanguages")]
pub fn compression_list_supported_languages() -> Vec<String> {
    list_supported_compression_languages()
}

/// Score a token for information value (0.0 prune-candidate .. 1.0 keep).
#[napi(js_name = "compressionScoreToken")]
pub fn compression_score_token(token: String) -> f64 {
    score_token(&token)
}

/// Prune tokens to achieve target keep rate. `keep_rate` in [0,1], `min_score` threshold.
#[napi(js_name = "compressionPruneByScore")]
pub fn compression_prune_by_score(text: String, keep_rate: f64, min_score: f64) -> String {
    prune_by_score(&text, keep_rate, min_score)
}

// ---------- Compression output-mode + lite text primitives (W19+, opt-in) ----------

fn parse_output_intensity(intensity: Option<String>) -> CavemanOutputIntensity {
    match intensity.as_deref() {
        Some("full") => CavemanOutputIntensity::Full,
        Some("ultra") => CavemanOutputIntensity::Ultra,
        _ => CavemanOutputIntensity::Lite,
    }
}

fn output_mode_config(
    enabled: bool,
    intensity: Option<String>,
    auto_clarity: Option<bool>,
) -> CavemanOutputModeConfig {
    CavemanOutputModeConfig {
        enabled,
        intensity: parse_output_intensity(intensity),
        auto_clarity: auto_clarity.unwrap_or(true),
    }
}

/// Build caveman output instruction (marker + localized/intensity text).
#[napi(js_name = "compressionBuildCavemanOutputInstruction")]
pub fn compression_build_caveman_output_instruction(
    enabled: bool,
    intensity: Option<String>,
    auto_clarity: Option<bool>,
    language: Option<String>,
) -> String {
    let cfg = output_mode_config(enabled, intensity, auto_clarity);
    build_caveman_output_instruction(&cfg, language.as_deref().unwrap_or("en"))
}

/// Return bypass reason for caveman output mode based on last up to 3 messages, or null.
/// `messages_json` must be a JSON array of message objects.
#[napi(js_name = "compressionShouldBypassCavemanOutputMode")]
pub fn compression_should_bypass_caveman_output_mode(
    messages_json: String,
) -> Result<Option<String>> {
    let messages: Vec<Value> = serde_json::from_str(&messages_json)
        .map_err(|e| err_msg("compressionShouldBypassCavemanOutputMode parse", e))?;
    Ok(should_bypass_caveman_output_mode(&messages))
}

/// Apply caveman output mode injection to a request body JSON.
/// Returns JSON string `{ "body": <object>, "applied": boolean, "skippedReason": string|null }`.
#[napi(js_name = "compressionApplyCavemanOutputMode")]
pub fn compression_apply_caveman_output_mode(
    body_json: String,
    enabled: bool,
    intensity: Option<String>,
    auto_clarity: Option<bool>,
    language: Option<String>,
) -> Result<String> {
    let body: Value = serde_json::from_str(&body_json)
        .map_err(|e| err_msg("compressionApplyCavemanOutputMode parse", e))?;
    let cfg = output_mode_config(enabled, intensity, auto_clarity);
    let res = apply_caveman_output_mode(&body, Some(cfg), language.as_deref().unwrap_or("en"));
    let out = serde_json::json!({
        "body": res.body,
        "applied": res.applied,
        "skippedReason": res.skipped_reason,
    });
    serde_json::to_string(&out)
        .map_err(|e| err_msg("compressionApplyCavemanOutputMode stringify", e))
}

/// Trim trailing horizontal whitespace (space/tab) from a line.
#[napi(js_name = "compressionTrimTrailingHorizontalWhitespace")]
pub fn compression_trim_trailing_horizontal_whitespace(line: String) -> String {
    trim_trailing_horizontal_whitespace(&line).to_string()
}

/// Collapse runs of newlines to at most two consecutive newlines.
#[napi(js_name = "compressionCollapseNewlineRuns")]
pub fn compression_collapse_newline_runs(content: String) -> String {
    collapse_newline_runs(&content)
}

/// Normalize message whitespace (collapse newline runs + trim trailing horizontal whitespace per line).
#[napi(js_name = "compressionNormalizeMessageWhitespace")]
pub fn compression_normalize_message_whitespace(content: String) -> String {
    normalize_message_whitespace(&content)
}

/// Adjust a cut index to a nearby word boundary (byte offset, slice-end semantics).
#[napi(js_name = "compressionBackOffToWordBoundary")]
pub fn compression_back_off_to_word_boundary(content: String, cut_byte: u32) -> u32 {
    back_off_to_word_boundary(&content, cut_byte as usize) as u32
}

/// Truncate tool content to `max_len` with word-boundary backoff and truncation marker.
#[napi(js_name = "compressionTruncateToolContent")]
pub fn compression_truncate_tool_content(content: String, max_len: u32) -> String {
    truncate_tool_content(&content, max_len as usize)
}

/// Stateful SSE event-prefix buffer (opt-in wrapper).
#[napi]
pub struct SseEventPrefixBufferState {
    inner: SseEventPrefixBuffer,
}

#[napi]
impl SseEventPrefixBufferState {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    #[napi]
    pub fn event_type(&self) -> String {
        self.inner.event_type()
    }

    #[napi]
    pub fn flush(&mut self) -> String {
        self.inner.flush()
    }

    #[napi]
    pub fn prefix_data(&mut self, output: String, line: String) -> String {
        self.inner.prefix_data(&output, &line)
    }

    #[napi]
    pub fn remember(&mut self, line: String) {
        self.inner.remember(&line);
    }
}

impl Default for SseEventPrefixBufferState {
    fn default() -> Self {
        Self {
            inner: SseEventPrefixBuffer::new(),
        }
    }
}
