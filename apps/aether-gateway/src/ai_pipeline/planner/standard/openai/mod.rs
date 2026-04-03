mod chat;
mod cli;
mod normalize_chat;
mod normalize_cli;

pub(crate) use crate::gateway::ai_pipeline::conversion::request::{
    convert_openai_chat_request_to_claude_request, convert_openai_chat_request_to_gemini_request,
    convert_openai_chat_request_to_openai_cli_request, extract_openai_text_content,
    normalize_openai_cli_request_to_openai_chat_request, parse_openai_tool_result_content,
};
pub(crate) use self::chat::{
    copy_request_number_field, copy_request_number_field_as,
    map_openai_reasoning_effort_to_claude_output, map_openai_reasoning_effort_to_gemini_budget,
    maybe_build_stream_local_decision_payload, maybe_build_sync_local_decision_payload,
    maybe_execute_stream_via_local_decision, maybe_execute_sync_via_local_decision,
    parse_openai_stop_sequences, resolve_openai_chat_max_tokens, value_as_u64,
};
pub(crate) use self::cli::{
    maybe_build_stream_local_openai_cli_decision_payload,
    maybe_build_sync_local_openai_cli_decision_payload,
    maybe_execute_stream_via_local_openai_cli_decision,
    maybe_execute_sync_via_local_openai_cli_decision,
};
pub(crate) use self::normalize_chat::{
    build_cross_format_openai_chat_request_body, build_cross_format_openai_chat_upstream_url,
    build_local_openai_chat_request_body, build_local_openai_chat_upstream_url,
};
pub(crate) use self::normalize_cli::{
    build_cross_format_openai_cli_request_body, build_cross_format_openai_cli_upstream_url,
    build_local_openai_cli_request_body, build_local_openai_cli_upstream_url,
};
