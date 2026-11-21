//! Pure reasoning-field helpers (opt-in).
//!
//! Bounded parity helpers for reasoning/thinking field handling patterns used in
//! OmniRoute translator/schema flows. No runtime wiring.

use serde_json::Value;

/// True when `message.reasoning_content` is a non-empty string.
///
/// Parity with TS `hasNonEmptyReasoningContent` in `open-sse/translator/index.ts`.
#[must_use]
pub fn has_non_empty_reasoning_content(message: &Value) -> bool {
    message
        .get("reasoning_content")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
}

/// Ensure assistant tool-call messages carry `reasoning_content`.
///
/// If a message is an assistant message with non-empty `tool_calls` and no
/// `reasoning_content` key, set `reasoning_content` to `""`.
///
/// Parity with TS `ensureReasoningContentOnToolCalls` in
/// `open-sse/translator/helpers/schemaCoercion.ts`.
#[must_use]
pub fn ensure_reasoning_content_on_tool_calls(messages: &Value) -> Value {
    let Some(arr) = messages.as_array() else {
        return messages.clone();
    };

    Value::Array(
        arr.iter()
            .map(|message| {
                let Some(obj) = message.as_object() else {
                    return message.clone();
                };
                let is_assistant = obj.get("role").and_then(Value::as_str) == Some("assistant");
                let has_tool_calls = obj
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|tc| !tc.is_empty());
                let has_reasoning = obj.contains_key("reasoning_content");
                if is_assistant && has_tool_calls && !has_reasoning {
                    let mut out = obj.clone();
                    out.insert(
                        "reasoning_content".to_string(),
                        Value::String(String::new()),
                    );
                    Value::Object(out)
                } else {
                    message.clone()
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn has_non_empty_reasoning_content_cases() {
        assert!(!has_non_empty_reasoning_content(&json!({})));
        assert!(!has_non_empty_reasoning_content(
            &json!({"reasoning_content": ""})
        ));
        assert!(!has_non_empty_reasoning_content(
            &json!({"reasoning_content": 1})
        ));
        assert!(has_non_empty_reasoning_content(
            &json!({"reasoning_content": "thought"})
        ));
    }

    #[test]
    fn ensure_reasoning_content_on_tool_calls_adds_empty_string() {
        let input = json!([
            {"role":"assistant","tool_calls":[{"id":"1"}],"content":null},
            {"role":"assistant","tool_calls":[{"id":"2"}],"reasoning_content":"keep"},
            {"role":"assistant","content":"plain"},
            {"role":"user","tool_calls":[{"id":"3"}]},
            "not-object"
        ]);
        let out = ensure_reasoning_content_on_tool_calls(&input);
        assert_eq!(out[0]["reasoning_content"], "");
        assert_eq!(out[1]["reasoning_content"], "keep");
        assert!(out[2].get("reasoning_content").is_none());
        assert!(out[3].get("reasoning_content").is_none());
        assert_eq!(out[4], "not-object");
    }

    #[test]
    fn ensure_reasoning_content_non_array_passthrough() {
        let input = json!({"role":"assistant"});
        assert_eq!(ensure_reasoning_content_on_tool_calls(&input), input);
    }

    #[test]
    fn ensure_reasoning_content_no_tool_calls_passthrough() {
        let input = json!([{"role":"assistant","tool_calls":[] }]);
        let out = ensure_reasoning_content_on_tool_calls(&input);
        assert!(out[0].get("reasoning_content").is_none());
    }
}
