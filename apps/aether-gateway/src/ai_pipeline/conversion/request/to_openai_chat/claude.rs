use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::shared::canonical_json_string;

pub(crate) fn normalize_claude_request_to_openai_chat_request(body_json: &Value) -> Option<Value> {
    let request = body_json.as_object()?;
    let mut output = Map::new();
    if let Some(model) = request.get("model") {
        output.insert("model".to_string(), model.clone());
    }

    let mut messages = Vec::new();
    if let Some(system_text) = extract_claude_system_text(request.get("system")) {
        if !system_text.trim().is_empty() {
            messages.push(json!({
                "role": "system",
                "content": system_text,
            }));
        }
    }

    if let Some(message_values) = request.get("messages").and_then(Value::as_array) {
        for message in message_values {
            let message_object = message.as_object()?;
            let role = message_object
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            match role.as_str() {
                "user" => {
                    let mut text_segments = Vec::new();
                    if let Some(content) = message_object.get("content") {
                        for block in normalize_claude_content_blocks(content)? {
                            match block {
                                ClaudeNormalizedBlock::Text(text) => {
                                    if !text.trim().is_empty() {
                                        text_segments.push(text);
                                    }
                                }
                                ClaudeNormalizedBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                } => {
                                    messages.push(json!({
                                        "role": "tool",
                                        "tool_call_id": tool_use_id,
                                        "content": content,
                                    }));
                                }
                                ClaudeNormalizedBlock::ToolUse { .. } => {}
                            }
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
                "assistant" => {
                    let mut text_segments = Vec::new();
                    let mut tool_calls = Vec::new();
                    if let Some(content) = message_object.get("content") {
                        for block in normalize_claude_content_blocks(content)? {
                            match block {
                                ClaudeNormalizedBlock::Text(text) => {
                                    if !text.trim().is_empty() {
                                        text_segments.push(text);
                                    }
                                }
                                ClaudeNormalizedBlock::ToolUse { id, name, input } => {
                                    tool_calls.push(json!({
                                        "id": id.unwrap_or_else(|| format!("toolu_{}", Uuid::new_v4().simple())),
                                        "type": "function",
                                        "function": {
                                            "name": name,
                                            "arguments": canonical_json_string(input.unwrap_or(Value::Object(Map::new()))),
                                        }
                                    }));
                                }
                                ClaudeNormalizedBlock::ToolResult { .. } => {}
                            }
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
                _ => {}
            }
        }
    }
    output.insert("messages".to_string(), Value::Array(messages));

    if let Some(max_tokens) = request.get("max_tokens").cloned() {
        output.insert("max_completion_tokens".to_string(), max_tokens);
    }
    for passthrough_key in ["temperature", "top_p", "metadata", "stop", "stream"] {
        if let Some(value) = request.get(passthrough_key) {
            output.insert(passthrough_key.to_string(), value.clone());
        }
    }
    if let Some(tools) = normalize_claude_tools_to_openai(request.get("tools"))? {
        output.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(tool_choice) = normalize_claude_tool_choice_to_openai(request.get("tool_choice"))? {
        output.insert("tool_choice".to_string(), tool_choice);
    }

    Some(Value::Object(output))
}

#[derive(Debug)]
enum ClaudeNormalizedBlock {
    Text(String),
    ToolUse {
        id: Option<String>,
        name: String,
        input: Option<Value>,
    },
    ToolResult {
        tool_use_id: String,
        content: Value,
    },
}

fn normalize_claude_content_blocks(content: &Value) -> Option<Vec<ClaudeNormalizedBlock>> {
    match content {
        Value::String(text) => Some(vec![ClaudeNormalizedBlock::Text(text.clone())]),
        Value::Array(blocks) => {
            let mut normalized = Vec::new();
            for block in blocks {
                let block = block.as_object()?;
                match block.get("type")?.as_str()? {
                    "text" | "thinking" => {
                        let text = block
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        normalized.push(ClaudeNormalizedBlock::Text(text.to_string()));
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())?
                            .to_string();
                        normalized.push(ClaudeNormalizedBlock::ToolUse {
                            id: block
                                .get("id")
                                .and_then(Value::as_str)
                                .filter(|value| !value.is_empty())
                                .map(ToOwned::to_owned),
                            name,
                            input: block.get("input").cloned(),
                        });
                    }
                    "tool_result" => {
                        let tool_use_id = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())?
                            .to_string();
                        let content = block.get("content").cloned().unwrap_or(Value::Null);
                        normalized.push(ClaudeNormalizedBlock::ToolResult {
                            tool_use_id,
                            content,
                        });
                    }
                    _ => {}
                }
            }
            Some(normalized)
        }
        _ => None,
    }
}

fn extract_claude_system_text(system: Option<&Value>) -> Option<String> {
    let system = system?;
    let text = match system {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => {
            let mut segments = Vec::new();
            for block in blocks {
                let block = block.as_object()?;
                if block.get("type").and_then(Value::as_str).unwrap_or("text") == "text" {
                    let text = block
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !text.trim().is_empty() {
                        segments.push(text.to_string());
                    }
                }
            }
            segments.join("\n\n")
        }
        _ => return None,
    };
    Some(strip_claude_billing_header(&text))
}

fn strip_claude_billing_header(text: &str) -> String {
    let trimmed = text.trim();
    let prefix = "x-anthropic-billing-header:";
    if !trimmed.to_ascii_lowercase().starts_with(prefix) {
        return trimmed.to_string();
    }
    let remainder = trimmed
        .split_once('\n')
        .map(|(_, rest)| rest.trim_start())
        .unwrap_or_default();
    remainder.trim_start_matches('\n').trim().to_string()
}

fn normalize_claude_tools_to_openai(tools: Option<&Value>) -> Option<Option<Vec<Value>>> {
    let Some(tools) = tools else {
        return Some(None);
    };
    let tools = tools.as_array()?;
    let mut normalized = Vec::new();
    for tool in tools {
        let tool = tool.as_object()?;
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let mut function = Map::new();
        function.insert("name".to_string(), Value::String(name.to_string()));
        if let Some(description) = tool.get("description").and_then(Value::as_str) {
            if !description.trim().is_empty() {
                function.insert(
                    "description".to_string(),
                    Value::String(description.trim().to_string()),
                );
            }
        }
        function.insert(
            "parameters".to_string(),
            tool.get("input_schema")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"})),
        );
        normalized.push(json!({
            "type": "function",
            "function": Value::Object(function),
        }));
    }
    Some(Some(normalized))
}

fn normalize_claude_tool_choice_to_openai(tool_choice: Option<&Value>) -> Option<Option<Value>> {
    let Some(tool_choice) = tool_choice else {
        return Some(None);
    };
    match tool_choice {
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Some(Value::String("auto".to_string()))),
            "any" => Some(Some(Value::String("required".to_string()))),
            "none" => Some(Some(Value::String("none".to_string()))),
            _ => Some(None),
        },
        Value::Object(value) => {
            if let Some(name) = value.get("name").and_then(Value::as_str) {
                return Some(Some(json!({
                    "type": "function",
                    "function": { "name": name }
                })));
            }
            let kind = value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match kind.trim().to_ascii_lowercase().as_str() {
                "auto" => Some(Some(Value::String("auto".to_string()))),
                "any" => Some(Some(Value::String("required".to_string()))),
                "none" => Some(Some(Value::String("none".to_string()))),
                "tool" => value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|name| {
                        Some(json!({
                            "type": "function",
                            "function": { "name": name }
                        }))
                    })
                    .or(Some(None)),
                _ => Some(None),
            }
        }
        _ => Some(None),
    }
}
