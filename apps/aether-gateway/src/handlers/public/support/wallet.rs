use super::*;
use sqlx::Row;

#[cfg(test)]
#[path = "wallet/test_support.rs"]
mod test_support;
#[cfg(test)]
pub(crate) use self::test_support::wallet_test_recharge_store;
#[cfg(test)]
use self::test_support::*;
#[path = "wallet/flow.rs"]
mod flow;
#[path = "wallet/reads.rs"]
mod reads;
#[path = "wallet/recharge.rs"]
mod recharge;
#[path = "wallet/refunds.rs"]
mod refunds;
use self::flow::handle_wallet_flow;
use self::reads::{
    build_wallet_daily_usage_payload, build_wallet_payload, build_wallet_zero_today_entry,
    handle_wallet_balance, handle_wallet_today_cost, handle_wallet_transactions,
    parse_wallet_limit, parse_wallet_offset, wallet_fixed_offset, wallet_today_billing_date_string,
    wallet_transaction_payload_from_row,
};
use self::recharge::{
    handle_wallet_create_recharge, handle_wallet_recharge_detail, handle_wallet_recharge_list,
};
pub(crate) use self::recharge::{
    sanitize_wallet_gateway_response, wallet_payment_order_payload_from_row,
};
use self::refunds::{
    handle_wallet_create_refund, handle_wallet_refund_detail, handle_wallet_refunds_list,
};

const WALLET_LEGACY_TIMEZONE: &str = "Asia/Shanghai";
const WALLET_SAFE_GATEWAY_RESPONSE_KEYS: &[&str] = &[
    "gateway",
    "display_name",
    "gateway_order_id",
    "payment_url",
    "qr_code",
    "expires_at",
    "manual_credit",
];

pub(super) fn wallet_normalize_optional_string_field(
    value: Option<String>,
    max_chars: usize,
) -> Result<Option<String>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > max_chars {
        return Err("输入验证失败");
    }
    Ok(Some(trimmed.to_string()))
}

pub(super) async fn maybe_build_local_wallet_legacy_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
    request_body: Option<&axum::body::Bytes>,
) -> Option<Response<Body>> {
    let decision = request_context.control_decision.as_ref()?;
    if decision.route_family.as_deref() != Some("wallet_legacy") {
        return None;
    }

    if decision.route_kind.as_deref() == Some("balance")
        && request_context.request_path == "/api/wallet/balance"
    {
        return Some(handle_wallet_balance(state, request_context, headers).await);
    }

    if decision.route_kind.as_deref() == Some("today_cost")
        && request_context.request_path == "/api/wallet/today-cost"
    {
        return Some(handle_wallet_today_cost(state, request_context, headers).await);
    }

    if decision.route_kind.as_deref() == Some("transactions")
        && request_context.request_path == "/api/wallet/transactions"
    {
        return Some(handle_wallet_transactions(state, request_context, headers).await);
    }

    if decision.route_kind.as_deref() == Some("flow")
        && request_context.request_path == "/api/wallet/flow"
    {
        return Some(handle_wallet_flow(state, request_context, headers).await);
    }

    if decision.route_kind.as_deref() == Some("list_refunds")
        && request_context.request_path == "/api/wallet/refunds"
    {
        return Some(handle_wallet_refunds_list(state, request_context, headers).await);
    }

    if decision.route_kind.as_deref() == Some("refund_detail")
        && request_context
            .request_path
            .starts_with("/api/wallet/refunds/")
    {
        return Some(handle_wallet_refund_detail(state, request_context, headers).await);
    }

    if decision.route_kind.as_deref() == Some("create_refund")
        && request_context.request_path == "/api/wallet/refunds"
    {
        return Some(
            handle_wallet_create_refund(state, request_context, headers, request_body).await,
        );
    }

    if decision.route_kind.as_deref() == Some("create_recharge_order")
        && request_context.request_path == "/api/wallet/recharge"
    {
        return Some(
            handle_wallet_create_recharge(state, request_context, headers, request_body).await,
        );
    }

    if decision.route_kind.as_deref() == Some("list_recharge_orders")
        && request_context.request_path == "/api/wallet/recharge"
    {
        return Some(handle_wallet_recharge_list(state, request_context, headers).await);
    }

    if decision.route_kind.as_deref() == Some("recharge_detail")
        && request_context
            .request_path
            .starts_with("/api/wallet/recharge/")
    {
        return Some(handle_wallet_recharge_detail(state, request_context, headers).await);
    }

    None
}
