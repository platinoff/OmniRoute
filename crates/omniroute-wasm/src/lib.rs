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
