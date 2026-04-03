use super::*;

pub(super) const ADMIN_PROVIDER_STRATEGY_RUST_BACKEND_DETAIL: &str =
    "Admin provider strategy routes require Rust maintenance backend";
pub(super) const ADMIN_PROVIDER_STRATEGY_STATS_RUST_BACKEND_DETAIL: &str =
    "Admin provider strategy stats require Rust maintenance backend";

pub(super) fn admin_provider_strategy_maintenance_response(detail: &str) -> Response<Body> {
    build_proxy_error_response(
        http::StatusCode::SERVICE_UNAVAILABLE,
        "maintenance_mode",
        detail,
        Some(json!({ "error": detail })),
    )
}

pub(super) fn admin_provider_strategy_provider_not_found_response() -> Response<Body> {
    (
        http::StatusCode::NOT_FOUND,
        Json(json!({ "detail": "Provider not found" })),
    )
        .into_response()
}

pub(super) fn admin_provider_strategy_dispatcher_not_found_response() -> Response<Body> {
    (
        http::StatusCode::NOT_FOUND,
        Json(json!({ "detail": "Provider strategy route not found" })),
    )
        .into_response()
}
