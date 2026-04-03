use serde_json::{json, Map, Value};

use super::shared::{build_generated_tool_call_id, canonicalize_tool_arguments};

pub(crate) fn convert_gemini_chat_response_to_openai_chat(
    body_json: &Value,
    report_context: &Value,
) -> Option<Value> {
    let body = body_json.as_object()?;
    let candidates = body.get("candidates")?.as_array()?;
    let first_candidate = candidates.first()?.as_object()?;
    let content = first_candidate.get("content")?.as_object()?;
    let parts = content.get("parts")?.as_array()?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let part = part.as_object()?;
        if let Some(piece) = part.get("text").and_then(Value::as_str) {
            text.push_str(piece);
        } else if let Some(function_call) = part.get("functionCall").and_then(Value::as_object) {
            let tool_name = function_call.get("name")?.as_str()?;
            let tool_id = function_call
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| build_generated_tool_call_id(index));
            let arguments = canonicalize_tool_arguments(function_call.get("args").cloned());
            tool_calls.push(json!({
                "id": tool_id,
                "type": "function",
                "function": {
                    "name": tool_name,
                    "arguments": arguments,
                }
            }));
        } else {
            return None;
        }
    }
    let mut finish_reason = match first_candidate.get("finishReason").and_then(Value::as_str) {
        Some("STOP") => Some("stop"),
        Some("MAX_TOKENS") => Some("length"),
        Some("SAFETY") => Some("content_filter"),
        Some(other) if !other.is_empty() => Some(other),
        _ => None,
    };
    if !tool_calls.is_empty() && finish_reason.is_none_or(|reason| reason == "stop") {
        finish_reason = Some("tool_calls");
    }
    let usage = body.get("usageMetadata").and_then(Value::as_object);
    let prompt_tokens = usage
        .and_then(|value| value.get("promptTokenCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|value| value.get("candidatesTokenCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .and_then(|value| value.get("totalTokenCount"))
        .and_then(Value::as_u64)
        .unwrap_or(prompt_tokens + completion_tokens);
    let model = body
        .get("modelVersion")
        .and_then(Value::as_str)
        .or_else(|| report_context.get("mapped_model").and_then(Value::as_str))
        .or_else(|| report_context.get("model").and_then(Value::as_str))
        .unwrap_or("unknown");
    let id = body
        .get("responseId")
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
            "index": first_candidate.get("index").and_then(Value::as_u64).unwrap_or(0),
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
