use super::*;

#[path = "payment/postgres.rs"]
mod payment_postgres;
#[path = "payment/route.rs"]
mod payment_route;
#[path = "payment/shared.rs"]
mod payment_shared;
#[cfg(test)]
#[path = "payment/test_support.rs"]
mod payment_test_support;

pub(super) use self::payment_postgres::*;
pub(super) use self::payment_shared::*;

use self::payment_shared::NormalizedPaymentCallbackRequest;

pub(super) async fn maybe_build_local_payment_callback_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
    request_body: Option<&axum::body::Bytes>,
) -> Option<Response<Body>> {
    payment_route::maybe_build_local_payment_callback_route_response(
        state,
        request_context,
        headers,
        request_body,
    )
    .await
}
