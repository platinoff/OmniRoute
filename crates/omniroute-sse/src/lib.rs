//! SSE/stream protocol primitives ported from OmniRoute `open-sse/utils/streamHelpers.ts`.
//!
//! CNV-1 W8 (bounded, additive): pure parsing/formatting helpers.
//! Not wired into default runtime; intended for opt-in/adapter use.

use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

fn ansi_escape_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"\x1b(?:\[[0-9;?]*[A-Za-z]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[A-Z\[\]\\^_`])|[\x00-\x08\x0b\x0c\x0e-\x1f]",
        )
        .expect("valid ansi escape regex")
    })
}

/// Strip ANSI/VT100 escape sequences and stray C0 controls, preserving \t, \n, \r.
pub fn strip_ansi_codes(s: &str) -> String {
    ansi_escape_re().replace_all(s, "").into_owned()
}

/// Parse an SSE `data:` payload string.
///
/// - Trims whitespace.
/// - Empty -> `None`.
/// - `[DONE]` -> `Some({"done": true})`.
/// - JSON parse failure -> `None`.
/// - If `event_type` is provided and parsed value is an object without a string `type`,
///   injects `type = event_type`.
pub fn parse_sse_data_payload(data: &str, event_type: Option<&str>) -> Option<Value> {
    let payload = data.trim();
    if payload.is_empty() {
        return None;
    }
    if payload == "[DONE]" {
        return Some(json!({ "done": true }));
    }
    match serde_json::from_str::<Value>(payload) {
        Ok(parsed) => {
            if let Some(et) = event_type {
                if let Value::Object(mut map) = parsed {
                    let has_string_type = matches!(map.get("type"), Some(Value::String(_)));
                    if !has_string_type {
                        map.insert("type".to_string(), Value::String(et.to_string()));
                    }
                    Some(Value::Object(map))
                } else {
                    Some(parsed)
                }
            } else {
                Some(parsed)
            }
        }
        Err(_) => None,
    }
}

/// Parse SSE `data:` lines joined by newline (same payload semantics as a single payload).
pub fn parse_sse_data_lines(lines: &[String], event_type: Option<&str>) -> Option<Value> {
    parse_sse_data_payload(&lines.join("\n"), event_type)
}

/// Extract the payload portion of an SSE data line after trimming/ANSI stripping.
/// Returns `None` if the line is not a `data:` line.
pub fn extract_sse_data_line(line: &str) -> Option<String> {
    let normalized = line.replace('\r', "");
    let trimmed = normalized.trim_start();
    let clean = strip_ansi_codes(trimmed);
    if !clean.starts_with("data:") {
        return None;
    }
    Some(clean[5..].trim_start().to_string())
}

/// Parse a single SSE line. Returns `None` if not a `data:` line or payload is unparseable/empty.
pub fn parse_sse_line(line: &str) -> Option<Value> {
    if line.is_empty() {
        return None;
    }
    let trimmed = line.trim_start();
    let clean = strip_ansi_codes(trimmed);
    if !clean.starts_with("data:") {
        return None;
    }
    parse_sse_data_payload(&clean[5..], None)
}

fn clean_perf_metrics(value: &Value) -> Value {
    let mut out = value.clone();
    if let Value::Object(ref mut obj) = out {
        if let Some(Value::Object(usage)) = obj.get_mut("usage") {
            if matches!(usage.get("perf_metrics"), Some(Value::Null)) {
                usage.remove("perf_metrics");
            }
        }
    }
    out
}

/// Format a value as an SSE frame string.
///
/// - `None` or `Value::Null` -> empty string.
/// - `{"done": true}` -> `data: [DONE]\n\n`.
/// - `{"event": "...", "data": <value>}` -> `event: ...\ndata: <json>\n\n`.
/// - Claude format (`source_format == Some("claude")`) with string `type` -> `event: <type>\ndata: <json>\n\n`.
/// - Otherwise -> `data: <json>\n\n`.
pub fn format_sse(data: Option<&Value>, source_format: Option<&str>) -> String {
    let Some(value) = data else {
        return String::new();
    };
    if value.is_null() {
        return String::new();
    }
    if let Value::Object(map) = value {
        if map.get("done") == Some(&Value::Bool(true)) && map.len() == 1 {
            return "data: [DONE]\n\n".to_string();
        }
        // OpenAI Responses API/event wrapper form
        if let (Some(event), Some(data_field)) = (map.get("event"), map.get("data")) {
            if !event.is_null() && !data_field.is_null() {
                if let Some(event_str) = event.as_str() {
                    return format!("event: {}\ndata: {}\n\n", event_str, data_field);
                }
                return format!("event: {}\ndata: {}\n\n", event, data_field);
            }
        }
        let cleaned = clean_perf_metrics(value);
        if source_format == Some("claude") {
            if let Value::Object(cleaned_map) = &cleaned {
                if cleaned_map.get("type").and_then(|v| v.as_str()).is_some() {
                    if let Some(t) = cleaned_map.get("type").and_then(|v| v.as_str()) {
                        return format!("event: {}\ndata: {}\n\n", t, cleaned);
                    }
                }
            }
        }
        return format!("data: {}\n\n", cleaned);
    }

    // Non-object values: serialize as data JSON
    let cleaned = clean_perf_metrics(value);
    format!("data: {}\n\n", cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_ansi_and_c0_preserving_whitespace() {
        let input = "\x1b[2K\x1b[1Adata: hi\x00\x07\t\n\r";
        let out = strip_ansi_codes(input);
        assert_eq!(out, "data: hi\t\n\r");
    }

    #[test]
    fn parses_sse_data_payload_cases() {
        assert_eq!(parse_sse_data_payload("", None), None);
        assert_eq!(parse_sse_data_payload("   ", None), None);
        assert_eq!(
            parse_sse_data_payload("[DONE]", None),
            Some(json!({"done": true}))
        );
        assert_eq!(
            parse_sse_data_payload(" [DONE] ", None),
            Some(json!({"done": true}))
        );

        let obj = parse_sse_data_payload(r#"{"a":1}"#, None).unwrap();
        assert_eq!(obj, json!({"a":1}));

        let injected = parse_sse_data_payload(r#"{"a":1}"#, Some("message_start")).unwrap();
        assert_eq!(injected, json!({"a":1,"type":"message_start"}));

        let no_override = parse_sse_data_payload(r#"{"type":"x"}"#, Some("y")).unwrap();
        assert_eq!(no_override, json!({"type":"x"}));

        assert_eq!(parse_sse_data_payload("{bad", None), None);
    }

    #[test]
    fn parses_sse_line_with_ansi_prefix() {
        let line = "\x1b[2K\x1b[1Adata: {\"ok\":true}";
        assert_eq!(parse_sse_line(line), Some(json!({"ok":true})));
        assert_eq!(parse_sse_line("event: ping"), None);
        assert_eq!(parse_sse_line(""), None);
        assert_eq!(parse_sse_line("data: [DONE]"), Some(json!({"done": true})));
    }

    #[test]
    fn extracts_data_line() {
        assert_eq!(
            extract_sse_data_line("  data:  {\"x\":1}\r"),
            Some("{\"x\":1}".to_string())
        );
        assert_eq!(extract_sse_data_line("event: x"), None);
    }

    #[test]
    fn formats_sse_frames() {
        assert_eq!(format_sse(None, None), "");
        assert_eq!(format_sse(Some(&Value::Null), None), "");
        assert_eq!(
            format_sse(Some(&json!({"done": true})), None),
            "data: [DONE]\n\n"
        );

        let event_form = json!({"event":"response.delta","data":{"x":1}});
        assert_eq!(
            format_sse(Some(&event_form), None),
            "event: response.delta\ndata: {\"x\":1}\n\n"
        );

        let claude = json!({"type":"content_block_delta","delta":{"text":"hi"}});
        assert_eq!(
            format_sse(Some(&claude), Some("claude")),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\n"
        );

        let openai = json!({"id":"1","choices":[]});
        assert_eq!(
            format_sse(Some(&openai), None),
            "data: {\"id\":\"1\",\"choices\":[]}\n\n"
        );
    }

    #[test]
    fn format_sse_cleans_null_perf_metrics() {
        let v = json!({"usage":{"prompt_tokens":1,"perf_metrics":null}});
        let out = format_sse(Some(&v), None);
        assert_eq!(out, "data: {\"usage\":{\"prompt_tokens\":1}}\n\n");
    }
}
