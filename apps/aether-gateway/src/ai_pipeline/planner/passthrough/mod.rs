//! Requests that can stay in the same public/provider contract family.

mod provider;

pub(crate) use self::provider::{
    maybe_build_stream_local_same_format_provider_decision_payload,
    maybe_build_sync_local_same_format_provider_decision_payload,
    maybe_execute_stream_via_local_same_format_provider_decision,
    maybe_execute_sync_via_local_same_format_provider_decision,
};
pub(crate) use crate::gateway::provider_transport::provider_type_supports_local_same_format_transport;
