use serde_json::{json, Map, Value};

pub(crate) fn build_openai_cli_response(
    response_id: &str,
    model: &str,
    text: &str,
    function_calls: Vec<Value>,
    prompt_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
) -> Value {
    let mut output = Vec::new();
    if !text.is_empty() {
        output.push(json!({
            "type": "message",
            "id": format!("{response_id}_msg"),
            "role": "assistant",
            "status": "completed",
            "content": [{
                "type": "output_text",
                "text": text,
                "annotations": []
            }]
        }));
    }
    output.extend(function_calls);
    json!({
        "id": response_id,
        "object": "response",
        "status": "completed",
        "model": model,
        "output": output,
        "usage": {
            "input_tokens": prompt_tokens,
            "output_tokens": output_tokens,
            "total_tokens": total_tokens,
        }
    })
}

pub(super) fn extract_openai_assistant_text(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::Null => Some(String::new()),
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let mut text = String::new();
            for part in parts {
                let part = part.as_object()?;
                let part_type = part
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase();
                if matches!(part_type.as_str(), "text" | "output_text") {
                    if let Some(piece) = part.get("text").and_then(Value::as_str) {
                        text.push_str(piece);
                    }
                }
            }
            Some(text)
        }
        _ => None,
    }
}

pub(super) fn parse_openai_function_arguments(arguments: Option<&Value>) -> Option<Value> {
    match arguments.cloned().unwrap_or(Value::Object(Map::new())) {
        Value::String(text) => serde_json::from_str(&text)
            .ok()
            .or(Some(Value::String(text))),
        other => Some(other),
    }
}

pub(super) fn build_generated_tool_call_id(index: usize) -> String {
    format!("call_auto_{index}")
}

pub(super) fn canonicalize_tool_arguments(value: Option<Value>) -> String {
    match value {
        Some(Value::String(text)) => text,
        Some(other) => serde_json::to_string(&other).unwrap_or_else(|_| "null".to_string()),
        None => "{}".to_string(),
    }
}
