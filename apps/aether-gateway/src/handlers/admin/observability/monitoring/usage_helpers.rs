use aether_admin::observability::usage::admin_usage_is_failed;
use aether_data_contracts::repository::usage::StoredRequestUsageAudit;

pub(super) fn admin_monitoring_usage_is_error(item: &StoredRequestUsageAudit) -> bool {
    item.status.trim().eq_ignore_ascii_case("error")
        || admin_usage_is_failed(item)
        || item.error_category.is_some()
}
