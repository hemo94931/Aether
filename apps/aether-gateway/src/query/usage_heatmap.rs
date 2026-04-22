use aether_data::postgres::PostgresPool;
use aether_data_contracts::repository::usage::StoredUsageDailySummary;
use chrono::{DateTime, NaiveDate, Utc};
use futures_util::TryStreamExt;
use sqlx::Row;

use crate::GatewayError;

const USER_HEATMAP_AGGREGATE_SQL: &str = r#"
SELECT
  date,
  total_requests,
  input_tokens,
  output_tokens,
  cache_creation_tokens,
  cache_read_tokens,
  COALESCE(total_cost, 0)::DOUBLE PRECISION AS total_cost,
  COALESCE(actual_total_cost, 0)::DOUBLE PRECISION AS actual_total_cost
FROM stats_user_daily
WHERE user_id = $1
  AND date >= $2
  AND date < $3
ORDER BY date ASC
"#;

const GLOBAL_HEATMAP_AGGREGATE_SQL: &str = r#"
SELECT
  date,
  total_requests,
  input_tokens,
  output_tokens,
  cache_creation_tokens,
  cache_read_tokens,
  COALESCE(total_cost, 0)::DOUBLE PRECISION AS total_cost,
  COALESCE(actual_total_cost, 0)::DOUBLE PRECISION AS actual_total_cost
FROM stats_daily
WHERE date >= $1
  AND date < $2
ORDER BY date ASC
"#;

fn internal(err: impl ToString) -> GatewayError {
    GatewayError::Internal(err.to_string())
}

pub(crate) async fn read_stats_daily_cutoff_date(
    pool: &PostgresPool,
) -> Result<Option<DateTime<Utc>>, GatewayError> {
    let row = sqlx::query(
        r#"
SELECT cutoff_date
FROM stats_summary
ORDER BY updated_at DESC, created_at DESC
LIMIT 1
"#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|err| internal(format!("stats summary cutoff lookup failed: {err}")))?;

    let Some(row) = row else {
        return Ok(None);
    };

    row.try_get::<DateTime<Utc>, _>("cutoff_date")
        .map(Some)
        .map_err(|err| internal(format!("stats summary cutoff decode failed: {err}")))
}

pub(crate) async fn list_usage_heatmap_aggregate_rows(
    pool: &PostgresPool,
    start_date: NaiveDate,
    end_date_exclusive: NaiveDate,
    user_id: Option<&str>,
) -> Result<Vec<StoredUsageDailySummary>, GatewayError> {
    if start_date >= end_date_exclusive {
        return Ok(Vec::new());
    }

    let start_at = DateTime::<Utc>::from_naive_utc_and_offset(
        start_date
            .and_hms_opt(0, 0, 0)
            .expect("midnight should be valid"),
        Utc,
    );
    let end_at = DateTime::<Utc>::from_naive_utc_and_offset(
        end_date_exclusive
            .and_hms_opt(0, 0, 0)
            .expect("midnight should be valid"),
        Utc,
    );

    let mut rows = if let Some(user_id) = user_id {
        sqlx::query(USER_HEATMAP_AGGREGATE_SQL)
            .bind(user_id)
            .bind(start_at)
            .bind(end_at)
            .fetch(pool)
    } else {
        sqlx::query(GLOBAL_HEATMAP_AGGREGATE_SQL)
            .bind(start_at)
            .bind(end_at)
            .fetch(pool)
    };

    let mut items = Vec::new();
    while let Some(row) = rows
        .try_next()
        .await
        .map_err(|err| internal(format!("aggregate heatmap read failed: {err}")))?
    {
        let date = row
            .try_get::<DateTime<Utc>, _>("date")
            .map_err(|err| internal(format!("aggregate heatmap date decode failed: {err}")))?;
        let requests = row
            .try_get::<i32, _>("total_requests")
            .map_err(|err| internal(format!("aggregate heatmap request decode failed: {err}")))?;
        let input_tokens = row
            .try_get::<i64, _>("input_tokens")
            .map_err(|err| internal(format!("aggregate heatmap token decode failed: {err}")))?;
        let output_tokens = row
            .try_get::<i64, _>("output_tokens")
            .map_err(|err| internal(format!("aggregate heatmap token decode failed: {err}")))?;
        let cache_creation_tokens = row
            .try_get::<i64, _>("cache_creation_tokens")
            .map_err(|err| internal(format!("aggregate heatmap token decode failed: {err}")))?;
        let cache_read_tokens = row
            .try_get::<i64, _>("cache_read_tokens")
            .map_err(|err| internal(format!("aggregate heatmap token decode failed: {err}")))?;
        let total_cost_usd = row
            .try_get::<f64, _>("total_cost")
            .map_err(|err| internal(format!("aggregate heatmap cost decode failed: {err}")))?;
        let actual_total_cost_usd = row.try_get::<f64, _>("actual_total_cost").map_err(|err| {
            internal(format!(
                "aggregate heatmap actual cost decode failed: {err}"
            ))
        })?;
        items.push(StoredUsageDailySummary {
            date: date.date_naive().to_string(),
            requests: u64::try_from(requests.max(0)).unwrap_or_default(),
            total_tokens: u64::try_from(
                input_tokens
                    .saturating_add(output_tokens)
                    .saturating_add(cache_creation_tokens)
                    .saturating_add(cache_read_tokens)
                    .max(0),
            )
            .unwrap_or_default(),
            total_cost_usd,
            actual_total_cost_usd,
        });
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::{GLOBAL_HEATMAP_AGGREGATE_SQL, USER_HEATMAP_AGGREGATE_SQL};

    #[test]
    fn user_heatmap_query_casts_cost_columns_to_double_precision() {
        assert!(USER_HEATMAP_AGGREGATE_SQL
            .contains("COALESCE(total_cost, 0)::DOUBLE PRECISION AS total_cost"));
        assert!(USER_HEATMAP_AGGREGATE_SQL
            .contains("COALESCE(actual_total_cost, 0)::DOUBLE PRECISION AS actual_total_cost"));
    }

    #[test]
    fn global_heatmap_query_casts_cost_columns_to_double_precision() {
        assert!(GLOBAL_HEATMAP_AGGREGATE_SQL
            .contains("COALESCE(total_cost, 0)::DOUBLE PRECISION AS total_cost"));
        assert!(GLOBAL_HEATMAP_AGGREGATE_SQL
            .contains("COALESCE(actual_total_cost, 0)::DOUBLE PRECISION AS actual_total_cost"));
    }
}
