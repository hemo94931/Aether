use serde_json::{json, Map, Value};

pub use aether_ai_formats::planner::openai::{
    auto_reasoning_effort_base_model, normalize_auto_reasoning_effort_model,
    split_auto_reasoning_effort_model, AutoReasoningEffortModel,
};

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

pub fn apply_auto_reasoning_effort_from_request_model(
    provider_request_body: &mut Value,
    provider_api_format: &str,
    request_body: &Value,
    request_path: Option<&str>,
) -> Option<AutoReasoningEffortModel> {
    let model = request_body
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| request_path.and_then(extract_gemini_model_from_path))?;

    apply_auto_reasoning_effort_from_model(provider_request_body, provider_api_format, &model)
}

pub fn apply_auto_reasoning_effort_from_model(
    provider_request_body: &mut Value,
    provider_api_format: &str,
    model: &str,
) -> Option<AutoReasoningEffortModel> {
    let parsed = split_auto_reasoning_effort_model(model)?;
    match aether_ai_formats::normalize_api_format_alias(provider_api_format).as_str() {
        "openai:chat" => {
            set_object_field(
                provider_request_body,
                "reasoning_effort",
                Value::String(parsed.reasoning_effort.clone()),
            )?;
        }
        "openai:responses" | "openai:responses:compact" => {
            set_openai_responses_reasoning_effort(
                provider_request_body,
                parsed.reasoning_effort.as_str(),
            )?;
        }
        "claude:messages" => {
            set_claude_reasoning_effort(
                provider_request_body,
                parsed.reasoning_effort.as_str(),
                parsed.base_model.as_str(),
            )?;
        }
        "gemini:generate_content" => {
            set_gemini_reasoning_effort(
                provider_request_body,
                parsed.reasoning_effort.as_str(),
                parsed.base_model.as_str(),
            )?;
        }
        _ => return None,
    }
    Some(parsed)
}

fn set_object_field(body: &mut Value, key: &str, value: Value) -> Option<()> {
    body.as_object_mut()?.insert(key.to_string(), value);
    Some(())
}

fn set_openai_responses_reasoning_effort(body: &mut Value, effort: &str) -> Option<()> {
    let body_object = body.as_object_mut()?;
    let reasoning = body_object
        .entry("reasoning".to_string())
        .or_insert_with(|| json!({}));
    if !reasoning.is_object() {
        *reasoning = json!({});
    }
    reasoning.as_object_mut()?.insert(
        "effort".to_string(),
        Value::String(openai_responses_effort(effort).to_string()),
    );
    Some(())
}

fn openai_responses_effort(effort: &str) -> &str {
    match effort.trim().to_ascii_lowercase().as_str() {
        "xhigh" | "max" => "xhigh",
        "low" => "low",
        "medium" => "medium",
        "high" => "high",
        _ => effort,
    }
}

fn set_claude_reasoning_effort(
    body: &mut Value,
    effort: &str,
    requested_model: &str,
) -> Option<()> {
    let body_object = body.as_object_mut()?;
    let provider_model = body_object
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let uses_adaptive = claude_model_uses_adaptive_effort(provider_model)
        || claude_model_uses_adaptive_effort(requested_model);
    let output_effort = map_openai_reasoning_effort_to_claude_output(effort)?;
    let output_config = body_object
        .entry("output_config".to_string())
        .or_insert_with(|| json!({}));
    if !output_config.is_object() {
        *output_config = json!({});
    }
    output_config.as_object_mut()?.insert(
        "effort".to_string(),
        Value::String(output_effort.to_string()),
    );

    let budget_tokens = map_openai_reasoning_effort_to_thinking_budget(effort)?;
    let thinking = body_object
        .entry("thinking".to_string())
        .or_insert_with(|| json!({}));
    if !thinking.is_object() {
        *thinking = json!({});
    }
    let thinking = thinking.as_object_mut()?;
    if uses_adaptive {
        thinking.insert("type".to_string(), Value::String("adaptive".to_string()));
        thinking.remove("budget_tokens");
    } else {
        thinking.insert("type".to_string(), Value::String("enabled".to_string()));
        thinking.insert("budget_tokens".to_string(), Value::from(budget_tokens));
    }
    Some(())
}

fn set_gemini_reasoning_effort(body: &mut Value, effort: &str, model: &str) -> Option<()> {
    let body_object = body.as_object_mut()?;
    let generation_key = if body_object.contains_key("generation_config")
        && !body_object.contains_key("generationConfig")
    {
        "generation_config"
    } else {
        "generationConfig"
    };
    let generation_config = body_object
        .entry(generation_key.to_string())
        .or_insert_with(|| json!({}));
    if !generation_config.is_object() {
        *generation_config = json!({});
    }
    let generation_config = generation_config.as_object_mut()?;
    let thinking_key = if generation_config.contains_key("thinking_config")
        && !generation_config.contains_key("thinkingConfig")
    {
        "thinking_config"
    } else {
        "thinkingConfig"
    };
    let thinking_config = gemini_reasoning_effort_config(effort, model, thinking_key)?;
    generation_config.insert(thinking_key.to_string(), thinking_config);
    Some(())
}

fn gemini_reasoning_effort_config(effort: &str, model: &str, thinking_key: &str) -> Option<Value> {
    if gemini_model_uses_thinking_level(model) {
        let thinking_level = map_openai_reasoning_effort_to_gemini_level(effort)?;
        return Some(if thinking_key == "thinking_config" {
            json!({
                "include_thoughts": true,
                "thinking_level": thinking_level,
            })
        } else {
            json!({
                "includeThoughts": true,
                "thinkingLevel": thinking_level,
            })
        });
    }

    let budget_tokens = map_openai_reasoning_effort_to_gemini_budget(effort)?;
    Some(if thinking_key == "thinking_config" {
        json!({
            "include_thoughts": true,
            "thinking_budget": budget_tokens,
        })
    } else {
        json!({
            "includeThoughts": true,
            "thinkingBudget": budget_tokens,
        })
    })
}

fn extract_gemini_model_from_path(path: &str) -> Option<String> {
    let (_, suffix) = path.split_once("/models/")?;
    let model = suffix
        .split_once(':')
        .map(|(value, _)| value)
        .unwrap_or(suffix)
        .trim();
    (!model.is_empty()).then(|| model.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        apply_auto_reasoning_effort_from_model, apply_auto_reasoning_effort_from_request_model,
    };

    #[test]
    fn applies_auto_reasoning_effort_to_openai_chat_body() {
        let mut body = json!({
            "model": "gpt-5-upstream",
            "messages": [],
            "reasoning_effort": "low"
        });

        let applied =
            apply_auto_reasoning_effort_from_model(&mut body, "openai:chat", "gpt-5.4-XHIGH")
                .expect("suffix should apply");

        assert_eq!(applied.base_model, "gpt-5.4");
        assert_eq!(body["model"], "gpt-5-upstream");
        assert_eq!(body["reasoning_effort"], "xhigh");
    }

    #[test]
    fn applies_auto_reasoning_effort_to_responses_body() {
        let mut body = json!({
            "model": "gpt-5-upstream",
            "input": [],
            "reasoning": {"summary": "auto"}
        });

        apply_auto_reasoning_effort_from_model(&mut body, "openai:responses", "gpt-5.4-max")
            .expect("suffix should apply");

        assert_eq!(body["reasoning"]["summary"], "auto");
        assert_eq!(body["reasoning"]["effort"], "xhigh");
    }

    #[test]
    fn applies_auto_reasoning_effort_to_claude_body() {
        let mut body = json!({
            "model": "claude-sonnet",
            "messages": [],
            "thinking": {"type": "disabled"},
            "output_config": {"other": true}
        });

        apply_auto_reasoning_effort_from_model(&mut body, "claude:messages", "claude-sonnet-max")
            .expect("suffix should apply");

        assert_eq!(body["output_config"]["other"], true);
        assert_eq!(body["output_config"]["effort"], "max");
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 8192);
    }

    #[test]
    fn preserves_xhigh_auto_reasoning_effort_for_claude_body() {
        let mut body = json!({
            "model": "claude-opus",
            "messages": []
        });

        apply_auto_reasoning_effort_from_model(&mut body, "claude:messages", "claude-opus-xhigh")
            .expect("suffix should apply");

        assert_eq!(body["output_config"]["effort"], "xhigh");
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 8192);
    }

    #[test]
    fn applies_auto_reasoning_effort_to_adaptive_claude_body() {
        let mut body = json!({
            "model": "claude-opus-4-7",
            "messages": [],
            "thinking": {"type": "enabled", "budget_tokens": 2048}
        });

        apply_auto_reasoning_effort_from_model(
            &mut body,
            "claude:messages",
            "claude-opus-4-7-xhigh",
        )
        .expect("suffix should apply");

        assert_eq!(body["output_config"]["effort"], "xhigh");
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert!(body["thinking"].get("budget_tokens").is_none());
    }

    #[test]
    fn applies_auto_reasoning_effort_to_gemini_body_from_request_path() {
        let mut body = json!({
            "contents": [],
            "generationConfig": {"temperature": 0.2}
        });
        let request = json!({"contents": []});

        apply_auto_reasoning_effort_from_request_model(
            &mut body,
            "gemini:generate_content",
            &request,
            Some("/v1beta/models/gemini-2.5-pro-medium:generateContent"),
        )
        .expect("suffix should apply");

        assert_eq!(body["generationConfig"]["temperature"], 0.2);
        assert_eq!(
            body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            2048
        );
    }

    #[test]
    fn applies_auto_reasoning_effort_to_gemini_3_body_as_thinking_level() {
        let mut body = json!({
            "contents": [],
            "generationConfig": {"temperature": 0.2}
        });

        apply_auto_reasoning_effort_from_model(
            &mut body,
            "gemini:generate_content",
            "gemini-3-pro-xhigh",
        )
        .expect("suffix should apply");

        assert_eq!(body["generationConfig"]["temperature"], 0.2);
        assert_eq!(
            body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            "high"
        );
        assert!(body["generationConfig"]["thinkingConfig"]
            .get("thinkingBudget")
            .is_none());
    }
}
