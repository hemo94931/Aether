use super::{
    json_object, ProviderOpsActionSpec, ProviderOpsArchitectureSpec, ProviderOpsAuthSpec,
    ProviderOpsBalanceMode, ProviderOpsCheckinMode, ProviderOpsVerifyMode,
};
use serde_json::{json, Map, Value};

pub(super) fn spec() -> ProviderOpsArchitectureSpec {
    let credentials_schema = json!({
        "type": "object",
        "properties": {
            "auth_cookie": {
                "type": "string",
                "title": "Auth Cookie",
                "description": "从浏览器复制的 Cookie（包含 yescode_auth 和 yescode_csrf）",
                "x-sensitive": true,
                "x-input-type": "password"
            },
            "base_url": {
                "type": "string",
                "title": "站点地址",
                "description": "API 基础地址",
                "x-default-value": "https://co.yes.vg"
            }
        },
        "required": ["auth_cookie"],
        "x-auth-type": "cookie",
        "x-balance-extra-format": [
            {
                "label": "天",
                "type": "weekly_spent",
                "source_limit": "daily_limit",
                "source_spent": "daily_spent",
                "source_resets_at": "daily_resets_at"
            },
            {
                "label": "周",
                "type": "weekly_spent",
                "source_limit": "weekly_limit",
                "source_spent": "weekly_spent",
                "source_resets_at": "weekly_resets_at"
            }
        ],
        "x-currency": "USD",
        "x-default-base-url": "https://co.yes.vg",
        "x-field-groups": [
            { "fields": ["base_url"] },
            { "fields": ["auth_cookie"] }
        ],
        "x-quota-divisor": null,
        "x-validation": [
            {
                "type": "required",
                "fields": ["auth_cookie"],
                "message": "请填写 Auth Cookie"
            }
        ]
    });

    ProviderOpsArchitectureSpec {
        architecture_id: "yescode",
        display_name: "YesCode",
        description: "YesCode 中转站预设配置，使用 Cookie 认证",
        hidden: false,
        credentials_schema: credentials_schema.clone(),
        verify_endpoint: "/api/v1/auth/profile",
        verify_mode: ProviderOpsVerifyMode::DirectGet,
        balance_mode: ProviderOpsBalanceMode::YescodeCombined,
        checkin_mode: ProviderOpsCheckinMode::None,
        query_balance_cookie_auth_errors: true,
        supported_auth_types: vec![ProviderOpsAuthSpec {
            auth_type: "cookie",
            display_name: "YesCode Cookie",
            credentials_schema,
        }],
        supported_actions: vec![ProviderOpsActionSpec {
            action_type: "query_balance",
            display_name: "查询余额（含每周限额）",
            description: "查询账户余额和每周限额信息",
            config_schema: json!({
                "type": "object",
                "properties": {
                    "currency": {
                        "type": "string",
                        "title": "货币单位",
                        "default": "USD"
                    }
                },
                "required": []
            }),
        }],
        default_connector: Some("cookie"),
    }
}

pub(super) fn default_action_config(action_type: &str) -> Option<Map<String, Value>> {
    match action_type {
        "query_balance" => Some(json_object(json!({
            "endpoint": "/api/v1/user/balance",
            "method": "GET",
            "currency": "USD"
        }))),
        _ => None,
    }
}
