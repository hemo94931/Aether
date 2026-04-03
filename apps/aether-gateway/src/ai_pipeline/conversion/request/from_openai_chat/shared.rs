use serde_json::{json, Value};

pub(super) fn parse_openai_tool_arguments(arguments: Option<&Value>) -> Option<Value> {
    match arguments {
        Some(Value::Object(object)) => Some(Value::Object(object.clone())),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Some(json!({}))
            } else {
                match serde_json::from_str::<Value>(trimmed) {
                    Ok(Value::Object(object)) => Some(Value::Object(object)),
                    Ok(other) => Some(json!({ "input": other })),
                    Err(_) => Some(json!({ "input": trimmed })),
                }
            }
        }
        Some(other) => Some(json!({ "input": other })),
        None => Some(json!({})),
    }
}
