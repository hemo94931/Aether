use serde_json::{json, Map, Value};

use super::shared::{extract_openai_text_content, parse_openai_tool_result_content};

pub(crate) fn normalize_openai_cli_request_to_openai_chat_request(
    body_json: &Value,
) -> Option<Value> {
    let request = body_json.as_object()?;
    let mut output = Map::new();
    if let Some(model) = request.get("model") {
        output.insert("model".to_string(), model.clone());
    }

    let mut messages = Vec::new();
    if let Some(instructions) = request.get("instructions") {
        let text = extract_openai_text_content(Some(instructions))?;
        if !text.trim().is_empty() {
            messages.push(json!({
                "role": "system",
                "content": text,
            }));
        }
    }
    messages.extend(normalize_openai_cli_input_to_openai_chat_messages(
        request.get("input"),
    )?);
    output.insert("messages".to_string(), Value::Array(messages));

    if let Some(max_output_tokens) = request.get("max_output_tokens").cloned() {
        output.insert("max_completion_tokens".to_string(), max_output_tokens);
    }
    for passthrough_key in [
        "temperature",
        "top_p",
        "metadata",
        "store",
        "previous_response_id",
        "service_tier",
        "reasoning",
        "stop",
        "stream",
    ] {
        if let Some(value) = request.get(passthrough_key) {
            output.insert(passthrough_key.to_string(), value.clone());
        }
    }
    if let Some(tools) = normalize_openai_cli_tools_to_openai_chat(request.get("tools"))? {
        output.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(tool_choice) =
        normalize_openai_cli_tool_choice_to_openai_chat(request.get("tool_choice"))?
    {
        output.insert("tool_choice".to_string(), tool_choice);
    }

    Some(Value::Object(output))
}

fn normalize_openai_cli_input_to_openai_chat_messages(input: Option<&Value>) -> Option<Vec<Value>> {
    let Some(input) = input else {
        return Some(Vec::new());
    };
    match input {
        Value::Null => Some(Vec::new()),
        Value::String(text) => {
            if text.trim().is_empty() {
                Some(Vec::new())
            } else {
                Some(vec![json!({
                    "role": "user",
                    "content": text,
                })])
            }
        }
        Value::Array(items) => {
            let mut messages = Vec::new();
            let mut next_generated_tool_call_index = 0usize;
            for item in items {
                if let Some(item_text) = item.as_str() {
                    if !item_text.trim().is_empty() {
                        messages.push(json!({
                            "role": "user",
                            "content": item_text,
                        }));
                    }
                    continue;
                }
                let item_object = item.as_object()?;
                let item_type = item_object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("message")
                    .trim()
                    .to_ascii_lowercase();
                match item_type.as_str() {
                    "message" => {
                        let role = item_object
                            .get("role")
                            .and_then(Value::as_str)
                            .unwrap_or("user")
                            .trim()
                            .to_ascii_lowercase();
                        if role == "system" || role == "developer" {
                            let text = extract_openai_text_content(item_object.get("content"))?;
                            if !text.trim().is_empty() {
                                messages.push(json!({
                                    "role": "system",
                                    "content": text,
                                }));
                            }
                            continue;
                        }
                        let normalized_content =
                            normalize_openai_cli_message_content(item_object.get("content"))?;
                        messages.push(json!({
                            "role": role,
                            "content": normalized_content,
                        }));
                    }
                    "function_call" => {
                        let tool_name = item_object
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())?;
                        let call_id = item_object
                            .get("call_id")
                            .or_else(|| item_object.get("id"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| {
                                let generated =
                                    format!("call_auto_{next_generated_tool_call_index}");
                                next_generated_tool_call_index += 1;
                                generated
                            });
                        let arguments = item_object
                            .get("arguments")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| "{}".to_string());
                        messages.push(json!({
                            "role": "assistant",
                            "content": Value::Array(Vec::new()),
                            "tool_calls": [{
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": tool_name,
                                    "arguments": arguments,
                                }
                            }]
                        }));
                    }
                    "function_call_output" => {
                        let tool_call_id = item_object
                            .get("call_id")
                            .or_else(|| item_object.get("tool_call_id"))
                            .or_else(|| item_object.get("id"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| {
                                let generated =
                                    format!("call_auto_{next_generated_tool_call_index}");
                                next_generated_tool_call_index += 1;
                                generated
                            });
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_call_id,
                            "content": parse_openai_tool_result_content(item_object.get("output")),
                        }));
                    }
                    _ => {}
                }
            }
            Some(messages)
        }
        _ => None,
    }
}

fn normalize_openai_cli_message_content(content: Option<&Value>) -> Option<Value> {
    let Some(content) = content else {
        return Some(Value::Array(Vec::new()));
    };
    match content {
        Value::String(text) => Some(Value::String(text.clone())),
        Value::Array(parts) => {
            let mut normalized = Vec::new();
            for part in parts {
                let part_object = part.as_object()?;
                let part_type = part_object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase();
                match part_type.as_str() {
                    "input_text" | "output_text" | "text" => {
                        if let Some(text) = part_object.get("text").and_then(Value::as_str) {
                            normalized.push(json!({
                                "type": "text",
                                "text": text,
                            }));
                        }
                    }
                    "input_image" | "output_image" | "image_url" => {
                        let image_url = part_object
                            .get("image_url")
                            .and_then(|value| {
                                value.as_str().map(ToOwned::to_owned).or_else(|| {
                                    value
                                        .as_object()
                                        .and_then(|object| object.get("url"))
                                        .and_then(Value::as_str)
                                        .map(ToOwned::to_owned)
                                })
                            })
                            .or_else(|| {
                                part_object
                                    .get("url")
                                    .and_then(Value::as_str)
                                    .map(ToOwned::to_owned)
                            })?;
                        normalized.push(json!({
                            "type": "input_image",
                            "image_url": image_url,
                        }));
                    }
                    _ => {}
                }
            }
            Some(Value::Array(normalized))
        }
        _ => Some(content.clone()),
    }
}

fn normalize_openai_cli_tools_to_openai_chat(tools: Option<&Value>) -> Option<Option<Vec<Value>>> {
    let Some(Value::Array(tool_values)) = tools else {
        return Some(None);
    };
    let mut normalized = Vec::new();
    for tool in tool_values {
        let tool_object = tool.as_object()?;
        let tool_type = tool_object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("function")
            .trim()
            .to_ascii_lowercase();
        if tool_object.get("function").is_some() || tool_type != "function" {
            normalized.push(tool.clone());
            continue;
        }
        let name = tool_object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let mut function = Map::new();
        function.insert("name".to_string(), Value::String(name.to_string()));
        if let Some(description) = tool_object.get("description") {
            function.insert("description".to_string(), description.clone());
        }
        if let Some(parameters) = tool_object.get("parameters") {
            function.insert("parameters".to_string(), parameters.clone());
        }
        normalized.push(json!({
            "type": "function",
            "function": function,
        }));
    }
    Some((!normalized.is_empty()).then_some(normalized))
}

fn normalize_openai_cli_tool_choice_to_openai_chat(
    tool_choice: Option<&Value>,
) -> Option<Option<Value>> {
    let Some(tool_choice) = tool_choice else {
        return Some(None);
    };
    match tool_choice {
        Value::Object(object)
            if object.get("function").is_none()
                && object
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.eq_ignore_ascii_case("function")) =>
        {
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            Some(Some(json!({
                "type": "function",
                "function": {
                    "name": name,
                }
            })))
        }
        _ => Some(Some(tool_choice.clone())),
    }
}
