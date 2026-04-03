use axum::body::Bytes;
use base64::Engine as _;

use crate::gateway::headers::is_json_request;

pub(crate) const GEMINI_FILES_GET_PLAN_KIND: &str = "gemini_files_get";
pub(crate) const GEMINI_FILES_UPLOAD_PLAN_KIND: &str = "gemini_files_upload";
pub(crate) const GEMINI_FILES_LIST_PLAN_KIND: &str = "gemini_files_list";
pub(crate) const GEMINI_FILES_DELETE_PLAN_KIND: &str = "gemini_files_delete";
pub(crate) const GEMINI_FILES_DOWNLOAD_PLAN_KIND: &str = "gemini_files_download";
pub(crate) const OPENAI_VIDEO_CONTENT_PLAN_KIND: &str = "openai_video_content";
pub(crate) const OPENAI_VIDEO_CANCEL_SYNC_PLAN_KIND: &str = "openai_video_cancel_sync";
pub(crate) const OPENAI_VIDEO_REMIX_SYNC_PLAN_KIND: &str = "openai_video_remix_sync";
pub(crate) const OPENAI_VIDEO_DELETE_SYNC_PLAN_KIND: &str = "openai_video_delete_sync";
pub(crate) const GEMINI_VIDEO_CREATE_SYNC_PLAN_KIND: &str = "gemini_video_create_sync";
pub(crate) const GEMINI_VIDEO_CANCEL_SYNC_PLAN_KIND: &str = "gemini_video_cancel_sync";
pub(crate) const OPENAI_CHAT_STREAM_PLAN_KIND: &str = "openai_chat_stream";
pub(crate) const CLAUDE_CHAT_STREAM_PLAN_KIND: &str = "claude_chat_stream";
pub(crate) const GEMINI_CHAT_STREAM_PLAN_KIND: &str = "gemini_chat_stream";
pub(crate) const OPENAI_CLI_STREAM_PLAN_KIND: &str = "openai_cli_stream";
pub(crate) const OPENAI_COMPACT_STREAM_PLAN_KIND: &str = "openai_compact_stream";
pub(crate) const CLAUDE_CLI_STREAM_PLAN_KIND: &str = "claude_cli_stream";
pub(crate) const GEMINI_CLI_STREAM_PLAN_KIND: &str = "gemini_cli_stream";
pub(crate) const OPENAI_VIDEO_CREATE_SYNC_PLAN_KIND: &str = "openai_video_create_sync";
pub(crate) const OPENAI_CHAT_SYNC_PLAN_KIND: &str = "openai_chat_sync";
pub(crate) const OPENAI_CLI_SYNC_PLAN_KIND: &str = "openai_cli_sync";
pub(crate) const OPENAI_COMPACT_SYNC_PLAN_KIND: &str = "openai_compact_sync";
pub(crate) const CLAUDE_CHAT_SYNC_PLAN_KIND: &str = "claude_chat_sync";
pub(crate) const GEMINI_CHAT_SYNC_PLAN_KIND: &str = "gemini_chat_sync";
pub(crate) const CLAUDE_CLI_SYNC_PLAN_KIND: &str = "claude_cli_sync";
pub(crate) const GEMINI_CLI_SYNC_PLAN_KIND: &str = "gemini_cli_sync";
pub(crate) const EXECUTION_RUNTIME_SYNC_ACTION: &str = "execution_runtime_sync";
pub(crate) const EXECUTION_RUNTIME_SYNC_DECISION_ACTION: &str = "execution_runtime_sync_decision";
pub(crate) const EXECUTION_RUNTIME_STREAM_ACTION: &str = "execution_runtime_stream";
pub(crate) const EXECUTION_RUNTIME_STREAM_DECISION_ACTION: &str =
    "execution_runtime_stream_decision";

pub(crate) fn parse_direct_request_body(
    parts: &http::request::Parts,
    body_bytes: &Bytes,
) -> Option<(serde_json::Value, Option<String>)> {
    if is_json_request(&parts.headers) {
        if body_bytes.is_empty() {
            Some((serde_json::json!({}), None))
        } else {
            serde_json::from_slice::<serde_json::Value>(body_bytes)
                .ok()
                .map(|value| (value, None))
        }
    } else {
        Some((
            serde_json::json!({}),
            (!body_bytes.is_empty())
                .then(|| base64::engine::general_purpose::STANDARD.encode(body_bytes)),
        ))
    }
}
