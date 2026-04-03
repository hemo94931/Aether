use super::*;

const ADMIN_API_KEYS_RUST_BACKEND_DETAIL: &str =
    "Admin standalone API key routes require Rust maintenance backend";

#[path = "api_keys/mutation_routes.rs"]
mod admin_api_keys_mutation_routes;
#[path = "api_keys/read_routes.rs"]
mod admin_api_keys_read_routes;
#[path = "api_keys/routes.rs"]
mod admin_api_keys_routes;
#[path = "api_keys/shared.rs"]
mod admin_api_keys_shared;

use self::admin_api_keys_mutation_routes::{
    build_admin_create_api_key_response, build_admin_delete_api_key_response,
    build_admin_toggle_api_key_response, build_admin_update_api_key_response,
};
use self::admin_api_keys_read_routes::{
    build_admin_api_key_detail_response, build_admin_list_api_keys_response,
};
use self::admin_api_keys_shared::{
    admin_api_key_total_tokens_by_ids, admin_api_keys_id_from_path, admin_api_keys_operator_id,
    admin_api_keys_parse_limit, admin_api_keys_parse_skip, build_admin_api_key_detail_payload,
    build_admin_api_key_list_item_payload, build_admin_api_keys_bad_request_response,
    build_admin_api_keys_maintenance_response, build_admin_api_keys_not_found_response,
    AdminStandaloneApiKeyCreateRequest, AdminStandaloneApiKeyFieldPresence,
    AdminStandaloneApiKeyToggleRequest, AdminStandaloneApiKeyUpdateRequest,
};

pub(crate) async fn maybe_build_local_admin_api_keys_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    request_body: Option<&axum::body::Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    admin_api_keys_routes::maybe_build_local_admin_api_keys_routes_response(
        state,
        request_context,
        request_body,
    )
    .await
}
