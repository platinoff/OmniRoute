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

fn is_sse_control_or_comment_line(trimmed: &str) -> bool {
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("event:")
        || lower.starts_with("id:")
        || lower.starts_with("retry:")
        || lower.starts_with(':')
}

fn has_self_describing_pending_data_payload(pending_event_lines: &[String]) -> bool {
    let data_lines: Vec<String> = pending_event_lines
        .iter()
        .filter_map(|line| extract_sse_data_line(line))
        .collect();
    if data_lines.is_empty() {
        return false;
    }
    let Some(parsed) = parse_sse_data_lines(&data_lines, None) else {
        return false;
    };
    let Value::Object(map) = parsed else {
        return false;
    };
    map.get("done") == Some(&Value::Bool(true))
        || matches!(map.get("type"), Some(Value::String(_)))
        || matches!(map.get("object"), Some(Value::String(_)))
        || matches!(map.get("choices"), Some(Value::Array(_)))
        || matches!(map.get("candidates"), Some(Value::Array(_)))
        || matches!(map.get("response"), Some(Value::Object(_)))
}

/// Stateful SSE data-line normalizer (bounded port of `createSSEDataLineNormalizer`).
#[derive(Debug, Default, Clone)]
pub struct SseDataLineNormalizer {
    pending_event_lines: Vec<String>,
}

impl SseDataLineNormalizer {
    pub fn new() -> Self {
        Self {
            pending_event_lines: Vec::new(),
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.pending_event_lines.is_empty()
    }

    fn flush(&mut self, output: &mut Vec<String>) {
        if self.pending_event_lines.is_empty() {
            return;
        }

        let event_lines: Vec<String> = self
            .pending_event_lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .cloned()
            .collect();
        let mut data_lines: Vec<String> = Vec::new();
        let mut passthrough_lines: Vec<String> = Vec::new();

        for line in &event_lines {
            if let Some(data_line) = extract_sse_data_line(line) {
                data_lines.push(data_line);
            } else {
                passthrough_lines.push(line.clone());
            }
        }

        output.extend(passthrough_lines);
        if !data_lines.is_empty() {
            let parsed = parse_sse_data_lines(&data_lines, None);
            if let Some(parsed_value) = parsed {
                if parsed_value.get("done") == Some(&Value::Bool(true)) {
                    output.push("data: [DONE]".to_string());
                } else {
                    output.push(format!("data: {}", parsed_value));
                }
            } else {
                output.extend(
                    event_lines
                        .iter()
                        .filter(|line| extract_sse_data_line(line).is_some())
                        .cloned(),
                );
            }
        } else {
            output.extend(
                event_lines
                    .iter()
                    .filter(|line| extract_sse_data_line(line).is_some())
                    .cloned(),
            );
        }

        output.push(String::new());
        self.pending_event_lines.clear();
    }

    /// Normalize incoming SSE lines, buffering partial events and emitting normalized events.
    pub fn normalize(&mut self, lines: &[String]) -> Vec<String> {
        let mut output: Vec<String> = Vec::new();
        for line in lines {
            let normalized_line = line.replace('\r', "");
            let trimmed = normalized_line.trim().to_string();

            if !trimmed.is_empty()
                && is_sse_control_or_comment_line(&trimmed)
                && has_self_describing_pending_data_payload(&self.pending_event_lines)
            {
                self.flush(&mut output);
            }

            self.pending_event_lines.push(normalized_line);
            if trimmed.is_empty() {
                self.flush(&mut output);
            }
        }
        output
    }
}

/// Normalize SSE data lines in a single batch using a fresh normalizer.
pub fn normalize_sse_data_lines(lines: &[String]) -> Vec<String> {
    let mut normalizer = SseDataLineNormalizer::new();
    normalizer.normalize(lines)
}

/// Stateful SSE event-prefix buffer (bounded port of `createSSEEventPrefixBuffer`).
#[derive(Debug, Default, Clone)]
pub struct SseEventPrefixBuffer {
    lines: Vec<String>,
    emitted: bool,
}

impl SseEventPrefixBuffer {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            emitted: false,
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.emitted = false;
    }

    fn has_unemitted(&self) -> bool {
        !self.lines.is_empty() && !self.emitted
    }

    fn prefix(&mut self, output: &str) -> String {
        if !self.has_unemitted() {
            return output.to_string();
        }
        self.emitted = true;
        format!("{}\n{}", self.lines.join("\n"), output)
    }

    /// Last remembered `event:` type, if any.
    pub fn event_type(&self) -> String {
        for line in self.lines.iter().rev() {
            let trimmed = line.trim();
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("event:") {
                return trimmed[6..].trim().to_string();
            }
        }
        String::new()
    }

    /// Flush buffered prefix as a block terminator when unemitted.
    pub fn flush(&mut self) -> String {
        if self.has_unemitted() {
            self.prefix("\n")
        } else {
            String::new()
        }
    }

    /// Prefix output with buffered event lines when `line` is a `data:` line.
    pub fn prefix_data(&mut self, output: &str, line: &str) -> String {
        if line.starts_with("data:") {
            self.prefix(output)
        } else {
            output.to_string()
        }
    }

    pub fn remember(&mut self, line: &str) {
        self.lines.push(line.to_string());
        self.emitted = false;
    }
}

fn value_is_record(value: &Value) -> bool {
    matches!(value, Value::Object(_))
}

/// True when payload carries an OpenAI `choices` array (shape marker).
pub fn is_openai_choices_payload(parsed: &Value) -> bool {
    matches!(parsed.get("choices"), Some(Value::Array(_)))
}

/// True when any OpenAI choice carries a non-null `finish_reason`.
pub fn has_openai_finish_reason(parsed: &Value) -> bool {
    let Some(Value::Array(choices)) = parsed.get("choices") else {
        return false;
    };
    choices.iter().any(|choice| {
        let Value::Object(c) = choice else {
            return false;
        };
        !matches!(c.get("finish_reason"), None | Some(Value::Null))
    })
}

fn has_openai_compatible_stream_value(parsed: &Value) -> bool {
    let Some(Value::Array(choices)) = parsed.get("choices") else {
        return false;
    };
    choices.iter().any(|choice| {
        let Value::Object(c) = choice else {
            return false;
        };
        let Some(Value::Object(delta)) = c.get("delta") else {
            return false;
        };
        if matches!(delta.get("content"), Some(Value::String(s)) if !s.is_empty()) {
            return true;
        }
        if matches!(delta.get("reasoning_content"), Some(Value::String(s)) if !s.is_empty()) {
            return true;
        }
        if matches!(delta.get("reasoning_text"), Some(Value::String(s)) if !s.is_empty()) {
            return true;
        }
        matches!(delta.get("tool_calls"), Some(Value::Array(arr)) if !arr.is_empty())
    })
}

fn has_responses_stream_value(parsed: &Value, event_type: &str) -> bool {
    let ty = parsed
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or(event_type);
    if !ty.starts_with("response.") {
        return false;
    }

    match ty {
        "response.output_text.delta"
        | "response.reasoning_text.delta"
        | "response.reasoning_summary_text.delta"
        | "response.function_call_arguments.delta" => {
            matches!(parsed.get("delta"), Some(Value::String(s)) if !s.is_empty())
                || matches!(parsed.get("text"), Some(Value::String(s)) if !s.is_empty())
                || matches!(parsed.get("arguments"), Some(Value::String(s)) if !s.is_empty())
        }
        "response.output_item.added" | "response.output_item.done" => {
            matches!(parsed.get("item"), Some(v) if value_is_record(v))
        }
        "response.content_part.added" => {
            matches!(parsed.get("part"), Some(v) if value_is_record(v))
        }
        "response.completed" => {
            if let Some(Value::Object(resp)) = parsed.get("response") {
                matches!(resp.get("output"), Some(Value::Array(arr)) if !arr.is_empty())
            } else {
                false
            }
        }
        _ => false,
    }
}

fn has_gemini_candidate_stream_value(parsed: &Value) -> bool {
    let candidates: &Vec<Value> = if let Some(Value::Array(arr)) = parsed.get("candidates") {
        arr
    } else if let Some(Value::Object(resp)) = parsed.get("response") {
        if let Some(Value::Array(arr)) = resp.get("candidates") {
            arr
        } else {
            return false;
        }
    } else {
        return false;
    };

    candidates.iter().any(|candidate| {
        let Value::Object(c) = candidate else {
            return false;
        };
        let parts = if let Some(Value::Object(content)) = c.get("content") {
            if let Some(Value::Array(parts)) = content.get("parts") {
                parts
            } else {
                return false;
            }
        } else {
            return false;
        };
        parts.iter().any(|part| {
            let Value::Object(p) = part else {
                return false;
            };
            if matches!(p.get("text"), Some(Value::String(s)) if !s.is_empty()) {
                return true;
            }
            value_is_record(p.get("functionCall").unwrap_or(&Value::Null))
                || value_is_record(p.get("executableCode").unwrap_or(&Value::Null))
        })
    })
}

/// True when payload is a known non-Claude stream payload shape with relevant stream value.
pub fn is_known_non_claude_stream_payload(parsed: &Value, event_type: &str) -> bool {
    if parsed.get("choices").is_some() {
        if is_openai_choices_payload(parsed) {
            return has_openai_compatible_stream_value(parsed);
        }
        return false;
    }

    if let Some(Value::String(object_type)) = parsed.get("object") {
        if object_type == "chat.completion.chunk"
            || object_type == "text_completion"
            || object_type.ends_with(".completion.chunk")
        {
            return has_openai_compatible_stream_value(parsed);
        }
    }

    let ty = parsed
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or(event_type);
    if ty.starts_with("response.") {
        return has_responses_stream_value(parsed, event_type);
    }
    if matches!(parsed.get("candidates"), Some(Value::Array(_))) {
        return has_gemini_candidate_stream_value(parsed);
    }

    if let Some(Value::Object(resp)) = parsed.get("response") {
        if matches!(resp.get("candidates"), Some(Value::Array(_))) {
            return has_gemini_candidate_stream_value(parsed);
        }
    }
    false
}

/// Unwrap Cloud Code API Gemini envelope `{ response: { candidates: ... } }`
/// when top-level `candidates` is absent.
pub fn unwrap_gemini_chunk(parsed: &Value) -> Value {
    if parsed.get("candidates").is_none() {
        if let Some(Value::Object(resp)) = parsed.get("response") {
            return Value::Object(resp.clone());
        }
    }
    parsed.clone()
}

/// Maximum retained length for bounded streamed text accumulation.
pub const STREAM_SUMMARY_TEXT_LIMIT: usize = 64 * 1024;

/// Append `next` to `current`, keeping only the tail once the limit is reached.
pub fn append_bounded_text(current: &str, next: &str) -> String {
    if next.is_empty() {
        return current.to_string();
    }
    if current.len() >= STREAM_SUMMARY_TEXT_LIMIT {
        let keep = STREAM_SUMMARY_TEXT_LIMIT.saturating_sub(next.len());
        if keep == 0 {
            let start = next.len().saturating_sub(STREAM_SUMMARY_TEXT_LIMIT);
            return next[start..].to_string();
        }
        let cur_start = current.len().saturating_sub(keep);
        let mut out = String::with_capacity(keep + next.len().min(STREAM_SUMMARY_TEXT_LIMIT));
        out.push_str(&current[cur_start..]);
        out.push_str(next);
        return out;
    }
    let mut combined = String::with_capacity(current.len() + next.len());
    combined.push_str(current);
    combined.push_str(next);
    if combined.len() <= STREAM_SUMMARY_TEXT_LIMIT {
        return combined;
    }
    let start = combined.len().saturating_sub(STREAM_SUMMARY_TEXT_LIMIT);
    combined[start..].to_string()
}

/// Recursively check whether a delta-like value contains meaningful content.
pub fn has_active_delta_value(value: &Value) -> bool {
    match value {
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => arr.iter().any(has_active_delta_value),
        Value::Object(map) => map.values().any(has_active_delta_value),
        Value::Null => false,
        _ => true,
    }
}

/// Fix invalid OpenAI-style chunk id deterministically.
/// Replaces ids that are `"chat"`, `"completion"`, or shorter than 8 chars.
/// Fallback resolution: `extend_fields.requestId` -> `extend_fields.traceId` -> `fallback_id` -> `"0"`.
/// Returns `(updated_value, changed)`.
pub fn fix_invalid_id(parsed: &Value, fallback_id: Option<&str>) -> (Value, bool) {
    let Some(obj) = parsed.as_object() else {
        return (parsed.clone(), false);
    };
    let Some(id) = obj.get("id").and_then(|v| v.as_str()) else {
        return (parsed.clone(), false);
    };
    if !(id == "chat" || id == "completion" || id.len() < 8) {
        return (parsed.clone(), false);
    }

    let ext_fallback = obj
        .get("extend_fields")
        .and_then(|v| v.as_object())
        .and_then(|ext| {
            ext.get("requestId")
                .or_else(|| ext.get("traceId"))
                .and_then(|v| v.as_str())
        });
    let resolved = ext_fallback.or(fallback_id).unwrap_or("0");
    let new_id = format!("chatcmpl-{resolved}");

    let mut out = obj.clone();
    out.insert("id".to_string(), Value::String(new_id));
    (Value::Object(out), true)
}

/// Build a deterministic synthetic OpenAI-shaped chat completion chunk.
/// Caller supplies `id`, `created` (unix seconds), and `model` explicitly.
pub fn build_synthetic_chat_chunk(
    id: &str,
    model: &str,
    created: i64,
    delta: &Value,
    finish_reason: Option<&Value>,
) -> Value {
    json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": finish_reason.cloned().unwrap_or(Value::Null),
        }]
    })
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

    #[test]
    fn normalizes_multiline_data_events() {
        let mut n = SseDataLineNormalizer::new();
        let lines = vec![
            "event: message".to_string(),
            "data: {\"a\":".to_string(),
            "data: 1}".to_string(),
            "".to_string(),
        ];
        let out = n.normalize(&lines);
        assert_eq!(
            out,
            vec![
                "event: message".to_string(),
                "data: {\"a\":1}".to_string(),
                "".to_string(),
            ]
        );
        assert!(!n.has_pending());
    }

    #[test]
    fn normalizer_flushes_on_done_and_control_boundary() {
        let mut n = SseDataLineNormalizer::new();
        let out = n.normalize(&[
            "data: [DONE]".to_string(),
            "".to_string(),
            "data: {\"type\":\"response.output_text.delta\"}".to_string(),
            "event: next".to_string(),
        ]);
        assert_eq!(
            out,
            vec![
                "data: [DONE]".to_string(),
                "".to_string(),
                "data: {\"type\":\"response.output_text.delta\"}".to_string(),
                "".to_string(),
            ]
        );
        assert!(n.has_pending());

        let out2 = n.normalize(&["data: {}".to_string(), "".to_string()]);
        assert_eq!(
            out2,
            vec![
                "event: next".to_string(),
                "data: {}".to_string(),
                "".to_string()
            ]
        );
    }

    #[test]
    fn event_prefix_buffer_behaviour() {
        let mut b = SseEventPrefixBuffer::new();
        b.remember("event: response.delta");
        b.remember("id: 1");
        assert_eq!(b.event_type(), "response.delta");
        let prefixed = b.prefix_data("data: {\"x\":1}\n\n", "data: {\"x\":1}\n\n");
        assert_eq!(
            prefixed,
            "event: response.delta\nid: 1\ndata: {\"x\":1}\n\n"
        );
        // second prefix should not re-emit
        let again = b.prefix_data("data: {\"y\":2}\n\n", "data: {\"y\":2}\n\n");
        assert_eq!(again, "data: {\"y\":2}\n\n");
        b.clear();
        assert_eq!(b.flush(), "");
    }

    #[test]
    fn stream_payload_predicates() {
        assert!(is_openai_choices_payload(&json!({"choices": []})));
        assert!(!is_openai_choices_payload(&json!({"object": "x"})));

        assert!(has_openai_finish_reason(&json!({
            "choices": [{"finish_reason": "stop"}]
        })));
        assert!(!has_openai_finish_reason(&json!({
            "choices": [{"finish_reason": null}]
        })));

        assert!(is_known_non_claude_stream_payload(
            &json!({"choices":[{"delta":{"content":"hi"}}]}),
            ""
        ));
        assert!(!is_known_non_claude_stream_payload(
            &json!({"choices":[{"delta":{"content":""}}]}),
            ""
        ));
        assert!(is_known_non_claude_stream_payload(
            &json!({"type":"response.output_text.delta","delta":"hi"}),
            ""
        ));
        assert!(is_known_non_claude_stream_payload(
            &json!({"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}),
            ""
        ));
        assert!(is_known_non_claude_stream_payload(
            &json!({"response":{"candidates":[{"content":{"parts":[{"functionCall":{"name":"x"}}]}}]}}),
            ""
        ));
        assert!(!is_known_non_claude_stream_payload(
            &json!({"foo":"bar"}),
            ""
        ));
    }

    #[test]
    fn unwraps_gemini_envelope() {
        let wrapped = json!({"response":{"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}});
        assert_eq!(
            unwrap_gemini_chunk(&wrapped),
            json!({"candidates":[{"content":{"parts":[{"text":"hi"}]}}]})
        );
        let plain = json!({"candidates":[{"content":{"parts":[{"text":"x"}]}}]});
        assert_eq!(unwrap_gemini_chunk(&plain), plain);
        let other = json!({"response":{"foo":1}});
        assert_eq!(unwrap_gemini_chunk(&other), json!({"foo":1}));
    }

    #[test]
    fn normalizes_lines_in_batch() {
        let lines = vec![
            "data: {\"a\":".to_string(),
            "data: 1}".to_string(),
            "".to_string(),
        ];
        assert_eq!(
            normalize_sse_data_lines(&lines),
            vec!["data: {\"a\":1}".to_string(), "".to_string()]
        );
    }

    #[test]
    fn bounded_text_append_keeps_tail() {
        assert_eq!(append_bounded_text("ab", ""), "ab");
        assert_eq!(append_bounded_text("ab", "cd"), "abcd");
        let big = "x".repeat(STREAM_SUMMARY_TEXT_LIMIT);
        assert_eq!(
            append_bounded_text(&big, "y").len(),
            STREAM_SUMMARY_TEXT_LIMIT
        );
        assert!(append_bounded_text(&big, "y").ends_with('y'));
        let over = "z".repeat(STREAM_SUMMARY_TEXT_LIMIT + 10);
        assert_eq!(
            append_bounded_text("a", &over).len(),
            STREAM_SUMMARY_TEXT_LIMIT
        );
    }

    #[test]
    fn active_delta_value_detection() {
        assert!(!has_active_delta_value(&json!("")));
        assert!(has_active_delta_value(&json!("hi")));
        assert!(has_active_delta_value(&json!(1)));
        assert!(!has_active_delta_value(&json!(null)));
        assert!(has_active_delta_value(&json!(["", "x"])));
        assert!(!has_active_delta_value(&json!(["", null])));
        assert!(has_active_delta_value(&json!({"a": {"b": "x"}})));
        assert!(!has_active_delta_value(&json!({"a": {"b": ""}})));
    }

    #[test]
    fn fixes_invalid_ids_deterministically() {
        let (v, changed) = fix_invalid_id(&json!({"id":"chat","x":1}), Some("fb"));
        assert!(changed);
        assert_eq!(v["id"], "chatcmpl-fb");

        let (v, changed) = fix_invalid_id(
            &json!({"id":"short","extend_fields":{"requestId":"r1"}}),
            Some("fb"),
        );
        assert!(changed);
        assert_eq!(v["id"], "chatcmpl-r1");

        let (v, changed) = fix_invalid_id(&json!({"id":"chatcmpl-validid"}), Some("fb"));
        assert!(!changed);
        assert_eq!(v["id"], "chatcmpl-validid");

        let (v, changed) = fix_invalid_id(&json!({"id":"completion"}), None);
        assert!(changed);
        assert_eq!(v["id"], "chatcmpl-0");
    }

    #[test]
    fn builds_synthetic_chat_chunk_deterministically() {
        let chunk = build_synthetic_chat_chunk(
            "chatcmpl-test",
            "gpt-x",
            123,
            &json!({"content":"hi"}),
            Some(&json!("stop")),
        );
        assert_eq!(chunk["id"], "chatcmpl-test");
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["created"], 123);
        assert_eq!(chunk["model"], "gpt-x");
        assert_eq!(chunk["choices"][0]["delta"]["content"], "hi");
        assert_eq!(chunk["choices"][0]["finish_reason"], "stop");

        let chunk2 = build_synthetic_chat_chunk("id", "m", 0, &json!({}), None);
        assert!(chunk2["choices"][0]["finish_reason"].is_null());
    }
}
