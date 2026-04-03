use serde_json::Value;

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
