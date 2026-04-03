use super::*;
use super::{build_auth_error_response, resolve_authenticated_local_user};

const USER_MONITORING_MAINTENANCE_DETAIL: &str =
    "User monitoring routes require Rust maintenance backend";

#[path = "monitoring/audit_logs.rs"]
mod user_monitoring_audit_logs;
#[path = "monitoring/rate_limit_status.rs"]
mod user_monitoring_rate_limit_status;

use self::user_monitoring_audit_logs::handle_user_audit_logs;
use self::user_monitoring_rate_limit_status::handle_user_rate_limit_status;

pub(super) async fn maybe_build_local_user_monitoring_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
) -> Option<Response<Body>> {
    let decision = request_context.control_decision.as_ref()?;
    if decision.route_family.as_deref() != Some("monitoring_user_legacy") {
        return None;
    }

    match decision.route_kind.as_deref() {
        Some("audit_logs")
            if request_context.request_method == http::Method::GET
                && request_context.request_path == "/api/monitoring/my-audit-logs" =>
        {
            Some(handle_user_audit_logs(state, request_context, headers).await)
        }
        Some("rate_limit_status")
            if request_context.request_method == http::Method::GET
                && request_context.request_path == "/api/monitoring/rate-limit-status" =>
        {
            Some(handle_user_rate_limit_status(state, request_context, headers).await)
        }
        _ => Some(build_public_support_maintenance_response(
            USER_MONITORING_MAINTENANCE_DETAIL,
        )),
    }
}
