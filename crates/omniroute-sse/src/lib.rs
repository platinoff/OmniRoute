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
}
