use super::*;

const USERS_ME_MAINTENANCE_DETAIL: &str =
    "User self-service routes require Rust maintenance backend";
const USERS_ME_AVAILABLE_MODELS_FETCH_LIMIT: usize = 1000;

#[path = "user_me_management_tokens.rs"]
mod user_me_management_tokens;
use user_me_management_tokens::*;

#[path = "user_me_api_keys.rs"]
mod user_me_api_keys;
use user_me_api_keys::*;
#[path = "user_me_usage.rs"]
mod user_me_usage;
use user_me_usage::*;
#[path = "user_me_catalog.rs"]
mod user_me_catalog;
use user_me_catalog::*;
#[path = "user_me_preferences.rs"]
mod user_me_preferences;
use user_me_preferences::*;
#[path = "user_me_profile.rs"]
mod user_me_profile;
use user_me_profile::*;
#[path = "user_me_sessions.rs"]
mod user_me_sessions;
use user_me_sessions::*;
#[path = "user_me_shared.rs"]
mod user_me_shared;
use user_me_shared::*;
#[path = "user_me_routes.rs"]
mod user_me_routes;
use user_me_routes::*;

pub(super) use self::user_me_routes::maybe_build_local_users_me_legacy_response;
