use aether_contracts::{ExecutionStreamTerminalSummary, StandardizedUsage};
use serde_json::{json, Value};

use crate::ai_pipeline::{
    convert_claude_cli_response_to_openai_cli, convert_gemini_cli_response_to_openai_cli,
    convert_openai_chat_response_to_openai_cli, ClaudeClientEmitter, GeminiClientEmitter,
    OpenAIChatClientEmitter, OpenAICliClientEmitter, OpenAICliProviderState,
};
use crate::GatewayError;

pub(crate) struct SyncToStreamBridgeOutcome {
    pub(crate) sse_body: Vec<u8>,
    pub(crate) terminal_summary: Option<ExecutionStreamTerminalSummary>,
}

pub(crate) fn maybe_bridge_standard_sync_json_to_stream(
    provider_body_json: &Value,
    provider_api_format: &str,
    client_api_format: &str,
    report_context: Option<&Value>,
) -> Result<Option<SyncToStreamBridgeOutcome>, GatewayError> {
    let provider_api_format = normalize_api_format(provider_api_format);
    let client_api_format = normalize_api_format(client_api_format);
    if !is_standard_api_format(provider_api_format.as_str())
        || !is_standard_api_format(client_api_format.as_str())
    {
        return Ok(None);
    }

    let bridge_context = build_bridge_report_context(
        report_context,
        provider_api_format.as_str(),
        client_api_format.as_str(),
    );
    let Some(openai_cli_response) = convert_provider_sync_response_to_openai_cli(
        provider_body_json,
        provider_api_format.as_str(),
        &bridge_context,
    ) else {
        return Ok(None);
    };
    let terminal_summary = build_terminal_summary_from_openai_cli_response(&openai_cli_response);
    let canonical_frames =
        build_canonical_frames_from_openai_cli_response(&openai_cli_response, &bridge_context)?;
    let sse_body =
        emit_client_stream_from_canonical_frames(canonical_frames, client_api_format.as_str())?;

    Ok(Some(SyncToStreamBridgeOutcome {
        sse_body,
        terminal_summary,
    }))
}

fn normalize_api_format(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn is_standard_api_format(value: &str) -> bool {
    matches!(
        value,
        "openai:chat"
            | "openai:cli"
            | "openai:compact"
            | "claude:chat"
            | "claude:cli"
            | "gemini:chat"
            | "gemini:cli"
    )
}

fn build_bridge_report_context(
    report_context: Option<&Value>,
    provider_api_format: &str,
    client_api_format: &str,
) -> Value {
    let mut context = report_context
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let object = context
        .as_object_mut()
        .expect("bridge report context should stay object");
    object
        .entry("provider_api_format".to_string())
        .or_insert_with(|| Value::String(provider_api_format.to_string()));
    object
        .entry("client_api_format".to_string())
        .or_insert_with(|| Value::String(client_api_format.to_string()));
    context
}

fn convert_provider_sync_response_to_openai_cli(
    provider_body_json: &Value,
    provider_api_format: &str,
    report_context: &Value,
) -> Option<Value> {
    match provider_api_format {
        "openai:cli" | "openai:compact" => Some(provider_body_json.clone()),
        "openai:chat" => {
            convert_openai_chat_response_to_openai_cli(provider_body_json, report_context, false)
        }
        "claude:chat" | "claude:cli" => {
            convert_claude_cli_response_to_openai_cli(provider_body_json, report_context)
        }
        "gemini:chat" | "gemini:cli" => {
            convert_gemini_cli_response_to_openai_cli(provider_body_json, report_context)
        }
        _ => None,
    }
}

fn build_canonical_frames_from_openai_cli_response(
    openai_cli_response: &Value,
    report_context: &Value,
) -> Result<Vec<crate::ai_pipeline::CanonicalStreamFrame>, GatewayError> {
    let mut state = OpenAICliProviderState::default();
    let line = format!(
        "data: {}\n",
        serde_json::to_string(&json!({
            "type": "response.completed",
            "response": openai_cli_response,
        }))
        .map_err(|err| GatewayError::Internal(err.to_string()))?
    );
    let mut frames = state
        .push_line(report_context, line.into_bytes())
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    frames.extend(
        state
            .finish(report_context)
            .map_err(|err| GatewayError::Internal(err.to_string()))?,
    );
    Ok(frames)
}

fn emit_client_stream_from_canonical_frames(
    canonical_frames: Vec<crate::ai_pipeline::CanonicalStreamFrame>,
    client_api_format: &str,
) -> Result<Vec<u8>, GatewayError> {
    match client_api_format {
        "openai:chat" => {
            let mut emitter = OpenAIChatClientEmitter::default();
            emit_with_openai_chat_emitter(&mut emitter, canonical_frames)
        }
        "openai:cli" | "openai:compact" => {
            let mut emitter = OpenAICliClientEmitter::default();
            emit_with_openai_cli_emitter(&mut emitter, canonical_frames)
        }
        "claude:chat" | "claude:cli" => {
            let mut emitter = ClaudeClientEmitter::default();
            emit_with_claude_emitter(&mut emitter, canonical_frames)
        }
        "gemini:chat" | "gemini:cli" => {
            let mut emitter = GeminiClientEmitter::default();
            emit_with_gemini_emitter(&mut emitter, canonical_frames)
        }
        _ => Ok(Vec::new()),
    }
}

fn emit_with_openai_chat_emitter(
    emitter: &mut OpenAIChatClientEmitter,
    canonical_frames: Vec<crate::ai_pipeline::CanonicalStreamFrame>,
) -> Result<Vec<u8>, GatewayError> {
    let mut output = Vec::new();
    for frame in canonical_frames {
        output.extend(
            emitter
                .emit(frame)
                .map_err(|err| GatewayError::Internal(err.to_string()))?,
        );
    }
    output.extend(
        emitter
            .finish()
            .map_err(|err| GatewayError::Internal(err.to_string()))?,
    );
    Ok(output)
}

fn emit_with_openai_cli_emitter(
    emitter: &mut OpenAICliClientEmitter,
    canonical_frames: Vec<crate::ai_pipeline::CanonicalStreamFrame>,
) -> Result<Vec<u8>, GatewayError> {
    let mut output = Vec::new();
    for frame in canonical_frames {
        output.extend(
            emitter
                .emit(frame)
                .map_err(|err| GatewayError::Internal(err.to_string()))?,
        );
    }
    output.extend(
        emitter
            .finish()
            .map_err(|err| GatewayError::Internal(err.to_string()))?,
    );
    Ok(output)
}

fn emit_with_claude_emitter(
    emitter: &mut ClaudeClientEmitter,
    canonical_frames: Vec<crate::ai_pipeline::CanonicalStreamFrame>,
) -> Result<Vec<u8>, GatewayError> {
    let mut output = Vec::new();
    for frame in canonical_frames {
        output.extend(
            emitter
                .emit(frame)
                .map_err(|err| GatewayError::Internal(err.to_string()))?,
        );
    }
    output.extend(
        emitter
            .finish()
            .map_err(|err| GatewayError::Internal(err.to_string()))?,
    );
    Ok(output)
}

fn emit_with_gemini_emitter(
    emitter: &mut GeminiClientEmitter,
    canonical_frames: Vec<crate::ai_pipeline::CanonicalStreamFrame>,
) -> Result<Vec<u8>, GatewayError> {
    let mut output = Vec::new();
    for frame in canonical_frames {
        output.extend(
            emitter
                .emit(frame)
                .map_err(|err| GatewayError::Internal(err.to_string()))?,
        );
    }
    output.extend(
        emitter
            .finish()
            .map_err(|err| GatewayError::Internal(err.to_string()))?,
    );
    Ok(output)
}

fn build_terminal_summary_from_openai_cli_response(
    openai_cli_response: &Value,
) -> Option<ExecutionStreamTerminalSummary> {
    let response = openai_cli_response.as_object()?;
    let response_id = response
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let model = response
        .get("model")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let finish_reason = response
        .get("output")
        .and_then(Value::as_array)
        .map(|output| resolve_openai_cli_finish_reason(output))
        .filter(|value| !value.trim().is_empty());
    let standardized_usage = response
        .get("usage")
        .and_then(standardized_usage_from_openai_usage);
    Some(ExecutionStreamTerminalSummary {
        standardized_usage,
        finish_reason,
        response_id,
        model,
        observed_finish: true,
        parser_error: None,
    })
}

fn resolve_openai_cli_finish_reason(output: &[Value]) -> String {
    let has_tool_calls = output.iter().filter_map(Value::as_object).any(|item| {
        item.get("type")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "function_call")
    });
    if has_tool_calls {
        "tool_calls".to_string()
    } else {
        "stop".to_string()
    }
}

fn standardized_usage_from_openai_usage(value: &Value) -> Option<StandardizedUsage> {
    let usage = value.as_object()?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let cache_creation_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_i64)
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .or_else(|| usage.get("prompt_tokens_details"))
                .and_then(Value::as_object)
                .and_then(|details| details.get("cached_creation_tokens"))
                .and_then(Value::as_i64)
        })
        .unwrap_or(0);
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_i64)
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .or_else(|| usage.get("prompt_tokens_details"))
                .and_then(Value::as_object)
                .and_then(|details| details.get("cached_tokens"))
                .and_then(Value::as_i64)
        })
        .unwrap_or(0);
    let total_tokens = usage.get("total_tokens").and_then(Value::as_i64).unwrap_or(
        input_tokens
            .saturating_add(output_tokens)
            .saturating_add(cache_creation_tokens)
            .saturating_add(cache_read_tokens),
    );
    let mut standardized_usage = StandardizedUsage::new();
    standardized_usage.input_tokens = input_tokens;
    standardized_usage.output_tokens = output_tokens;
    standardized_usage.cache_creation_tokens = cache_creation_tokens;
    standardized_usage.cache_read_tokens = cache_read_tokens;
    standardized_usage
        .dimensions
        .insert("total_tokens".to_string(), json!(total_tokens));
    Some(standardized_usage.normalize_cache_creation_breakdown())
}
