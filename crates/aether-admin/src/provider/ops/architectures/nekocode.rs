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
                "x-default-value": "https://nekocode.ai"
            },
            "session_cookie": {
                "type": "string",
                "title": "Session Cookie",
                "description": "从浏览器复制的 session Cookie 值",
                "x-sensitive": true,
                "x-input-type": "password"
            }
        },
        "required": ["session_cookie"],
        "x-auth-type": "cookie",
        "x-balance-extra-format": [
            {
                "label": "天",
                "source_limit": "daily_quota_limit",
                "source_remaining": "daily_remaining_quota",
                "source_start_date": "effective_start_date",
                "type": "daily_quota"
            },
            {
                "label": "月",
                "source_end_date": "effective_end_date",
                "type": "monthly_expiry"
            }
        ],
        "x-currency": "USD",
        "x-default-base-url": "https://nekocode.ai",
        "x-field-groups": [
            { "fields": ["base_url"] },
            { "fields": ["session_cookie"] }
        ],
        "x-quota-divisor": null,
        "x-validation": [
            {
                "type": "required",
                "fields": ["session_cookie"],
                "message": "请填写 Session Cookie"
            }
        ]
    });

    ProviderOpsArchitectureSpec {
        architecture_id: "nekocode",
        display_name: "NekoCode",
        description: "NekoCode 中转站预设配置，使用 Cookie 认证",
        hidden: false,
        credentials_schema: credentials_schema.clone(),
        verify_endpoint: "/api/user/self",
        verify_mode: ProviderOpsVerifyMode::DirectGet,
        balance_mode: ProviderOpsBalanceMode::SingleRequest,
        checkin_mode: ProviderOpsCheckinMode::None,
        query_balance_cookie_auth_errors: true,
        supported_auth_types: vec![ProviderOpsAuthSpec {
            auth_type: "cookie",
            display_name: "NekoCode Cookie",
            credentials_schema,
        }],
        supported_actions: vec![ProviderOpsActionSpec {
            action_type: "query_balance",
            display_name: "查询余额",
            description: "查询 NekoCode 账户余额和订阅信息",
            config_schema: json!({
                "type": "object",
                "properties": {
                    "endpoint": {
                        "type": "string",
                        "title": "API 端点",
                        "default": "/api/usage/summary"
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
            "endpoint": "/api/usage/summary",
            "method": "GET",
            "currency": "USD"
        }))),
        _ => None,
    }
}
