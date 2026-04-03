use super::*;

#[path = "adaptive/routes.rs"]
mod adaptive_routes;
#[path = "adaptive/shared.rs"]
mod adaptive_shared;

pub(crate) async fn maybe_build_local_admin_adaptive_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    request_body: Option<&axum::body::Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    adaptive_routes::maybe_build_local_admin_adaptive_response(state, request_context, request_body)
        .await
}
