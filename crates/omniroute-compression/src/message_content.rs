//! Message content helpers ported from `open-sse/services/compression/messageContent.ts`.
//! Pure JSON/data abstractions for chat message content.

use serde_json::{Map, Value};

/// Returns true for text-like content blocks (`type` absent, `"text"`, or `"input_text"`).
pub fn is_text_block(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    if !matches!(obj.get("text"), Some(Value::String(_))) {
        return false;
    }
    match obj.get("type") {
        None => true,
        Some(Value::String(t)) => t == "text" || t == "input_text",
        _ => false,
    }
}

/// Extract concatenated text content from a string or array-of-blocks message content.
pub fn extract_text_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => {
            let mut out: Vec<String> = Vec::new();
            for part in parts {
                if is_text_block(part) {
                    if let Some(Value::String(text)) = part.get("text") {
                        if !text.is_empty() {
                            out.push(text.clone());
                        }
                    }
                }
            }
            out.join("\n")
        }
        _ => String::new(),
    }
}

/// Map/transform text in message content while preserving non-text blocks.
/// `transform(text, index)` is called for each text block (index increments per text block).
pub fn map_text_content(msg: &Value, mut transform: impl FnMut(&str, usize) -> String) -> Value {
    let Some(obj) = msg.as_object() else {
        return msg.clone();
    };
    let Some(content) = obj.get("content") else {
        return msg.clone();
    };

    match content {
        Value::String(s) => {
            let next = transform(s, 0);
            let mut out = obj.clone();
            out.insert("content".to_string(), Value::String(next));
            Value::Object(out)
        }
        Value::Array(parts) => {
            let mut text_index: usize = 0;
            let mut changed = false;
            let mut new_parts: Vec<Value> = Vec::with_capacity(parts.len());
            for part in parts {
                if !is_text_block(part) {
                    new_parts.push(part.clone());
                    continue;
                }
                let text = part.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let next_text = transform(text, text_index);
                text_index += 1;
                if next_text == text {
                    new_parts.push(part.clone());
                } else {
                    changed = true;
                    if let Value::Object(mut p) = part.clone() {
                        p.insert("text".to_string(), Value::String(next_text));
                        new_parts.push(Value::Object(p));
                    } else {
                        new_parts.push(part.clone());
                    }
                }
            }
            if !changed {
                msg.clone()
            } else {
                let mut out = obj.clone();
                out.insert("content".to_string(), Value::Array(new_parts));
                Value::Object(out)
            }
        }
        _ => msg.clone(),
    }
}

/// Replace text content in a message. Preserves non-text blocks.
/// For array content: first text block gets `new_text`; trailing text blocks are dropped
/// unless their text is not contained in `new_text` (to avoid silent loss).
pub fn replace_text_content(msg: &Value, new_text: &str) -> Value {
    let Some(obj) = msg.as_object() else {
        return msg.clone();
    };
    let content = obj.get("content");

    match content {
        Some(Value::Array(parts)) => {
            let mut replaced = false;
            let mut new_parts: Vec<Value> = Vec::new();
            for part in parts {
                if !is_text_block(part) {
                    new_parts.push(part.clone());
                    continue;
                }
                if !replaced {
                    replaced = true;
                    if let Value::Object(mut p) = part.clone() {
                        p.insert("text".to_string(), Value::String(new_text.to_string()));
                        new_parts.push(Value::Object(p));
                    } else {
                        let mut p = Map::new();
                        p.insert("type".to_string(), Value::String("text".to_string()));
                        p.insert("text".to_string(), Value::String(new_text.to_string()));
                        new_parts.push(Value::Object(p));
                    }
                } else {
                    let part_text = part.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    if !part_text.is_empty() && !new_text.contains(part_text) {
                        new_parts.push(part.clone());
                    }
                    // else drop (subsumed)
                }
            }

            if !replaced {
                let mut text_block = Map::new();
                text_block.insert("type".to_string(), Value::String("text".to_string()));
                text_block.insert("text".to_string(), Value::String(new_text.to_string()));
                let mut prefixed: Vec<Value> = Vec::with_capacity(parts.len() + 1);
                prefixed.push(Value::Object(text_block));
                prefixed.extend(parts.iter().cloned());
                let mut out = obj.clone();
                out.insert("content".to_string(), Value::Array(prefixed));
                return Value::Object(out);
            }

            let mut out = obj.clone();
            out.insert("content".to_string(), Value::Array(new_parts));
            Value::Object(out)
        }
        _ => {
            let mut out = obj.clone();
            out.insert("content".to_string(), Value::String(new_text.to_string()));
            Value::Object(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_block_detection() {
        assert!(is_text_block(&json!({"type":"text","text":"hi"})));
        assert!(is_text_block(&json!({"type":"input_text","text":"hi"})));
        assert!(is_text_block(&json!({"text":"hi"})));
        assert!(!is_text_block(&json!({"type":"image_url","text":"x"})));
        assert!(!is_text_block(&json!({"type":"text"})));
        assert!(!is_text_block(&json!("x")));
    }

    #[test]
    fn extract_text_variants() {
        assert_eq!(extract_text_content(Some(&json!("hello"))), "hello");
        assert_eq!(
            extract_text_content(Some(&json!([
                {"type":"text","text":"a"},
                {"type":"image_url","image_url":{"url":"x"}},
                {"type":"text","text":"b"}
            ]))),
            "a\nb"
        );
        assert_eq!(
            extract_text_content(Some(&json!([{"type":"text","text":""}]))),
            ""
        );
        assert_eq!(extract_text_content(None), "");
    }

    #[test]
    fn map_text_content_cases() {
        let msg = json!({"role":"user","content":"abc"});
        let out = map_text_content(&msg, |t, i| format!("{t}:{i}"));
        assert_eq!(out["content"], "abc:0");

        let msg = json!({"role":"user","content":[
            {"type":"text","text":"a"},
            {"type":"image_url","image_url":{"url":"x"}},
            {"type":"text","text":"b"}
        ]});
        let out = map_text_content(&msg, |t, i| format!("{t}{i}"));
        assert_eq!(out["content"][0]["text"], "a0");
        assert_eq!(out["content"][2]["text"], "b1");
        assert_eq!(out["content"][1]["type"], "image_url");

        let same = map_text_content(&msg, |t, _| t.to_string());
        assert_eq!(same, msg);
    }

    #[test]
    fn replace_text_content_cases() {
        let msg = json!({"role":"user","content":"old"});
        assert_eq!(replace_text_content(&msg, "new")["content"], "new");

        let msg = json!({"role":"user","content":[
            {"type":"text","text":"a"},
            {"type":"image_url","image_url":{"url":"x"}},
            {"type":"text","text":"b"}
        ], "x": 1});
        let out = replace_text_content(&msg, "joined a\nb");
        assert_eq!(out["content"][0]["text"], "joined a\nb");
        assert_eq!(out["content"][1]["type"], "image_url");
        assert_eq!(out["content"].as_array().unwrap().len(), 2);
        assert_eq!(out["x"], 1);

        // trailing text not subsumed -> preserved
        let msg = json!({"role":"user","content":[
            {"type":"text","text":"a"},
            {"type":"text","text":"keepme"}
        ]});
        let out = replace_text_content(&msg, "only-a");
        assert_eq!(out["content"].as_array().unwrap().len(), 2);
        assert_eq!(out["content"][1]["text"], "keepme");

        // no text blocks -> prepend
        let msg = json!({"role":"user","content":[{"type":"image_url","image_url":{"url":"x"}}]});
        let out = replace_text_content(&msg, "desc");
        assert_eq!(out["content"][0]["text"], "desc");
        assert_eq!(out["content"][1]["type"], "image_url");
    }
}
