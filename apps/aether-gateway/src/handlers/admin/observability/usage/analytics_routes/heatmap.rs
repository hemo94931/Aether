use crate::handlers::admin::request::AdminAppState;
use crate::query::usage_heatmap::{
    list_usage_heatmap_aggregate_rows, read_stats_daily_cutoff_date,
};
use crate::GatewayError;
use aether_admin::observability::stats::round_to;
use aether_admin::observability::usage::{
    admin_usage_data_unavailable_response, ADMIN_USAGE_DATA_UNAVAILABLE_DETAIL,
};
use aether_data_contracts::repository::usage::{StoredUsageDailySummary, UsageDailyHeatmapQuery};
use axum::{
    body::Body,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::collections::BTreeMap;

pub(super) async fn build_admin_usage_heatmap_response(
    state: &AdminAppState<'_>,
) -> Result<Response<Body>, GatewayError> {
    if !state.has_usage_data_reader() {
        return Ok(admin_usage_data_unavailable_response(
            ADMIN_USAGE_DATA_UNAVAILABLE_DETAIL,
        ));
    }
    let today = chrono::Utc::now().date_naive();
    let start_date = today
        .checked_sub_signed(chrono::Duration::days(364))
        .unwrap_or(today);
    let created_from_unix_secs = u64::try_from(
        chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            start_date.and_hms_opt(0, 0, 0).unwrap_or_default(),
            chrono::Utc,
        )
        .timestamp(),
    )
    .unwrap_or_default();

    let summaries =
        build_admin_heatmap_summaries(state, created_from_unix_secs, start_date, today).await?;

    let grouped: BTreeMap<String, _> = summaries.into_iter().map(|s| (s.date.clone(), s)).collect();

    let mut max_requests = 0_u64;
    let mut active_days = 0_u64;
    let mut cursor = start_date;
    let mut days = Vec::new();
    while cursor <= today {
        let date_str = cursor.to_string();
        let (requests, total_tokens, total_cost, actual_total_cost) =
            if let Some(s) = grouped.get(&date_str) {
                (
                    s.requests,
                    s.total_tokens,
                    s.total_cost_usd,
                    s.actual_total_cost_usd,
                )
            } else {
                (0, 0, 0.0, 0.0)
            };
        max_requests = max_requests.max(requests);
        if requests > 0 {
            active_days = active_days.saturating_add(1);
        }
        days.push(json!({
            "date": date_str,
            "requests": requests,
            "total_tokens": total_tokens,
            "total_cost": round_to(total_cost, 6),
            "actual_total_cost": round_to(actual_total_cost, 6),
        }));
        cursor = cursor
            .checked_add_signed(chrono::Duration::days(1))
            .unwrap_or(today + chrono::Duration::days(1));
    }

    Ok(Json(json!({
        "start_date": start_date.to_string(),
        "end_date": today.to_string(),
        "total_days": days.len(),
        "active_days": active_days,
        "max_requests": max_requests,
        "days": days,
    }))
    .into_response())
}

async fn build_admin_heatmap_summaries(
    state: &AdminAppState<'_>,
    created_from_unix_secs: u64,
    start_date: chrono::NaiveDate,
    today: chrono::NaiveDate,
) -> Result<Vec<StoredUsageDailySummary>, GatewayError> {
    let query = UsageDailyHeatmapQuery {
        created_from_unix_secs,
        user_id: None,
        admin_mode: true,
    };
    let Some(pool) = state.app().postgres_pool() else {
        return state.summarize_usage_daily_heatmap(&query).await;
    };

    let Some(cutoff_date) = read_stats_daily_cutoff_date(&pool).await? else {
        return state.summarize_usage_daily_heatmap(&query).await;
    };

    let cutoff_day = cutoff_date.date_naive().min(today);
    let mut summaries =
        list_usage_heatmap_aggregate_rows(&pool, start_date, cutoff_day, None).await?;
    let raw_start_date = start_date.max(cutoff_day);
    if raw_start_date <= today {
        let raw_start_of_day = raw_start_date
            .and_hms_opt(0, 0, 0)
            .expect("heatmap day start should be valid");
        let raw_created_from_unix_secs = u64::try_from(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                raw_start_of_day,
                chrono::Utc,
            )
            .timestamp(),
        )
        .unwrap_or_default();
        summaries.extend(
            state
                .summarize_usage_daily_heatmap(&UsageDailyHeatmapQuery {
                    created_from_unix_secs: raw_created_from_unix_secs,
                    user_id: None,
                    admin_mode: true,
                })
                .await?,
        );
    }

    summaries.sort_by(|left, right| left.date.cmp(&right.date));
    Ok(summaries)
}
