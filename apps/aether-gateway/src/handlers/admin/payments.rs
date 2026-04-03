use super::*;

#[path = "payment/postgres.rs"]
mod payment_postgres;
#[path = "payments/callbacks.rs"]
mod payments_callbacks;
#[path = "payments/orders.rs"]
mod payments_orders;
#[path = "payments/routes.rs"]
mod payments_routes;
#[path = "payments/shared.rs"]
mod payments_shared;

use payment_postgres::*;
use payments_shared::*;

pub(crate) async fn maybe_build_local_admin_payments_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    request_body: Option<&axum::body::Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    payments_routes::maybe_build_local_admin_payments_response(state, request_context, request_body)
        .await
}
