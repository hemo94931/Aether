"""
通用 API 架构

支持各种中转站的可配置架构。

## 添加新认证模板示例

如需添加新的中转站模板（如 MyApi），参考以下步骤：

1. 在 architectures/ 目录创建新文件，如 my_api.py：

    from src.services.provider_ops.architectures.base import ProviderArchitecture
    from src.services.provider_ops.connectors.base import ProviderConnector

    class MyApiConnector(ProviderConnector):
        # 实现自己的连接器
        pass

    class MyApiArchitecture(ProviderArchitecture):
        architecture_id = "my_api"
        display_name = "My API"
        description = "My API 风格中转站"

        supported_connectors = [MyApiConnector]
        supported_actions = [BalanceAction]

        # 如果需要特殊的认证 headers，重写此方法
        def build_verify_headers(self, config, credentials):
            headers = super().build_verify_headers(config, credentials)
            if "custom_field" in credentials:
                headers["X-Custom-Header"] = credentials["custom_field"]
            return headers

2. 在 registry.py 的 _register_builtin_architectures() 中注册：

    from .my_api import MyApiArchitecture
    builtin = [..., MyApiArchitecture]

3. 在前端 auth-templates/ 添加对应的模板定义
"""

from typing import Any, Dict, List, Optional, Type

import httpx

from src.services.provider_ops.actions import BalanceAction, CheckinAction, ProviderAction
from src.services.provider_ops.architectures.base import ProviderArchitecture, ProviderConnector
from src.services.provider_ops.types import ConnectorAuthType, ProviderActionType


class GenericApiKeyConnector(ProviderConnector):
    """
    通用 API Key 连接器

    支持多种 API Key 传递方式：
    - Bearer Token (Authorization: Bearer xxx)
    - Custom Header (X-API-Key: xxx)
    """

    auth_type = ConnectorAuthType.API_KEY
    display_name = "API Key"

    def __init__(self, base_url: str, config: Optional[Dict[str, Any]] = None):
        super().__init__(base_url, config)
        self._api_key: Optional[str] = None
        # 支持配置认证方式
        self._auth_method = self.config.get("auth_method", "bearer")
        self._header_name = self.config.get("header_name", "Authorization")

    async def connect(self, credentials: Dict[str, Any]) -> bool:
        """建立连接"""
        api_key = credentials.get("api_key")
        if not api_key:
            self._set_error("API Key 不能为空")
            return False

        self._api_key = api_key
        self._set_connected()
        return True

    async def disconnect(self) -> None:
        """断开连接"""
        self._api_key = None
        self._set_disconnected()

    async def is_authenticated(self) -> bool:
        """检查是否已认证"""
        return self._api_key is not None

    def _apply_auth(self, request: httpx.Request) -> httpx.Request:
        """为请求应用认证信息"""
        if not self._api_key:
            return request

        if self._auth_method == "bearer":
            request.headers["Authorization"] = f"Bearer {self._api_key}"
        elif self._auth_method == "header":
            request.headers[self._header_name] = self._api_key

        return request

    @classmethod
    def get_credentials_schema(cls) -> Dict[str, Any]:
        """获取凭据配置 schema"""
        return {
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "title": "API Key",
                    "description": "提供商的 API Key",
                },
            },
            "required": ["api_key"],
        }


class GenericApiArchitecture(ProviderArchitecture):
    """
    通用 API 架构

    适用于各种中转站，支持所有认证方式和操作类型。
    用户可以完全自定义 endpoint 和响应映射。

    这是"自定义"模板对应的后端架构。
    """

    architecture_id = "generic_api"
    display_name = "通用 API"
    description = "可配置的通用 API 架构，适用于各种中转站"

    supported_connectors: List[Type[ProviderConnector]] = [
        GenericApiKeyConnector,
    ]

    supported_actions: List[Type[ProviderAction]] = [
        BalanceAction,
        CheckinAction,
    ]

    # 默认操作配置（可被用户配置覆盖）
    default_action_configs: Dict[ProviderActionType, Dict[str, Any]] = {
        ProviderActionType.QUERY_BALANCE: {
            "endpoint": "/api/user/balance",
            "method": "GET",
        },
        ProviderActionType.CHECKIN: {
            "endpoint": "/api/user/checkin",
            "method": "POST",
        },
    }

    def get_credentials_schema(self) -> Dict[str, Any]:
        """通用架构只需要 api_key"""
        return GenericApiKeyConnector.get_credentials_schema()
