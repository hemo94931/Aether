use super::*;

pub(super) const ADMIN_GLOBAL_MODELS_RUST_BACKEND_DETAIL: &str =
    "Admin global model routes require Rust maintenance backend";

pub(super) fn build_admin_global_models_maintenance_response() -> Response<Body> {
    (
        http::StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "detail": ADMIN_GLOBAL_MODELS_RUST_BACKEND_DETAIL })),
    )
        .into_response()
}
