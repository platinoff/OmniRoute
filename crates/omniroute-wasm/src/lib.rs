//! Opt-in WebAssembly bindings for OmniRoute Rust crates (CNV-1).
//! Additive, not wired into default runtime flows.
//!
//! Build (example):
//! `cargo build -p omniroute-wasm --target wasm32-unknown-unknown --release`
//! `wasm-bindgen --target nodejs --out-dir crates/omniroute-wasm/pkg target/wasm32-unknown-unknown/release/omniroute_wasm.wasm`

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
fn js_err(prefix: &str, msg: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&format!("{prefix}: {msg}"))
}

// ---------- GCF ----------

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = gcfEncodeJson)]
pub fn gcf_encode_json(json_input: String, no_flatten: Option<bool>) -> Result<String, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(&json_input).map_err(|e| js_err("gcfEncodeJson parse", e))?;
    let opts = omniroute_gcf::GenericOptions {
        no_flatten: no_flatten.unwrap_or(false),
    };
    Ok(omniroute_gcf::encode_generic(&value, Some(&opts)))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = gcfDecodeToJson)]
pub fn gcf_decode_to_json(gcf_text: String) -> Result<String, JsValue> {
    let value =
        omniroute_gcf::decode_generic(&gcf_text).map_err(|e| js_err("gcfDecodeToJson", e))?;
    serde_json::to_string(&value).map_err(|e| js_err("gcfDecodeToJson stringify", e))
}

// ---------- SSE ----------

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseStripAnsiCodes)]
pub fn sse_strip_ansi_codes(s: String) -> String {
    omniroute_sse::strip_ansi_codes(&s)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseParseDataPayload)]
pub fn sse_parse_data_payload(
    data: String,
    event_type: Option<String>,
) -> Result<Option<String>, JsValue> {
    let parsed = omniroute_sse::parse_sse_data_payload(&data, event_type.as_deref());
    match parsed {
        Some(v) => serde_json::to_string(&v)
            .map(Some)
            .map_err(|e| js_err("sseParseDataPayload stringify", e)),
        None => Ok(None),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseParseDataLines)]
pub fn sse_parse_data_lines(
    lines_json: String,
    event_type: Option<String>,
) -> Result<Option<String>, JsValue> {
    let lines: Vec<String> =
        serde_json::from_str(&lines_json).map_err(|e| js_err("sseParseDataLines parse", e))?;
    let parsed = omniroute_sse::parse_sse_data_lines(&lines, event_type.as_deref());
    match parsed {
        Some(v) => serde_json::to_string(&v)
            .map(Some)
            .map_err(|e| js_err("sseParseDataLines stringify", e)),
        None => Ok(None),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseExtractDataLine)]
pub fn sse_extract_data_line(line: String) -> Option<String> {
    omniroute_sse::extract_sse_data_line(&line)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseParseLine)]
pub fn sse_parse_line(line: String) -> Result<Option<String>, JsValue> {
    let parsed = omniroute_sse::parse_sse_line(&line);
    match parsed {
        Some(v) => serde_json::to_string(&v)
            .map(Some)
            .map_err(|e| js_err("sseParseLine stringify", e)),
        None => Ok(None),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseFormatSse)]
pub fn sse_format_sse(
    data_json: Option<String>,
    source_format: Option<String>,
) -> Result<String, JsValue> {
    let value: Option<serde_json::Value> = match data_json {
        None => None,
        Some(s) => Some(serde_json::from_str(&s).map_err(|e| js_err("sseFormatSse parse", e))?),
    };
    Ok(omniroute_sse::format_sse(
        value.as_ref(),
        source_format.as_deref(),
    ))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseIsOpenaiChoicesPayload)]
pub fn sse_is_openai_choices_payload(parsed_json: String) -> Result<bool, JsValue> {
    let value: serde_json::Value = serde_json::from_str(&parsed_json)
        .map_err(|e| js_err("sseIsOpenaiChoicesPayload parse", e))?;
    Ok(omniroute_sse::is_openai_choices_payload(&value))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseHasOpenaiFinishReason)]
pub fn sse_has_openai_finish_reason(parsed_json: String) -> Result<bool, JsValue> {
    let value: serde_json::Value = serde_json::from_str(&parsed_json)
        .map_err(|e| js_err("sseHasOpenaiFinishReason parse", e))?;
    Ok(omniroute_sse::has_openai_finish_reason(&value))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseIsKnownNonClaudeStreamPayload)]
pub fn sse_is_known_non_claude_stream_payload(
    parsed_json: String,
    event_type: Option<String>,
) -> Result<bool, JsValue> {
    let value: serde_json::Value = serde_json::from_str(&parsed_json)
        .map_err(|e| js_err("sseIsKnownNonClaudeStreamPayload parse", e))?;
    Ok(omniroute_sse::is_known_non_claude_stream_payload(
        &value,
        event_type.as_deref().unwrap_or(""),
    ))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseUnwrapGeminiChunk)]
pub fn sse_unwrap_gemini_chunk(parsed_json: String) -> Result<String, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(&parsed_json).map_err(|e| js_err("sseUnwrapGeminiChunk parse", e))?;
    let unwrapped = omniroute_sse::unwrap_gemini_chunk(&value);
    serde_json::to_string(&unwrapped).map_err(|e| js_err("sseUnwrapGeminiChunk stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseNormalizeLinesBatch)]
pub fn sse_normalize_lines_batch(lines_json: String) -> Result<String, JsValue> {
    let lines: Vec<String> =
        serde_json::from_str(&lines_json).map_err(|e| js_err("sseNormalizeLinesBatch parse", e))?;
    let out = omniroute_sse::normalize_sse_data_lines(&lines);
    serde_json::to_string(&out).map_err(|e| js_err("sseNormalizeLinesBatch stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseAppendBoundedText)]
pub fn sse_append_bounded_text(current: String, next: String) -> String {
    omniroute_sse::append_bounded_text(&current, &next)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseHasActiveDeltaValue)]
pub fn sse_has_active_delta_value(value_json: String) -> Result<bool, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(&value_json).map_err(|e| js_err("sseHasActiveDeltaValue parse", e))?;
    Ok(omniroute_sse::has_active_delta_value(&value))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseFixInvalidId)]
pub fn sse_fix_invalid_id(
    parsed_json: String,
    fallback_id: Option<String>,
) -> Result<String, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(&parsed_json).map_err(|e| js_err("sseFixInvalidId parse", e))?;
    let (updated, changed) = omniroute_sse::fix_invalid_id(&value, fallback_id.as_deref());
    let out = serde_json::json!({ "value": updated, "changed": changed });
    serde_json::to_string(&out).map_err(|e| js_err("sseFixInvalidId stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = sseBuildSyntheticChatChunk)]
pub fn sse_build_synthetic_chat_chunk(
    id: String,
    model: String,
    created: f64,
    delta_json: String,
    finish_reason_json: Option<String>,
) -> Result<String, JsValue> {
    let delta: serde_json::Value = serde_json::from_str(&delta_json)
        .map_err(|e| js_err("sseBuildSyntheticChatChunk delta parse", e))?;
    let finish_reason: Option<serde_json::Value> = match finish_reason_json {
        None => None,
        Some(s) => Some(
            serde_json::from_str(&s)
                .map_err(|e| js_err("sseBuildSyntheticChatChunk finishReason parse", e))?,
        ),
    };
    let chunk = omniroute_sse::build_synthetic_chat_chunk(
        &id,
        &model,
        created as i64,
        &delta,
        finish_reason.as_ref(),
    );
    serde_json::to_string(&chunk).map_err(|e| js_err("sseBuildSyntheticChatChunk stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = SseDataLineNormalizerState)]
pub struct SseDataLineNormalizerStateWasm {
    inner: omniroute_sse::SseDataLineNormalizer,
}

#[cfg(target_arch = "wasm32")]
impl Default for SseDataLineNormalizerStateWasm {
    fn default() -> Self {
        Self {
            inner: omniroute_sse::SseDataLineNormalizer::new(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_class = SseDataLineNormalizerState)]
impl SseDataLineNormalizerStateWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen(js_name = hasPending)]
    pub fn has_pending(&self) -> bool {
        self.inner.has_pending()
    }

    #[wasm_bindgen(js_name = normalize)]
    pub fn normalize(&mut self, lines: Vec<String>) -> Vec<String> {
        self.inner.normalize(&lines)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = SseEventPrefixBufferState)]
pub struct SseEventPrefixBufferStateWasm {
    inner: omniroute_sse::SseEventPrefixBuffer,
}

#[cfg(target_arch = "wasm32")]
impl Default for SseEventPrefixBufferStateWasm {
    fn default() -> Self {
        Self {
            inner: omniroute_sse::SseEventPrefixBuffer::new(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_class = SseEventPrefixBufferState)]
impl SseEventPrefixBufferStateWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    #[wasm_bindgen(js_name = eventType)]
    pub fn event_type(&self) -> String {
        self.inner.event_type()
    }

    pub fn flush(&mut self) -> String {
        self.inner.flush()
    }

    #[wasm_bindgen(js_name = prefixData)]
    pub fn prefix_data(&mut self, output: String, line: String) -> String {
        self.inner.prefix_data(&output, &line)
    }

    pub fn remember(&mut self, line: String) {
        self.inner.remember(&line);
    }
}

// ---------- Compression ----------

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = validateCompressionJson)]
pub fn validate_compression_json(original: String, compressed: String) -> Result<String, JsValue> {
    let res = omniroute_compression::validate_compression(&original, &compressed);
    let out = serde_json::json!({
        "valid": res.valid,
        "errors": res.errors,
        "warnings": res.warnings,
        "fallbackApplied": res.fallback_applied,
    });
    serde_json::to_string(&out).map_err(|e| js_err("validateCompressionJson stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = validateRulePackJson)]
pub fn validate_rule_pack_json(json: String) -> Result<String, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| js_err("validateRulePackJson parse", e))?;
    let res = omniroute_compression::validate_rule_pack(&value);
    let out = serde_json::json!({
        "valid": res.valid,
        "errors": res.errors,
    });
    serde_json::to_string(&out).map_err(|e| js_err("validateRulePackJson stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = isCompressionExcluded)]
pub fn is_compression_excluded_js(
    provider: Option<String>,
    model: Option<String>,
    exclusions_json: String,
) -> Result<bool, JsValue> {
    let raw: Vec<serde_json::Value> = serde_json::from_str(&exclusions_json)
        .map_err(|e| js_err("isCompressionExcluded parse", e))?;
    let mut exclusions: omniroute_compression::CompressionExclusions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for v in raw {
        if let serde_json::Value::String(s) = v {
            let trimmed = s.trim().to_lowercase();
            if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
                exclusions.push(trimmed);
            }
        }
    }
    Ok(omniroute_compression::is_compression_excluded(
        provider.as_deref(),
        model.as_deref(),
        &exclusions,
    ))
}

// ---------- Compression core helpers (W17+, opt-in) ----------

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionIsTextBlock)]
pub fn compression_is_text_block(value_json: String) -> Result<bool, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(&value_json).map_err(|e| js_err("compressionIsTextBlock parse", e))?;
    Ok(omniroute_compression::is_text_block(&value))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionExtractTextContent)]
pub fn compression_extract_text_content(content_json: Option<String>) -> Result<String, JsValue> {
    let value: Option<serde_json::Value> = match content_json {
        None => None,
        Some(s) => Some(
            serde_json::from_str(&s)
                .map_err(|e| js_err("compressionExtractTextContent parse", e))?,
        ),
    };
    Ok(omniroute_compression::extract_text_content(value.as_ref()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionReplaceTextContent)]
pub fn compression_replace_text_content(
    msg_json: String,
    new_text: String,
) -> Result<String, JsValue> {
    let msg: serde_json::Value = serde_json::from_str(&msg_json)
        .map_err(|e| js_err("compressionReplaceTextContent parse", e))?;
    let out = omniroute_compression::replace_text_content(&msg, &new_text);
    serde_json::to_string(&out).map_err(|e| js_err("compressionReplaceTextContent stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionDetectLanguage)]
pub fn compression_detect_language(text: String) -> String {
    omniroute_compression::detect_compression_language(&text)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionListSupportedLanguages)]
pub fn compression_list_supported_languages() -> Vec<String> {
    omniroute_compression::list_supported_compression_languages()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionScoreToken)]
pub fn compression_score_token(token: String) -> f64 {
    omniroute_compression::score_token(&token)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionPruneByScore)]
pub fn compression_prune_by_score(text: String, keep_rate: f64, min_score: f64) -> String {
    omniroute_compression::prune_by_score(&text, keep_rate, min_score)
}

// ---------- Compression output-mode + lite text primitives (W19+, opt-in) ----------

#[cfg(target_arch = "wasm32")]
fn parse_output_intensity(
    intensity: Option<String>,
) -> omniroute_compression::CavemanOutputIntensity {
    match intensity.as_deref() {
        Some("full") => omniroute_compression::CavemanOutputIntensity::Full,
        Some("ultra") => omniroute_compression::CavemanOutputIntensity::Ultra,
        _ => omniroute_compression::CavemanOutputIntensity::Lite,
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionBuildCavemanOutputInstruction)]
pub fn compression_build_caveman_output_instruction(
    enabled: bool,
    intensity: Option<String>,
    auto_clarity: Option<bool>,
    language: Option<String>,
) -> String {
    let cfg = omniroute_compression::CavemanOutputModeConfig {
        enabled,
        intensity: parse_output_intensity(intensity),
        auto_clarity: auto_clarity.unwrap_or(true),
    };
    omniroute_compression::build_caveman_output_instruction(
        &cfg,
        language.as_deref().unwrap_or("en"),
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionShouldBypassCavemanOutputMode)]
pub fn compression_should_bypass_caveman_output_mode(
    messages_json: String,
) -> Result<Option<String>, JsValue> {
    let messages: Vec<serde_json::Value> = serde_json::from_str(&messages_json)
        .map_err(|e| js_err("compressionShouldBypassCavemanOutputMode parse", e))?;
    Ok(omniroute_compression::should_bypass_caveman_output_mode(
        &messages,
    ))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionApplyCavemanOutputMode)]
pub fn compression_apply_caveman_output_mode(
    body_json: String,
    enabled: bool,
    intensity: Option<String>,
    auto_clarity: Option<bool>,
    language: Option<String>,
) -> Result<String, JsValue> {
    let body: serde_json::Value = serde_json::from_str(&body_json)
        .map_err(|e| js_err("compressionApplyCavemanOutputMode parse", e))?;
    let cfg = omniroute_compression::CavemanOutputModeConfig {
        enabled,
        intensity: parse_output_intensity(intensity),
        auto_clarity: auto_clarity.unwrap_or(true),
    };
    let res = omniroute_compression::apply_caveman_output_mode(
        &body,
        Some(cfg),
        language.as_deref().unwrap_or("en"),
    );
    let out = serde_json::json!({
        "body": res.body,
        "applied": res.applied,
        "skippedReason": res.skipped_reason,
    });
    serde_json::to_string(&out)
        .map_err(|e| js_err("compressionApplyCavemanOutputMode stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionTrimTrailingHorizontalWhitespace)]
pub fn compression_trim_trailing_horizontal_whitespace(line: String) -> String {
    omniroute_compression::trim_trailing_horizontal_whitespace(&line).to_string()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionCollapseNewlineRuns)]
pub fn compression_collapse_newline_runs(content: String) -> String {
    omniroute_compression::collapse_newline_runs(&content)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionNormalizeMessageWhitespace)]
pub fn compression_normalize_message_whitespace(content: String) -> String {
    omniroute_compression::normalize_message_whitespace(&content)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionBackOffToWordBoundary)]
pub fn compression_back_off_to_word_boundary(content: String, cut_byte: u32) -> u32 {
    omniroute_compression::back_off_to_word_boundary(&content, cut_byte as usize) as u32
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionTruncateToolContent)]
pub fn compression_truncate_tool_content(content: String, max_len: u32) -> String {
    omniroute_compression::truncate_tool_content(&content, max_len as usize)
}

// ---------- Compression lite body transforms (W21+, opt-in) ----------

#[cfg(target_arch = "wasm32")]
fn lite_body_options_wasm(
    preserve_system_prompt: Option<bool>,
    supports_vision: Option<bool>,
) -> omniroute_compression::LiteCompressionOptions {
    omniroute_compression::LiteCompressionOptions {
        preserve_system_prompt: preserve_system_prompt.unwrap_or(false),
        supports_vision,
    }
}

#[cfg(target_arch = "wasm32")]
fn body_transform_json_wasm(
    result: omniroute_compression::BodyTransformResult,
) -> Result<String, JsValue> {
    let out = serde_json::json!({
        "body": result.body,
        "applied": result.applied,
    });
    serde_json::to_string(&out).map_err(|e| js_err("compressionBodyTransform stringify", e))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionCollapseWhitespace)]
pub fn compression_collapse_whitespace(
    body_json: String,
    preserve_system_prompt: Option<bool>,
    supports_vision: Option<bool>,
) -> Result<String, JsValue> {
    let body: serde_json::Value = serde_json::from_str(&body_json)
        .map_err(|e| js_err("compressionCollapseWhitespace parse", e))?;
    let opts = lite_body_options_wasm(preserve_system_prompt, supports_vision);
    body_transform_json_wasm(omniroute_compression::collapse_whitespace(&body, opts))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionDedupSystemPrompt)]
pub fn compression_dedup_system_prompt(
    body_json: String,
    preserve_system_prompt: Option<bool>,
    supports_vision: Option<bool>,
) -> Result<String, JsValue> {
    let body: serde_json::Value = serde_json::from_str(&body_json)
        .map_err(|e| js_err("compressionDedupSystemPrompt parse", e))?;
    let opts = lite_body_options_wasm(preserve_system_prompt, supports_vision);
    body_transform_json_wasm(omniroute_compression::dedup_system_prompt(&body, opts))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionCompressToolResults)]
pub fn compression_compress_tool_results(body_json: String) -> Result<String, JsValue> {
    let body: serde_json::Value = serde_json::from_str(&body_json)
        .map_err(|e| js_err("compressionCompressToolResults parse", e))?;
    body_transform_json_wasm(omniroute_compression::compress_tool_results(&body))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionRemoveRedundantContent)]
pub fn compression_remove_redundant_content(
    body_json: String,
    preserve_system_prompt: Option<bool>,
    supports_vision: Option<bool>,
) -> Result<String, JsValue> {
    let body: serde_json::Value = serde_json::from_str(&body_json)
        .map_err(|e| js_err("compressionRemoveRedundantContent parse", e))?;
    let opts = lite_body_options_wasm(preserve_system_prompt, supports_vision);
    body_transform_json_wasm(omniroute_compression::remove_redundant_content(&body, opts))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = compressionReplaceImageUrls)]
pub fn compression_replace_image_urls(
    body_json: String,
    supports_vision: Option<bool>,
) -> Result<String, JsValue> {
    let body: serde_json::Value = serde_json::from_str(&body_json)
        .map_err(|e| js_err("compressionReplaceImageUrls parse", e))?;
    body_transform_json_wasm(omniroute_compression::replace_image_urls(
        &body,
        supports_vision,
    ))
}
