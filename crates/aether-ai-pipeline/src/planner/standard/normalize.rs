use serde_json::{json, Value};

use crate::conversion::request::{
    convert_openai_chat_request_to_claude_request, convert_openai_chat_request_to_gemini_request,
    convert_openai_chat_request_to_openai_responses_request,
    normalize_openai_responses_request_to_openai_chat_request,
};
use crate::conversion::{request_conversion_kind, RequestConversionKind};
use crate::planner::openai::apply_auto_reasoning_effort_from_request_model;

pub fn build_local_openai_chat_request_body(
    body_json: &Value,
    mapped_model: &str,
    upstream_is_stream: bool,
) -> Option<Value> {
    let request_body_object = body_json.as_object()?;
    let mut provider_request_body = serde_json::Map::from_iter(
        request_body_object
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    provider_request_body.insert("model".to_string(), Value::String(mapped_model.to_string()));
    if upstream_is_stream {
        provider_request_body.insert("stream".to_string(), Value::Bool(true));
        match provider_request_body.get_mut("stream_options") {
            Some(Value::Object(stream_options)) => {
                stream_options.insert("include_usage".to_string(), Value::Bool(true));
            }
            _ => {
                provider_request_body.insert(
                    "stream_options".to_string(),
                    json!({
                        "include_usage": true,
                    }),
                );
            }
        }
    }
    Some(with_auto_reasoning_effort(
        Value::Object(provider_request_body),
        "openai:chat",
        body_json,
    ))
}

pub fn build_cross_format_openai_chat_request_body(
    body_json: &Value,
    mapped_model: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<Value> {
    let conversion_kind = request_conversion_kind("openai:chat", provider_api_format)?;
    match conversion_kind {
        RequestConversionKind::ToClaudeStandard => {
            let provider_request_body = convert_openai_chat_request_to_claude_request(
                body_json,
                mapped_model,
                upstream_is_stream,
            )?;
            Some(with_auto_reasoning_effort(
                provider_request_body,
                provider_api_format,
                body_json,
            ))
        }
        RequestConversionKind::ToGeminiStandard => {
            let provider_request_body = convert_openai_chat_request_to_gemini_request(
                body_json,
                mapped_model,
                upstream_is_stream,
            )?;
            Some(with_auto_reasoning_effort(
                provider_request_body,
                provider_api_format,
                body_json,
            ))
        }
        RequestConversionKind::ToOpenAiResponses => {
            let provider_request_body = convert_openai_chat_request_to_openai_responses_request(
                body_json,
                mapped_model,
                upstream_is_stream,
                false,
            )?;
            Some(with_auto_reasoning_effort(
                provider_request_body,
                provider_api_format,
                body_json,
            ))
        }
        _ => None,
    }
}

pub fn build_local_openai_responses_request_body(
    body_json: &Value,
    mapped_model: &str,
    require_streaming: bool,
) -> Option<Value> {
    let request_body_object = body_json.as_object()?;
    let mut provider_request_body = serde_json::Map::from_iter(
        request_body_object
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    provider_request_body.insert("model".to_string(), Value::String(mapped_model.to_string()));
    if require_streaming {
        provider_request_body.insert("stream".to_string(), Value::Bool(true));
    }
    Some(with_auto_reasoning_effort(
        Value::Object(provider_request_body),
        "openai:responses",
        body_json,
    ))
}

pub fn build_cross_format_openai_responses_request_body(
    body_json: &Value,
    mapped_model: &str,
    client_api_format: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<Value> {
    let chat_like_request = normalize_openai_responses_request_to_openai_chat_request(body_json)?;
    let conversion_kind = request_conversion_kind(client_api_format, provider_api_format)?;
    match conversion_kind {
        RequestConversionKind::ToOpenAIChat => {
            let provider_request_body = build_local_openai_chat_request_body(
                &chat_like_request,
                mapped_model,
                upstream_is_stream,
            )?;
            Some(with_auto_reasoning_effort(
                provider_request_body,
                provider_api_format,
                body_json,
            ))
        }
        RequestConversionKind::ToOpenAiResponses => {
            let provider_request_body = convert_openai_chat_request_to_openai_responses_request(
                &chat_like_request,
                mapped_model,
                upstream_is_stream,
                false,
            )?;
            Some(with_auto_reasoning_effort(
                provider_request_body,
                provider_api_format,
                body_json,
            ))
        }
        RequestConversionKind::ToClaudeStandard => {
            let provider_request_body = convert_openai_chat_request_to_claude_request(
                &chat_like_request,
                mapped_model,
                upstream_is_stream,
            )?;
            Some(with_auto_reasoning_effort(
                provider_request_body,
                provider_api_format,
                body_json,
            ))
        }
        RequestConversionKind::ToGeminiStandard => {
            let provider_request_body = convert_openai_chat_request_to_gemini_request(
                &chat_like_request,
                mapped_model,
                upstream_is_stream,
            )?;
            Some(with_auto_reasoning_effort(
                provider_request_body,
                provider_api_format,
                body_json,
            ))
        }
    }
}

fn with_auto_reasoning_effort(
    mut provider_request_body: Value,
    provider_api_format: &str,
    body_json: &Value,
) -> Value {
    apply_auto_reasoning_effort_from_request_model(
        &mut provider_request_body,
        provider_api_format,
        body_json,
        None,
    );
    provider_request_body
}

#[cfg(test)]
mod tests {
    use super::build_local_openai_responses_request_body;
    use super::{
        build_cross_format_openai_chat_request_body,
        build_cross_format_openai_responses_request_body, build_local_openai_chat_request_body,
    };
    use serde_json::{json, Value};

    fn object_keys(value: &Value) -> Vec<&str> {
        value
            .as_object()
            .expect("json object")
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn builds_openai_chat_cross_format_request_body_from_openai_responses_source() {
        let body_json = json!({
            "model": "gpt-5",
            "input": "hello",
        });

        let provider_request_body = build_cross_format_openai_responses_request_body(
            &body_json,
            "gpt-5-upstream",
            "openai:responses",
            "openai:chat",
            false,
        )
        .expect("openai responses to openai chat body should build");

        assert_eq!(provider_request_body["model"], "gpt-5-upstream");
        assert_eq!(provider_request_body["messages"][0]["role"], "user");
        assert_eq!(provider_request_body["messages"][0]["content"], "hello");
    }

    #[test]
    fn local_openai_responses_request_body_preserves_original_field_order() {
        let body_json: Value = serde_json::from_str(
            r#"{
                "model": "gpt-5",
                "include": ["reasoning.encrypted_content"],
                "input": [],
                "instructions": "Keep order"
            }"#,
        )
        .expect("request json should parse");

        let provider_request_body =
            build_local_openai_responses_request_body(&body_json, "gpt-5-upstream", false)
                .expect("openai responses body should build");

        assert_eq!(
            object_keys(&provider_request_body),
            vec!["model", "include", "input", "instructions"]
        );
        assert_eq!(provider_request_body["model"], "gpt-5-upstream");
    }

    #[test]
    fn builds_streaming_local_openai_chat_request_body_with_include_usage() {
        let body_json = json!({
            "model": "gpt-5",
            "messages": [{
                "role": "user",
                "content": "hello"
            }]
        });

        let provider_request_body =
            build_local_openai_chat_request_body(&body_json, "gpt-5-upstream", true)
                .expect("openai chat body should build");

        assert_eq!(provider_request_body["model"], "gpt-5-upstream");
        assert_eq!(provider_request_body["stream"], true);
        assert_eq!(
            provider_request_body["stream_options"]["include_usage"],
            true
        );
    }

    #[test]
    fn local_openai_chat_request_body_applies_reasoning_effort_suffix() {
        let body_json = json!({
            "model": "gpt-5.4-xhigh",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
            "reasoning_effort": "low"
        });

        let provider_request_body =
            build_local_openai_chat_request_body(&body_json, "gpt-5-upstream", false)
                .expect("openai chat body should build");

        assert_eq!(provider_request_body["model"], "gpt-5-upstream");
        assert_eq!(provider_request_body["reasoning_effort"], "xhigh");
    }

    #[test]
    fn local_openai_responses_request_body_applies_reasoning_effort_suffix() {
        let body_json = json!({
            "model": "gpt-5.4-max",
            "input": "hello",
            "reasoning": {"effort": "low", "summary": "auto"}
        });

        let provider_request_body =
            build_local_openai_responses_request_body(&body_json, "gpt-5-upstream", false)
                .expect("openai responses body should build");

        assert_eq!(provider_request_body["model"], "gpt-5-upstream");
        assert_eq!(provider_request_body["reasoning"]["summary"], "auto");
        assert_eq!(provider_request_body["reasoning"]["effort"], "xhigh");
    }

    #[test]
    fn cross_format_request_body_applies_reasoning_effort_suffix() {
        let body_json = json!({
            "model": "gpt-5.4-high",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
            "reasoning_effort": "low"
        });

        let provider_request_body = build_cross_format_openai_chat_request_body(
            &body_json,
            "claude-sonnet",
            "claude:messages",
            false,
        )
        .expect("claude body should build");

        assert_eq!(provider_request_body["model"], "claude-sonnet");
        assert_eq!(provider_request_body["output_config"]["effort"], "high");
        assert_eq!(provider_request_body["thinking"]["budget_tokens"], 4096);
    }

    #[test]
    fn streaming_local_openai_chat_request_body_preserves_stream_options_while_forcing_include_usage(
    ) {
        let body_json = json!({
            "model": "gpt-5",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
            "stream_options": {
                "include_usage": false,
                "extra": "keep-me"
            }
        });

        let provider_request_body =
            build_local_openai_chat_request_body(&body_json, "gpt-5-upstream", true)
                .expect("openai chat body should build");

        assert_eq!(
            provider_request_body["stream_options"]["include_usage"],
            true
        );
        assert_eq!(provider_request_body["stream_options"]["extra"], "keep-me");
    }
}
