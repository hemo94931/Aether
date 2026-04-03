use super::*;

#[path = "test_connection/route.rs"]
mod test_connection_route;
#[path = "test_connection/shared.rs"]
mod test_connection_shared;

pub(super) async fn maybe_build_local_test_connection_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
) -> Option<Response<Body>> {
    test_connection_route::maybe_build_local_test_connection_route_response(state, request_context)
        .await
}
