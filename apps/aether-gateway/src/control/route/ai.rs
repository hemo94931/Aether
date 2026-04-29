use super::{
    classified, is_claude_cli_request, is_gemini_cli_request, is_gemini_models_route,
    is_gemini_operation_route, ClassifiedRoute,
};

pub(super) fn classify_ai_public_route(
    method: &http::Method,
    normalized_path: &str,
    headers: &http::HeaderMap,
) -> Option<ClassifiedRoute> {
    if method == http::Method::POST && normalized_path == "/v1/chat/completions" {
        Some(classified(
            "ai_public",
            "openai",
            "chat",
            "openai:chat",
            true,
        ))
    } else if method == http::Method::POST
        && matches!(normalized_path, "/v1/responses" | "/v1/responses/compact")
    {
        if normalized_path.ends_with("/compact") {
            Some(classified(
                "ai_public",
                "openai",
                "responses:compact",
                "openai:responses:compact",
                true,
            ))
        } else {
            Some(classified(
                "ai_public",
                "openai",
                "responses",
                "openai:responses",
                true,
            ))
        }
    } else if method == http::Method::POST
        && matches!(
            normalized_path,
            "/v1/images/generations" | "/v1/images/edits" | "/v1/images/variations"
        )
    {
        Some(classified(
            "ai_public",
            "openai",
            "image",
            "openai:image",
            true,
        ))
    } else if method == http::Method::POST && normalized_path == "/v1/messages/count_tokens" {
        Some(classified(
            "ai_public",
            "claude",
            "count_tokens",
            "claude:messages",
            false,
        ))
    } else if method == http::Method::POST && normalized_path == "/v1/messages" {
        let route_kind = if is_claude_cli_request(headers) {
            "cli"
        } else {
            "messages"
        };
        Some(classified(
            "ai_public",
            "claude",
            route_kind,
            "claude:messages",
            true,
        ))
    } else if normalized_path.starts_with("/v1/videos") {
        Some(classified(
            "ai_public",
            "openai",
            "video",
            "openai:video",
            true,
        ))
    } else if is_gemini_models_route(normalized_path) {
        if normalized_path.ends_with(":predictLongRunning") {
            Some(classified(
                "ai_public",
                "gemini",
                "video",
                "gemini:video",
                true,
            ))
        } else if is_gemini_cli_request(headers) {
            Some(classified(
                "ai_public",
                "gemini",
                "cli",
                "gemini:generate_content",
                true,
            ))
        } else {
            Some(classified(
                "ai_public",
                "gemini",
                "generate_content",
                "gemini:generate_content",
                true,
            ))
        }
    } else if is_gemini_operation_route(normalized_path) {
        Some(classified(
            "ai_public",
            "gemini",
            "video",
            "gemini:video",
            true,
        ))
    } else if (method == http::Method::POST && normalized_path == "/upload/v1beta/files")
        || normalized_path.starts_with("/v1beta/files")
    {
        Some(classified(
            "ai_public",
            "gemini",
            "files",
            "gemini:files",
            true,
        ))
    } else {
        None
    }
}
