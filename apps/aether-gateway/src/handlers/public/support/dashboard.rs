use super::*;

const DASHBOARD_MAINTENANCE_DETAIL: &str = "Dashboard routes require Rust maintenance backend";

#[path = "dashboard_filters.rs"]
mod dashboard_helpers;

use self::dashboard_helpers::{
    decision_route_kind, handle_dashboard_daily_stats_get, handle_dashboard_provider_status_get,
    handle_dashboard_recent_requests_get, handle_dashboard_stats_get,
};

pub(super) async fn maybe_build_local_dashboard_legacy_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
) -> Response<Body> {
    match decision_route_kind(request_context) {
        Some("stats")
            if request_context.request_method == http::Method::GET
                && request_context.request_path == "/api/dashboard/stats" =>
        {
            handle_dashboard_stats_get(state, request_context, headers).await
        }
        Some("recent_requests")
            if request_context.request_method == http::Method::GET
                && request_context.request_path == "/api/dashboard/recent-requests" =>
        {
            handle_dashboard_recent_requests_get(state, request_context, headers).await
        }
        Some("provider_status")
            if request_context.request_method == http::Method::GET
                && request_context.request_path == "/api/dashboard/provider-status" =>
        {
            handle_dashboard_provider_status_get(state, request_context, headers).await
        }
        Some("daily_stats")
            if request_context.request_method == http::Method::GET
                && request_context.request_path == "/api/dashboard/daily-stats" =>
        {
            handle_dashboard_daily_stats_get(state, request_context, headers).await
        }
        _ => build_public_support_maintenance_response(DASHBOARD_MAINTENANCE_DETAIL),
    }
}
