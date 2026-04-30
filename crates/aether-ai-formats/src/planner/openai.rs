use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoReasoningEffortModel {
    pub base_model: String,
    pub reasoning_effort: String,
}

pub fn split_auto_reasoning_effort_model(model: &str) -> Option<AutoReasoningEffortModel> {
    let model = model.trim();
    let (base_model, suffix) = model.rsplit_once('-')?;
    let base_model = base_model.trim();
    if base_model.is_empty() {
        return None;
    }

    let reasoning_effort = suffix.trim().to_ascii_lowercase();
    if !is_auto_reasoning_effort(&reasoning_effort) {
        return None;
    }

    Some(AutoReasoningEffortModel {
        base_model: base_model.to_string(),
        reasoning_effort,
    })
}

pub fn auto_reasoning_effort_base_model(model: &str) -> Option<String> {
    split_auto_reasoning_effort_model(model).map(|parsed| parsed.base_model)
}

pub fn normalize_auto_reasoning_effort_model(model: &str) -> String {
    split_auto_reasoning_effort_model(model)
        .map(|parsed| parsed.base_model)
        .unwrap_or_else(|| model.trim().to_string())
}

pub fn is_auto_reasoning_effort(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "low" | "medium" | "high" | "xhigh" | "max"
    )
}

pub fn parse_openai_stop_sequences(stop: Option<&Value>) -> Option<Vec<Value>> {
    match stop {
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Some(vec![Value::String(value.clone())])
        }
        Some(Value::Array(values)) => Some(
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| Value::String(value.to_string()))
                .collect::<Vec<_>>(),
        )
        .filter(|values| !values.is_empty()),
        _ => None,
    }
}

pub fn resolve_openai_chat_max_tokens(request: &Map<String, Value>) -> u64 {
    request
        .get("max_completion_tokens")
        .and_then(value_as_u64)
        .or_else(|| request.get("max_tokens").and_then(value_as_u64))
        .unwrap_or(4096)
}

pub fn value_as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
}

pub fn copy_request_number_field(
    request: &Map<String, Value>,
    target: &mut Map<String, Value>,
    key: &str,
) {
    copy_request_number_field_as(request, target, key, key);
}

pub fn copy_request_number_field_as(
    request: &Map<String, Value>,
    target: &mut Map<String, Value>,
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = request.get(source_key).cloned() {
        if value.is_number() {
            target.insert(target_key.to_string(), value);
        }
    }
}

pub fn map_openai_reasoning_effort_to_claude_output(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "max" => Some("max"),
        _ => None,
    }
}

pub fn claude_model_uses_adaptive_effort(model: &str) -> bool {
    let model = model
        .trim()
        .to_ascii_lowercase()
        .replace('.', "-")
        .replace('_', "-");
    model.contains("mythos")
        || model.contains("opus-4-7")
        || model.contains("opus-4-6")
        || model.contains("sonnet-4-6")
}

pub fn map_openai_reasoning_effort_to_thinking_budget(value: &str) -> Option<u64> {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => Some(1280),
        "medium" => Some(2048),
        "high" => Some(4096),
        "xhigh" | "max" => Some(8192),
        _ => None,
    }
}

pub fn map_openai_reasoning_effort_to_gemini_budget(value: &str) -> Option<u64> {
    map_openai_reasoning_effort_to_thinking_budget(value)
}

pub fn map_openai_reasoning_effort_to_gemini_level(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" | "xhigh" | "max" => Some("high"),
        _ => None,
    }
}

pub fn gemini_model_uses_thinking_level(model: &str) -> bool {
    model
        .trim()
        .to_ascii_lowercase()
        .split('/')
        .any(|part| part.starts_with("gemini-3"))
}

pub fn map_thinking_budget_to_openai_reasoning_effort(value: u64) -> &'static str {
    match value {
        0..=1664 => "low",
        1665..=3072 => "medium",
        3073..=6144 => "high",
        _ => "xhigh",
    }
}

pub fn extract_openai_reasoning_effort(request: &Map<String, Value>) -> Option<String> {
    request
        .get("reasoning_effort")
        .and_then(Value::as_str)
        .or_else(|| {
            request
                .get("reasoning")
                .and_then(Value::as_object)
                .and_then(|reasoning| reasoning.get("effort"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{
        claude_model_uses_adaptive_effort, gemini_model_uses_thinking_level,
        map_openai_reasoning_effort_to_claude_output, map_openai_reasoning_effort_to_gemini_level,
        normalize_auto_reasoning_effort_model, split_auto_reasoning_effort_model,
        AutoReasoningEffortModel,
    };

    #[test]
    fn splits_supported_auto_reasoning_effort_suffixes() {
        assert_eq!(
            split_auto_reasoning_effort_model("gpt-5.4-xhigh"),
            Some(AutoReasoningEffortModel {
                base_model: "gpt-5.4".to_string(),
                reasoning_effort: "xhigh".to_string(),
            })
        );
        assert_eq!(
            split_auto_reasoning_effort_model("gpt-5.4-MAX"),
            Some(AutoReasoningEffortModel {
                base_model: "gpt-5.4".to_string(),
                reasoning_effort: "max".to_string(),
            })
        );
    }

    #[test]
    fn ignores_models_without_supported_auto_reasoning_suffix() {
        assert_eq!(split_auto_reasoning_effort_model("gpt-5.4-ultra"), None);
        assert_eq!(split_auto_reasoning_effort_model("gpt-5.4"), None);
        assert_eq!(split_auto_reasoning_effort_model("-high"), None);
        assert_eq!(
            normalize_auto_reasoning_effort_model("gpt-5.4-ultra"),
            "gpt-5.4-ultra"
        );
    }

    #[test]
    fn maps_provider_specific_reasoning_effort_values() {
        assert_eq!(
            map_openai_reasoning_effort_to_claude_output("xhigh"),
            Some("xhigh")
        );
        assert_eq!(
            map_openai_reasoning_effort_to_claude_output("max"),
            Some("max")
        );
        assert_eq!(
            map_openai_reasoning_effort_to_gemini_level("xhigh"),
            Some("high")
        );
        assert_eq!(
            map_openai_reasoning_effort_to_gemini_level("max"),
            Some("high")
        );
        assert!(gemini_model_uses_thinking_level("models/gemini-3-pro"));
        assert!(!gemini_model_uses_thinking_level("gemini-2.5-pro"));
        assert!(claude_model_uses_adaptive_effort("claude-opus-4.7"));
        assert!(claude_model_uses_adaptive_effort("claude-sonnet-4-6"));
        assert!(!claude_model_uses_adaptive_effort("claude-opus-4-5"));
    }
}
