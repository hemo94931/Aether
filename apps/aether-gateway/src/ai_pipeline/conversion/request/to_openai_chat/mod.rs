mod claude;
mod gemini;
mod openai_cli;
mod shared;

pub(crate) use claude::normalize_claude_request_to_openai_chat_request;
pub(crate) use gemini::normalize_gemini_request_to_openai_chat_request;
pub(crate) use openai_cli::normalize_openai_cli_request_to_openai_chat_request;
pub(crate) use shared::{extract_openai_text_content, parse_openai_tool_result_content};
