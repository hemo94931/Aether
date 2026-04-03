use super::*;

#[tokio::test]
async fn gateway_strips_spoofed_admin_principal_headers_without_gateway_marker() {
    #[derive(Debug, Clone)]
    struct SeenAdminRequest {
        trusted_admin_user_id: String,
        trusted_admin_user_role: String,
        trusted_admin_session_id: String,
    }

    let seen_admin = Arc::new(Mutex::new(None::<SeenAdminRequest>));
    let seen_admin_clone = Arc::clone(&seen_admin);
    let upstream = Router::new().route(
        "/api/admin/endpoints/health/api-formats",
        any(move |request: Request| {
            let seen_admin_inner = Arc::clone(&seen_admin_clone);
            async move {
                *seen_admin_inner.lock().expect("mutex should lock") = Some(SeenAdminRequest {
                    trusted_admin_user_id: request
                        .headers()
                        .get(TRUSTED_ADMIN_USER_ID_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    trusted_admin_user_role: request
                        .headers()
                        .get(TRUSTED_ADMIN_USER_ROLE_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    trusted_admin_session_id: request
                        .headers()
                        .get(TRUSTED_ADMIN_SESSION_ID_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                });
                (StatusCode::OK, Body::from("proxied"))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router(upstream_url.clone()).expect("gateway should build");
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/api/admin/endpoints/health/api-formats"
        ))
        .header(TRUSTED_ADMIN_USER_ID_HEADER, "admin-user-123")
        .header(TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(TRUSTED_ADMIN_SESSION_ID_HEADER, "session-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let seen_request = seen_admin
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("admin request should be captured");
    assert_eq!(seen_request.trusted_admin_user_id, "");
    assert_eq!(seen_request.trusted_admin_user_role, "");
    assert_eq!(seen_request.trusted_admin_session_id, "");

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_proxies_models_support_routes_with_public_support_control_headers() {
    #[derive(Debug, Clone)]
    struct SeenSupportRequest {
        control_route_class: String,
        control_route_family: String,
        control_route_kind: String,
        legacy_control_execution_runtime_candidate: String,
        control_execution_runtime_candidate: String,
        control_endpoint_signature: String,
        trace_id: String,
    }

    let seen_support = Arc::new(Mutex::new(None::<SeenSupportRequest>));
    let seen_support_clone = Arc::clone(&seen_support);

    let upstream = Router::new().route(
        "/v1/models",
        any(move |request: Request| {
            let seen_support_inner = Arc::clone(&seen_support_clone);
            async move {
                *seen_support_inner.lock().expect("mutex should lock") = Some(SeenSupportRequest {
                    control_route_class: request
                        .headers()
                        .get(CONTROL_ROUTE_CLASS_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    control_route_family: request
                        .headers()
                        .get(CONTROL_ROUTE_FAMILY_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    control_route_kind: request
                        .headers()
                        .get(CONTROL_ROUTE_KIND_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    legacy_control_execution_runtime_candidate: request
                        .headers()
                        .get(CONTROL_LEGACY_EXECUTION_RUNTIME_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    control_execution_runtime_candidate: request
                        .headers()
                        .get(CONTROL_EXECUTION_RUNTIME_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    control_endpoint_signature: request
                        .headers()
                        .get(CONTROL_ENDPOINT_SIGNATURE_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                    trace_id: request
                        .headers()
                        .get(TRACE_ID_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                });
                (
                    StatusCode::OK,
                    [(GATEWAY_HEADER, "python-support-upstream")],
                    Body::from("{\"object\":\"list\",\"data\":[]}"),
                )
            }
        }),
    );

    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key("sk-test-models")),
        sample_currently_usable_auth_snapshot("key-models-1", "user-models-1"),
    )]));

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new(upstream_url)
            .expect("gateway should build")
            .with_auth_api_key_data_reader_for_tests(auth_repository),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/v1/models?limit=20"))
        .header(http::header::AUTHORIZATION, "Bearer sk-test-models")
        .header(TRACE_ID_HEADER, "trace-models-123")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTROL_ROUTE_CLASS_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("public_support")
    );
    assert_eq!(
        response
            .headers()
            .get(CONTROL_LEGACY_EXECUTION_RUNTIME_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("false")
    );
    assert_eq!(
        response
            .headers()
            .get(CONTROL_EXECUTION_RUNTIME_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("false")
    );

    let seen_support_request = seen_support
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("support request should be captured");
    assert_eq!(seen_support_request.control_route_class, "public_support");
    assert_eq!(seen_support_request.control_route_family, "models");
    assert_eq!(seen_support_request.control_route_kind, "list");
    assert_eq!(
        seen_support_request.legacy_control_execution_runtime_candidate,
        "false"
    );
    assert_eq!(
        seen_support_request.control_execution_runtime_candidate,
        "false"
    );
    assert_eq!(
        seen_support_request.control_endpoint_signature,
        "openai:chat"
    );
    assert_eq!(seen_support_request.trace_id, "trace-models-123");

    gateway_handle.abort();
    upstream_handle.abort();
}
