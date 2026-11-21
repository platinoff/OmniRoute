//! Lite compression body-level transforms ported from `open-sse/services/compression/lite.ts`.
//! Pure JSON/message transforms using portable text primitives.

use crate::lite_text::{normalize_message_whitespace, truncate_tool_content};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiteCompressionOptions {
    pub preserve_system_prompt: bool,
    /// Explicit vision support. `Some(false)` enables image stripping; other values keep images.
    /// Mirrors TS: `supportsVision !== false` => no strip.
    pub supports_vision: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct BodyTransformResult {
    pub body: Value,
    pub applied: bool,
}

fn result(body: Value, applied: bool) -> BodyTransformResult {
    BodyTransformResult { body, applied }
}

/// Collapse whitespace in string message contents.
/// Skips system messages when `preserve_system_prompt` is true.
pub fn collapse_whitespace(body: &Value, options: LiteCompressionOptions) -> BodyTransformResult {
    let Some(obj) = body.as_object() else {
        return result(body.clone(), false);
    };
    let Some(messages) = obj.get("messages").and_then(|v| v.as_array()) else {
        return result(body.clone(), false);
    };

    let mut applied = false;
    let mut out_msgs: Vec<Value> = Vec::with_capacity(messages.len());
    for msg in messages {
        let Some(m) = msg.as_object() else {
            out_msgs.push(msg.clone());
            continue;
        };
        if options.preserve_system_prompt
            && m.get("role").and_then(|r| r.as_str()) == Some("system")
        {
            out_msgs.push(msg.clone());
            continue;
        }
        if let Some(Value::String(content)) = m.get("content") {
            let normalized = normalize_message_whitespace(content);
            if &normalized != content {
                applied = true;
                let mut nm = m.clone();
                nm.insert("content".to_string(), Value::String(normalized));
                out_msgs.push(Value::Object(nm));
                continue;
            }
        }
        out_msgs.push(msg.clone());
    }

    if !applied {
        return result(body.clone(), false);
    }
    let mut out = obj.clone();
    out.insert("messages".to_string(), Value::Array(out_msgs));
    result(Value::Object(out), true)
}

/// Deduplicate system prompt messages by first 200 chars (trimmed) key.
/// Skipped entirely when `preserve_system_prompt` is true.
pub fn dedup_system_prompt(body: &Value, options: LiteCompressionOptions) -> BodyTransformResult {
    let Some(obj) = body.as_object() else {
        return result(body.clone(), false);
    };
    let Some(messages) = obj.get("messages").and_then(|v| v.as_array()) else {
        return result(body.clone(), false);
    };
    if options.preserve_system_prompt {
        return result(body.clone(), false);
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut applied = false;
    let mut out_msgs: Vec<Value> = Vec::with_capacity(messages.len());
    for msg in messages {
        let Some(m) = msg.as_object() else {
            out_msgs.push(msg.clone());
            continue;
        };
        if m.get("role").and_then(|r| r.as_str()) == Some("system") {
            if let Some(Value::String(content)) = m.get("content") {
                let trimmed = content.trim();
                let key: String = trimmed.chars().take(200).collect();
                if seen.contains(&key) {
                    applied = true;
                    continue;
                }
                seen.insert(key);
                out_msgs.push(msg.clone());
                continue;
            }
        }
        out_msgs.push(msg.clone());
    }

    if !applied {
        return result(body.clone(), false);
    }
    let mut out = obj.clone();
    out.insert("messages".to_string(), Value::Array(out_msgs));
    result(Value::Object(out), true)
}

/// Compress tool result messages longer than 2000 chars with word-boundary truncation.
pub fn compress_tool_results(body: &Value) -> BodyTransformResult {
    const MAX_TOOL_LENGTH: usize = 2000;
    let Some(obj) = body.as_object() else {
        return result(body.clone(), false);
    };
    let Some(messages) = obj.get("messages").and_then(|v| v.as_array()) else {
        return result(body.clone(), false);
    };

    let mut applied = false;
    let mut out_msgs: Vec<Value> = Vec::with_capacity(messages.len());
    for msg in messages {
        let Some(m) = msg.as_object() else {
            out_msgs.push(msg.clone());
            continue;
        };
        if m.get("role").and_then(|r| r.as_str()) == Some("tool") {
            if let Some(Value::String(content)) = m.get("content") {
                if content.len() > MAX_TOOL_LENGTH {
                    applied = true;
                    let mut nm = m.clone();
                    nm.insert(
                        "content".to_string(),
                        Value::String(truncate_tool_content(content, MAX_TOOL_LENGTH)),
                    );
                    out_msgs.push(Value::Object(nm));
                    continue;
                }
            }
        }
        out_msgs.push(msg.clone());
    }

    if !applied {
        return result(body.clone(), false);
    }
    let mut out = obj.clone();
    out.insert("messages".to_string(), Value::Array(out_msgs));
    result(Value::Object(out), true)
}

/// Remove consecutive messages with identical role and string content.
/// Mirrors TS comparison semantics (previous content must be string-equal to current string form).
pub fn remove_redundant_content(
    body: &Value,
    options: LiteCompressionOptions,
) -> BodyTransformResult {
    let Some(obj) = body.as_object() else {
        return result(body.clone(), false);
    };
    let Some(messages) = obj.get("messages").and_then(|v| v.as_array()) else {
        return result(body.clone(), false);
    };

    let mut applied = false;
    let mut out_msgs: Vec<Value> = Vec::new();
    for (i, msg) in messages.iter().enumerate() {
        let Some(m) = msg.as_object() else {
            out_msgs.push(msg.clone());
            continue;
        };
        if options.preserve_system_prompt
            && m.get("role").and_then(|r| r.as_str()) == Some("system")
        {
            out_msgs.push(msg.clone());
            continue;
        }

        let content_str = match m.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(v) => serde_json::to_string(v).unwrap_or_default(),
            None => "null".to_string(),
        };

        if i > 0 {
            let prev = &messages[i - 1];
            let same_role = prev.get("role") == m.get("role");
            let prev_is_string = prev.get("content").map(|c| c.is_string()).unwrap_or(false);
            let prev_str = prev.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if same_role && prev_is_string && prev_str == content_str {
                applied = true;
                continue;
            }
        }
        out_msgs.push(msg.clone());
    }

    if !applied {
        return result(body.clone(), false);
    }
    let mut out = obj.clone();
    out.insert("messages".to_string(), Value::Array(out_msgs));
    result(Value::Object(out), true)
}

/// Replace data-image base64 URLs in `image_url` parts with text placeholders.
/// Only strips when `supports_vision == Some(false)` (TS: `supportsVision !== false` => no-op).
pub fn replace_image_urls(body: &Value, supports_vision: Option<bool>) -> BodyTransformResult {
    if supports_vision != Some(false) {
        return result(body.clone(), false);
    }
    let Some(obj) = body.as_object() else {
        return result(body.clone(), false);
    };
    let Some(messages) = obj.get("messages").and_then(|v| v.as_array()) else {
        return result(body.clone(), false);
    };

    let mut applied = false;
    let mut out_msgs: Vec<Value> = Vec::with_capacity(messages.len());
    for msg in messages {
        let Some(m) = msg.as_object() else {
            out_msgs.push(msg.clone());
            continue;
        };
        let Some(Value::Array(parts)) = m.get("content") else {
            out_msgs.push(msg.clone());
            continue;
        };

        let mut new_parts: Vec<Value> = Vec::with_capacity(parts.len());
        let mut changed = false;
        for part in parts {
            let mut replaced_part = None;
            if let Some(p) = part.as_object() {
                if p.get("type").and_then(|t| t.as_str()) == Some("image_url") {
                    if let Some(img_obj) = p.get("image_url").and_then(|v| v.as_object()) {
                        if let Some(Value::String(url)) = img_obj.get("url") {
                            if url.starts_with("data:image/") {
                                let format = url
                                    .split_once('/')
                                    .and_then(|(_, rest)| rest.split_once(';').map(|(f, _)| f))
                                    .unwrap_or("unknown");
                                replaced_part = Some(json!({
                                    "type": "text",
                                    "text": format!("[image: {format}]")
                                }));
                            }
                        }
                    }
                }
            }
            if let Some(rp) = replaced_part {
                applied = true;
                changed = true;
                new_parts.push(rp);
            } else {
                new_parts.push(part.clone());
            }
        }

        if changed {
            let mut nm = m.clone();
            nm.insert("content".to_string(), Value::Array(new_parts));
            out_msgs.push(Value::Object(nm));
        } else {
            out_msgs.push(msg.clone());
        }
    }

    if !applied {
        return result(body.clone(), false);
    }
    let mut out = obj.clone();
    out.insert("messages".to_string(), Value::Array(out_msgs));
    result(Value::Object(out), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collapses_whitespace_and_respects_preserve_system() {
        let body = json!({
            "messages": [
                {"role":"system","content":"a \n\n\nb\t"},
                {"role":"user","content":"x \n\n\ny\t"}
            ]
        });
        let r = collapse_whitespace(
            &body,
            LiteCompressionOptions {
                preserve_system_prompt: true,
                supports_vision: None,
            },
        );
        assert!(r.applied);
        assert_eq!(r.body["messages"][0]["content"], "a \n\n\nb\t");
        assert_eq!(r.body["messages"][1]["content"], "x\n\ny");
    }

    #[test]
    fn dedups_system_prompts() {
        let body = json!({
            "messages": [
                {"role":"system","content":" same prompt "},
                {"role":"user","content":"hi"},
                {"role":"system","content":"same prompt"}
            ]
        });
        let r = dedup_system_prompt(&body, LiteCompressionOptions::default());
        assert!(r.applied);
        assert_eq!(r.body["messages"].as_array().unwrap().len(), 2);

        let r = dedup_system_prompt(
            &body,
            LiteCompressionOptions {
                preserve_system_prompt: true,
                supports_vision: None,
            },
        );
        assert!(!r.applied);
    }

    #[test]
    fn compresses_tool_results() {
        let long = "hello wonderful world ".repeat(200); // > 2000 chars
        let body = json!({"messages":[{"role":"tool","content": long}]});
        let r = compress_tool_results(&body);
        assert!(r.applied);
        let c = r.body["messages"][0]["content"].as_str().unwrap();
        assert!(c.ends_with("\n...[truncated]"));
        assert!(c.len() < long.len());
    }

    #[test]
    fn removes_redundant_consecutive_content() {
        let body = json!({
            "messages": [
                {"role":"user","content":"same"},
                {"role":"user","content":"same"},
                {"role":"assistant","content":"same"},
                {"role":"user","content":[{"type":"text","text":"x"}]}
            ]
        });
        let r = remove_redundant_content(&body, LiteCompressionOptions::default());
        assert!(r.applied);
        assert_eq!(r.body["messages"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn replaces_image_urls_only_when_vision_false() {
        let body = json!({
            "messages": [{
                "role":"user",
                "content":[
                    {"type":"text","text":"look"},
                    {"type":"image_url","image_url":{"url":"data:image/png;base64,abc"}}
                ]
            }]
        });
        let r = replace_image_urls(&body, Some(true));
        assert!(!r.applied);
        let r = replace_image_urls(&body, None);
        assert!(!r.applied);
        let r = replace_image_urls(&body, Some(false));
        assert!(r.applied);
        assert_eq!(r.body["messages"][0]["content"][1]["type"], "text");
        assert_eq!(r.body["messages"][0]["content"][1]["text"], "[image: png]");
    }
}
