use super::{
    json_object, ProviderOpsActionSpec, ProviderOpsArchitectureSpec, ProviderOpsAuthSpec,
    ProviderOpsBalanceMode, ProviderOpsCheckinMode, ProviderOpsVerifyMode,
};
use serde_json::{json, Map, Value};

pub(super) fn spec() -> ProviderOpsArchitectureSpec {
    let credentials_schema = json!({
        "type": "object",
        "properties": {
            "base_url": {
                "type": "string",
                "title": "站点地址",
                "description": "API 基础地址",
                "x-default-value": "https://cubence.com"
            },
            "token_cookie": {
                "type": "string",
                "title": "Cookie",
                "description": "支持粘贴完整 Cookie Header，至少包含 token；若站点启用 Cloudflare，请一并包含 cf_clearance。也兼容仅填写 token 值",
                "x-sensitive": true,
                "x-input-type": "password"
            }
        },
        "required": ["token_cookie"],
        "x-auth-type": "cookie",
        "x-balance-extra-format": [
            {
                "label": "5h",
                "source": "five_hour_limit",
                "type": "window_limit",
                "unit_divisor": 1000000
            },
            {
                "label": "周",
                "source": "weekly_limit",
                "type": "window_limit",
                "unit_divisor": 1000000
            }
        ],
        "x-currency": "USD",
        "x-default-base-url": "https://cubence.com",
        "x-field-groups": [
            { "fields": ["base_url"] },
            { "fields": ["token_cookie"] }
        ],
        "x-quota-divisor": null,
        "x-validation": [
            {
                "type": "required",
                "fields": ["token_cookie"],
                "message": "请填写 Cubence Cookie"
            }
        ]
    });

    ProviderOpsArchitectureSpec {
        architecture_id: "cubence",
        display_name: "Cubence",
        description: "Cubence 中转站预设配置，使用 Cookie 认证",
        hidden: false,
        credentials_schema: credentials_schema.clone(),
        verify_endpoint: "/api/v1/dashboard/overview",
        verify_mode: ProviderOpsVerifyMode::DirectGet,
        balance_mode: ProviderOpsBalanceMode::SingleRequest,
        checkin_mode: ProviderOpsCheckinMode::None,
        query_balance_cookie_auth_errors: true,
        supported_auth_types: vec![ProviderOpsAuthSpec {
            auth_type: "cookie",
            display_name: "Cubence Cookie",
            credentials_schema,
        }],
        supported_actions: vec![ProviderOpsActionSpec {
            action_type: "query_balance",
            display_name: "查询余额（含窗口限额）",
            description: "查询账户余额和窗口限额信息",
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
            "endpoint": "/api/v1/dashboard/overview",
            "method": "GET",
            "currency": "USD"
        }))),
        _ => None,
    }
}
