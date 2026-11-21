//! N-API bindings for OmniRoute Rust crates (CNV-1 wave 5).
//! Additive, opt-in native bindings. Does not affect existing npm/Node runtime flows.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use omniroute_compression::{
    is_compression_excluded, validate_compression, validate_rule_pack, CompressionExclusions,
};
use omniroute_gcf::{decode_generic, encode_generic, GenericOptions};
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
