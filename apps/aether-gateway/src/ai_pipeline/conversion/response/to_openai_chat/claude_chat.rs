use serde_json::{json, Map, Value};

use super::shared::{build_generated_tool_call_id, canonicalize_tool_arguments};

pub(crate) fn convert_claude_chat_response_to_openai_chat(
    body_json: &Value,
    report_context: &Value,
) -> Option<Value> {
    let body = body_json.as_object()?;
    let content = body.get("content")?.as_array()?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for (index, block) in content.iter().enumerate() {
        let block = block.as_object()?;
        match block.get("type")?.as_str()? {
            "text" => {
                text.push_str(block.get("text")?.as_str()?);
            }
            "tool_use" => {
                let tool_name = block.get("name")?.as_str()?;
                let tool_id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| build_generated_tool_call_id(index));
                let arguments = canonicalize_tool_arguments(block.get("input").cloned());
                tool_calls.push(json!({
                    "id": tool_id,
                    "type": "function",
                    "function": {
                        "name": tool_name,
                        "arguments": arguments,
                    }
                }));
            }
            _ => return None,
        }
    }
    let mut finish_reason = match body.get("stop_reason").and_then(Value::as_str) {
        Some("end_turn") | Some("stop_sequence") => Some("stop"),
        Some("max_tokens") => Some("length"),
        Some("tool_use") => Some("tool_calls"),
        Some(other) if !other.is_empty() => Some(other),
        _ => None,
    };
    if !tool_calls.is_empty() && finish_reason.is_none_or(|reason| reason == "stop") {
        finish_reason = Some("tool_calls");
    }
    let usage = body.get("usage").and_then(Value::as_object);
    let prompt_tokens = usage
        .and_then(|value| value.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|value| value.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = prompt_tokens + completion_tokens;
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| report_context.get("mapped_model").and_then(Value::as_str))
        .or_else(|| report_context.get("model").and_then(Value::as_str))
        .unwrap_or("unknown");
    let id = body
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("chatcmpl-local-finalize");
    let message_content = if text.is_empty() && !tool_calls.is_empty() {
        Value::Null
    } else {
        Value::String(text)
    };
    let mut message = Map::new();
    message.insert("role".to_string(), Value::String("assistant".to_string()));
    message.insert("content".to_string(), message_content);
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    Some(json!({
        "id": id,
        "object": "chat.completion",
        "model": model,
        "choices": [{
            "index": 0,
            "message": Value::Object(message),
            "finish_reason": finish_reason,
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
        }
    }))
}
