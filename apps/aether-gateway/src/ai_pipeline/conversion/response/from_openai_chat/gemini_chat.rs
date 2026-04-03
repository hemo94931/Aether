use serde_json::{json, Value};

use super::shared::{
    build_generated_tool_call_id, extract_openai_assistant_text, parse_openai_function_arguments,
};

pub(crate) fn convert_openai_chat_response_to_gemini_chat(
    body_json: &Value,
    report_context: &Value,
) -> Option<Value> {
    let body = body_json.as_object()?;
    let choices = body.get("choices")?.as_array()?;
    let first_choice = choices.first()?.as_object()?;
    let message = first_choice.get("message")?.as_object()?;
    let mut parts = Vec::new();

    if let Some(text) = extract_openai_assistant_text(message.get("content")) {
        if !text.trim().is_empty() {
            parts.push(json!({ "text": text }));
        }
    }
    if let Some(tool_call_values) = message.get("tool_calls").and_then(Value::as_array) {
        for (index, tool_call) in tool_call_values.iter().enumerate() {
            let tool_call = tool_call.as_object()?;
            let function = tool_call.get("function")?.as_object()?;
            let tool_name = function
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let call_id = tool_call
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| build_generated_tool_call_id(index));
            parts.push(json!({
                "functionCall": {
                    "id": call_id,
                    "name": tool_name,
                    "args": parse_openai_function_arguments(function.get("arguments"))?,
                }
            }));
        }
    }
    if parts.is_empty() {
        parts.push(json!({ "text": "" }));
    }

    let usage = body.get("usage").and_then(Value::as_object);
    let prompt_tokens = usage
        .and_then(|value| value.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|value| value.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .and_then(|value| value.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(prompt_tokens + completion_tokens);
    let mut finish_reason = match first_choice.get("finish_reason").and_then(Value::as_str) {
        Some("stop") | None => "STOP",
        Some("length") => "MAX_TOKENS",
        Some("content_filter") => "SAFETY",
        Some("tool_calls") | Some("function_call") => "STOP",
        Some(other) => other,
    };
    if parts.iter().any(|part| part.get("functionCall").is_some()) {
        finish_reason = "STOP";
    }
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| report_context.get("mapped_model").and_then(Value::as_str))
        .or_else(|| report_context.get("model").and_then(Value::as_str))
        .unwrap_or("unknown");
    let response_id = body
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("resp-local-finalize");

    Some(json!({
        "responseId": response_id,
        "modelVersion": model,
        "candidates": [{
            "content": {
                "role": "model",
                "parts": parts,
            },
            "finishReason": finish_reason,
            "index": 0,
        }],
        "usageMetadata": {
            "promptTokenCount": prompt_tokens,
            "candidatesTokenCount": completion_tokens,
            "totalTokenCount": total_tokens,
        }
    }))
}
