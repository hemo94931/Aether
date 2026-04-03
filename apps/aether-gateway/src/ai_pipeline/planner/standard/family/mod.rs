mod candidates;
mod execute;
mod payload;
mod types;

pub(crate) use self::execute::{
    maybe_build_stream_via_standard_family_payload, maybe_build_sync_via_standard_family_payload,
    maybe_execute_stream_via_standard_family_decision,
    maybe_execute_sync_via_standard_family_decision,
};
pub(crate) use self::types::{
    LocalStandardSourceFamily, LocalStandardSourceMode, LocalStandardSpec,
};
