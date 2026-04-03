use serde_json::{json, Value};

use super::shared::{
    build_generated_tool_call_id, extract_openai_assistant_text, parse_openai_function_arguments,
};

pub(crate) fn convert_openai_chat_response_to_claude_chat(
    body_json: &Value,
    report_context: &Value,
) -> Option<Value> {
    let body = body_json.as_object()?;
    let choices = body.get("choices")?.as_array()?;
    let first_choice = choices.first()?.as_object()?;
    let message = first_choice.get("message")?.as_object()?;
    let mut content = Vec::new();

    if let Some(text) = extract_openai_assistant_text(message.get("content")) {
        if !text.trim().is_empty() {
            content.push(json!({
                "type": "text",
                "text": text,
            }));
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
            let tool_id = tool_call
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| build_generated_tool_call_id(index));
            let input = parse_openai_function_arguments(function.get("arguments"))?;
            content.push(json!({
                "type": "tool_use",
                "id": tool_id,
                "name": tool_name,
                "input": input,
            }));
        }
    }
    if content.is_empty() {
        content.push(json!({
            "type": "text",
            "text": "",
        }));
    }

    let stop_reason = match first_choice.get("finish_reason").and_then(Value::as_str) {
        Some("stop") | None => "end_turn",
        Some("length") => "max_tokens",
        Some("tool_calls") | Some("function_call") => "tool_use",
        Some("content_filter") => "content_filtered",
        Some(other) => other,
    };
    let usage = body.get("usage").and_then(Value::as_object);
    let input_tokens = usage
        .and_then(|value| value.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|value| value.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| report_context.get("mapped_model").and_then(Value::as_str))
        .or_else(|| report_context.get("model").and_then(Value::as_str))
        .unwrap_or("unknown");
    let id = body
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("msg-local-finalize");

    Some(json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": stop_reason,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
        }
    }))
}
