use std::time::Duration;

pub(crate) const DIRECT_PLAN_BYPASS_TTL: Duration = Duration::from_secs(30);
pub(crate) const DIRECT_PLAN_BYPASS_MAX_ENTRIES: usize = 512;
