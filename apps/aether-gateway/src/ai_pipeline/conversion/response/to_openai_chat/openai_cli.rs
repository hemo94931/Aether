use serde_json::{json, Map, Value};

use super::shared::{build_generated_tool_call_id, canonicalize_tool_arguments};

pub(crate) fn convert_openai_cli_response_to_openai_chat(
    body_json: &Value,
    report_context: &Value,
) -> Option<Value> {
    let body = body_json.as_object()?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();

    if let Some(output_items) = body.get("output").and_then(Value::as_array) {
        for (index, item) in output_items.iter().enumerate() {
            let item_object = item.as_object()?;
            let item_type = item_object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            match item_type.as_str() {
                "message" => {
                    if let Some(content) = item_object.get("content").and_then(Value::as_array) {
                        for part in content {
                            let part_object = part.as_object()?;
                            let part_type = part_object
                                .get("type")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .trim()
                                .to_ascii_lowercase();
                            if matches!(part_type.as_str(), "output_text" | "text") {
                                if let Some(piece) = part_object.get("text").and_then(Value::as_str)
                                {
                                    text.push_str(piece);
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let tool_name = item_object
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())?;
                    let tool_id = item_object
                        .get("call_id")
                        .and_then(Value::as_str)
                        .filter(|value| !value.is_empty())
                        .or_else(|| {
                            item_object
                                .get("id")
                                .and_then(Value::as_str)
                                .filter(|value| !value.is_empty())
                        })
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| build_generated_tool_call_id(index));
                    tool_calls.push(json!({
                        "id": tool_id,
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "arguments": canonicalize_tool_arguments(item_object.get("arguments").cloned()),
                        }
                    }));
                }
                "output_text" | "text" => {
                    if let Some(piece) = item_object.get("text").and_then(Value::as_str) {
                        text.push_str(piece);
                    }
                }
                _ => {}
            }
        }
    }

    let finish_reason = if tool_calls.is_empty() {
        Some("stop")
    } else {
        Some("tool_calls")
    };
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| report_context.get("mapped_model").and_then(Value::as_str))
        .or_else(|| report_context.get("model").and_then(Value::as_str))
        .unwrap_or("unknown");
    let id = body
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("chatcmpl-local-openai-cli");

    let usage = body.get("usage").and_then(Value::as_object);
    let prompt_tokens = usage
        .and_then(|value| value.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|value| value.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .and_then(|value| value.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(prompt_tokens + completion_tokens);

    let mut message = Map::new();
    message.insert("role".to_string(), Value::String("assistant".to_string()));
    if text.is_empty() && !tool_calls.is_empty() {
        message.insert("content".to_string(), Value::Null);
    } else {
        message.insert("content".to_string(), Value::String(text));
    }
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
