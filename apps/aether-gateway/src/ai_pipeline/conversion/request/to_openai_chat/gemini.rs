use serde_json::{json, Map, Value};

use super::shared::canonical_json_string;

pub(crate) fn normalize_gemini_request_to_openai_chat_request(
    body_json: &Value,
    request_path: &str,
) -> Option<Value> {
    let request = body_json.as_object()?;
    let mut output = Map::new();
    if let Some(model) = request
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        output.insert("model".to_string(), Value::String(model.to_string()));
    } else if let Some(model) = extract_gemini_model_from_path(request_path) {
        output.insert("model".to_string(), Value::String(model));
    }

    let mut messages = Vec::new();
    if let Some(system_text) = extract_gemini_system_text(
        request
            .get("systemInstruction")
            .or_else(|| request.get("system_instruction")),
    ) {
        if !system_text.trim().is_empty() {
            messages.push(json!({
                "role": "system",
                "content": system_text,
            }));
        }
    }

    if let Some(contents) = request.get("contents").and_then(Value::as_array) {
        for content in contents {
            let content_object = content.as_object()?;
            let role = content_object
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user")
                .trim()
                .to_ascii_lowercase();
            let parts = content_object.get("parts").and_then(Value::as_array)?;
            match role.as_str() {
                "model" => {
                    let mut text_segments = Vec::new();
                    let mut tool_calls = Vec::new();
                    for (index, part) in parts.iter().enumerate() {
                        let part = part.as_object()?;
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            if !text.trim().is_empty() {
                                text_segments.push(text.to_string());
                            }
                        } else if let Some(function_call) =
                            part.get("functionCall").and_then(Value::as_object)
                        {
                            let name = function_call
                                .get("name")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())?;
                            let id = function_call
                                .get("id")
                                .and_then(Value::as_str)
                                .filter(|value| !value.is_empty())
                                .map(ToOwned::to_owned)
                                .unwrap_or_else(|| format!("toolu_{}_{}", name, index));
                            tool_calls.push(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": canonical_json_string(function_call.get("args").cloned().unwrap_or(Value::Object(Map::new()))),
                                }
                            }));
                        }
                    }
                    let mut assistant = Map::new();
                    assistant.insert("role".to_string(), Value::String("assistant".to_string()));
                    assistant.insert(
                        "content".to_string(),
                        if text_segments.is_empty() && !tool_calls.is_empty() {
                            Value::Null
                        } else {
                            Value::String(text_segments.join("\n\n"))
                        },
                    );
                    if !tool_calls.is_empty() {
                        assistant.insert("tool_calls".to_string(), Value::Array(tool_calls));
                    }
                    messages.push(Value::Object(assistant));
                }
                _ => {
                    let mut text_segments = Vec::new();
                    for part in parts {
                        let part = part.as_object()?;
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            if !text.trim().is_empty() {
                                text_segments.push(text.to_string());
                            }
                        } else if let Some(function_response) =
                            part.get("functionResponse").and_then(Value::as_object)
                        {
                            let name = function_response
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("tool");
                            let response_value = function_response
                                .get("response")
                                .cloned()
                                .unwrap_or(Value::Object(Map::new()));
                            let tool_call_id = function_response
                                .get("id")
                                .and_then(Value::as_str)
                                .filter(|value| !value.is_empty())
                                .map(ToOwned::to_owned)
                                .unwrap_or_else(|| format!("toolu_{}", name));
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_call_id,
                                "content": response_value,
                            }));
                        }
                    }
                    let text = text_segments.join("\n\n");
                    if !text.trim().is_empty() {
                        messages.push(json!({
                            "role": "user",
                            "content": text,
                        }));
                    }
                }
            }
        }
    }
    output.insert("messages".to_string(), Value::Array(messages));

    let generation_config = request
        .get("generationConfig")
        .or_else(|| request.get("generation_config"))
        .and_then(Value::as_object);
    if let Some(generation_config) = generation_config {
        if let Some(value) = generation_config.get("maxOutputTokens").cloned() {
            output.insert("max_completion_tokens".to_string(), value);
        }
        if let Some(value) = generation_config.get("temperature").cloned() {
            output.insert("temperature".to_string(), value);
        }
        if let Some(value) = generation_config.get("topP").cloned() {
            output.insert("top_p".to_string(), value);
        }
        if let Some(value) = generation_config.get("candidateCount").cloned() {
            output.insert("n".to_string(), value);
        }
        if let Some(value) = generation_config.get("stopSequences").cloned() {
            output.insert("stop".to_string(), value);
        }
    }
    if let Some(value) = request.get("stream").cloned() {
        output.insert("stream".to_string(), value);
    }
    if let Some(tools) = normalize_gemini_tools_to_openai(request.get("tools"))? {
        output.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(tool_choice) = normalize_gemini_tool_choice_to_openai(request.get("toolConfig"))? {
        output.insert("tool_choice".to_string(), tool_choice);
    }

    Some(Value::Object(output))
}

fn extract_gemini_system_text(system_instruction: Option<&Value>) -> Option<String> {
    let system_instruction = system_instruction?;
    match system_instruction {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Object(object) => {
            let parts = object.get("parts")?.as_array()?;
            let mut segments = Vec::new();
            for part in parts {
                let part = part.as_object()?;
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        segments.push(text.to_string());
                    }
                }
            }
            Some(segments.join("\n\n"))
        }
        _ => None,
    }
}

fn normalize_gemini_tools_to_openai(tools: Option<&Value>) -> Option<Option<Vec<Value>>> {
    let Some(tools) = tools else {
        return Some(None);
    };
    let tools = tools.as_array()?;
    let mut normalized = Vec::new();
    for tool in tools {
        let tool = tool.as_object()?;
        let declarations = tool
            .get("functionDeclarations")
            .or_else(|| tool.get("function_declarations"))
            .and_then(Value::as_array);
        let Some(declarations) = declarations else {
            continue;
        };
        for declaration in declarations {
            let declaration = declaration.as_object()?;
            let name = declaration
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let mut function = Map::new();
            function.insert("name".to_string(), Value::String(name.to_string()));
            if let Some(description) = declaration.get("description").and_then(Value::as_str) {
                if !description.trim().is_empty() {
                    function.insert(
                        "description".to_string(),
                        Value::String(description.trim().to_string()),
                    );
                }
            }
            function.insert(
                "parameters".to_string(),
                declaration
                    .get("parameters")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object"})),
            );
            normalized.push(json!({
                "type": "function",
                "function": Value::Object(function),
            }));
        }
    }
    Some(Some(normalized))
}

fn normalize_gemini_tool_choice_to_openai(tool_config: Option<&Value>) -> Option<Option<Value>> {
    let Some(tool_config) = tool_config else {
        return Some(None);
    };
    let tool_config = tool_config.as_object()?;
    let function_config = tool_config
        .get("functionCallingConfig")
        .or_else(|| tool_config.get("function_calling_config"))
        .and_then(Value::as_object)?;
    let mode = function_config
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase();
    match mode.as_str() {
        "NONE" => Some(Some(Value::String("none".to_string()))),
        "AUTO" => Some(Some(Value::String("auto".to_string()))),
        "ANY" | "REQUIRED" => Some(Some(Value::String("required".to_string()))),
        _ => {
            if let Some(name) = function_config
                .get("allowedFunctionNames")
                .or_else(|| function_config.get("allowed_function_names"))
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_str)
            {
                Some(Some(json!({
                    "type": "function",
                    "function": { "name": name }
                })))
            } else {
                Some(None)
            }
        }
    }
}

fn extract_gemini_model_from_path(path: &str) -> Option<String> {
    let marker = "/models/";
    let start = path.find(marker)? + marker.len();
    let tail = &path[start..];
    let end = tail.find(':').unwrap_or(tail.len());
    let model = tail[..end].trim();
    if model.is_empty() {
        None
    } else {
        Some(model.to_string())
    }
}
