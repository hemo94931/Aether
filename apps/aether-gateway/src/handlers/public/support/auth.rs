use super::*;

#[path = "auth_helpers.rs"]
mod auth_helpers;
pub(crate) use auth_helpers::*;

#[path = "auth_email.rs"]
mod auth_email;
use auth_email::*;

#[path = "auth_ldap.rs"]
mod auth_ldap;
use auth_ldap::*;

#[path = "auth_session.rs"]
pub(super) mod auth_session;
use auth_session::*;

#[path = "auth_registration.rs"]
pub(super) mod auth_registration;
use auth_registration::*;

#[derive(Debug, Deserialize)]
struct AuthLoginRequest {
    email: String,
    password: String,
    #[serde(default = "default_auth_login_type")]
    auth_type: String,
}

fn default_auth_login_type() -> String {
    "local".to_string()
}

fn system_config_f64(value: Option<&serde_json::Value>, default: f64) -> f64 {
    match value {
        Some(serde_json::Value::Number(value)) => value.as_f64().unwrap_or(default),
        Some(serde_json::Value::String(value)) => value.trim().parse::<f64>().unwrap_or(default),
        _ => default,
    }
}

fn system_config_u16(value: Option<&serde_json::Value>, default: u16) -> u16 {
    match value {
        Some(serde_json::Value::Number(value)) => value
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(default),
        Some(serde_json::Value::String(value)) => value.trim().parse::<u16>().unwrap_or(default),
        _ => default,
    }
}

fn system_config_string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase())
            .collect(),
        Some(serde_json::Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase())
            .collect(),
        _ => Vec::new(),
    }
}
async fn handle_auth_login(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
    request_body: Option<&axum::body::Bytes>,
) -> Response<Body> {
    let Some(request_body) = request_body else {
        return build_auth_error_response(http::StatusCode::BAD_REQUEST, "缺少登录请求体", false);
    };
    let payload = match serde_json::from_slice::<AuthLoginRequest>(request_body) {
        Ok(value) => value,
        Err(_) => {
            return build_auth_error_response(
                http::StatusCode::BAD_REQUEST,
                "无效的登录请求",
                false,
            )
        }
    };
    let identifier = normalize_auth_login_identifier(&payload.email);
    if identifier.is_empty() {
        return build_auth_error_response(
            http::StatusCode::BAD_REQUEST,
            "邮箱或用户名不能为空",
            false,
        );
    }
    if let Err(detail) = validate_auth_login_password(&payload.password) {
        return build_auth_error_response(http::StatusCode::BAD_REQUEST, detail, false);
    }
    let client_device_id = match extract_client_device_id(request_context, headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let auth_type = payload.auth_type.trim().to_ascii_lowercase();
    let user = match auth_type.as_str() {
        "local" => {
            let user = match state.find_user_auth_by_identifier(&identifier).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    return build_auth_error_response(
                        http::StatusCode::UNAUTHORIZED,
                        "邮箱或密码错误",
                        false,
                    )
                }
                Err(err) => {
                    return build_auth_error_response(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("auth user lookup failed: {err:?}"),
                        false,
                    )
                }
            };
            if user.is_deleted
                || !user.is_active
                || !user.auth_source.eq_ignore_ascii_case("local")
                || user.password_hash.as_deref().is_none_or(str::is_empty)
            {
                return build_auth_error_response(
                    http::StatusCode::UNAUTHORIZED,
                    "邮箱或密码错误",
                    false,
                );
            }
            match auth_local_login_allowed_for_user(state, &user).await {
                Ok(true) => {}
                Ok(false) => {
                    return build_auth_error_response(
                        http::StatusCode::UNAUTHORIZED,
                        "邮箱或密码错误",
                        false,
                    )
                }
                Err(err) => {
                    return build_auth_error_response(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("auth settings lookup failed: {err:?}"),
                        false,
                    )
                }
            }
            let password_hash = user
                .password_hash
                .as_deref()
                .expect("validated password hash should exist");
            let password_matches =
                bcrypt::verify(&payload.password, password_hash).unwrap_or(false);
            if !password_matches {
                return build_auth_error_response(
                    http::StatusCode::UNAUTHORIZED,
                    "邮箱或密码错误",
                    false,
                );
            }
            user
        }
        "ldap" => {
            let ldap_user =
                match authenticate_auth_ldap_user(state, &identifier, &payload.password).await {
                    Ok(Some(user)) => user,
                    Ok(None) => {
                        return build_auth_error_response(
                            http::StatusCode::UNAUTHORIZED,
                            "邮箱或密码错误",
                            false,
                        )
                    }
                    Err(err) => {
                        return build_auth_error_response(
                            http::StatusCode::INTERNAL_SERVER_ERROR,
                            format!("auth ldap login failed: {err:?}"),
                            false,
                        )
                    }
                };
            let _ = &ldap_user.display_name;
            let initial_gift = match state
                .read_system_config_json_value("default_user_initial_gift_usd")
                .await
            {
                Ok(value) => system_config_f64(value.as_ref(), 10.0),
                Err(err) => {
                    return build_auth_error_response(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("auth settings lookup failed: {err:?}"),
                        false,
                    )
                }
            };
            match state
                .get_or_create_ldap_auth_user(
                    ldap_user.email,
                    ldap_user.username,
                    Some(ldap_user.ldap_dn),
                    Some(ldap_user.ldap_username),
                    auth_now(),
                    initial_gift,
                    false,
                )
                .await
            {
                Ok(Some(user)) => user,
                Ok(None) => {
                    return build_auth_error_response(
                        http::StatusCode::UNAUTHORIZED,
                        "邮箱或密码错误",
                        false,
                    )
                }
                Err(err) => {
                    return build_auth_error_response(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("auth ldap user sync failed: {err:?}"),
                        false,
                    )
                }
            }
        }
        _ => {
            return build_public_support_maintenance_response(
                "Non-local auth login requires Rust maintenance backend",
            )
        }
    };

    build_auth_login_success_response(state, headers, client_device_id, user).await
}

pub(super) async fn maybe_build_local_auth_legacy_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
    request_body: Option<&axum::body::Bytes>,
) -> Option<Response<Body>> {
    let decision = request_context.control_decision.as_ref()?;
    if decision.route_family.as_deref() != Some("auth_legacy") {
        return None;
    }

    match decision.route_kind.as_deref() {
        Some("send_verification_code")
            if request_context.request_path == "/api/auth/send-verification-code" =>
        {
            Some(handle_auth_send_verification_code(state, request_body).await)
        }
        Some("login") if request_context.request_path == "/api/auth/login" => {
            Some(handle_auth_login(state, request_context, headers, request_body).await)
        }
        Some("register") if request_context.request_path == "/api/auth/register" => {
            Some(handle_auth_register(state, request_body).await)
        }
        Some("verify_email") if request_context.request_path == "/api/auth/verify-email" => {
            Some(handle_auth_verify_email(state, request_body).await)
        }
        Some("verification_status")
            if request_context.request_path == "/api/auth/verification-status" =>
        {
            Some(handle_auth_verification_status(state, request_body).await)
        }
        Some("me") if request_context.request_path == "/api/auth/me" => {
            Some(handle_auth_me(state, request_context, headers).await)
        }
        Some("refresh") if request_context.request_path == "/api/auth/refresh" => {
            Some(handle_auth_refresh(state, request_context, headers).await)
        }
        Some("logout") if request_context.request_path == "/api/auth/logout" => {
            Some(handle_auth_logout(state, request_context, headers).await)
        }
        _ => Some(build_public_support_maintenance_response(
            "Auth routes require Rust maintenance backend",
        )),
    }
}
