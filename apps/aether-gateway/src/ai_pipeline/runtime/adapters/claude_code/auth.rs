use crate::gateway::provider_transport::auth::resolve_local_standard_auth;
use crate::gateway::provider_transport::snapshot::GatewayProviderTransportSnapshot;
use crate::gateway::provider_transport::supports_local_oauth_request_auth_resolution;

pub(crate) fn supports_local_claude_code_auth(
    transport: &GatewayProviderTransportSnapshot,
) -> bool {
    resolve_local_standard_auth(transport).is_some()
        || supports_local_oauth_request_auth_resolution(transport)
}
