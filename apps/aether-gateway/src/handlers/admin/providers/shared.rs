use super::*;

const ADMIN_PROVIDERS_RUST_BACKEND_DETAIL: &str =
    "Admin provider catalog routes require Rust maintenance backend";

pub(super) fn build_admin_providers_maintenance_response() -> Response<Body> {
    (
        http::StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "detail": ADMIN_PROVIDERS_RUST_BACKEND_DETAIL })),
    )
        .into_response()
}
