"""
One API 架构

针对 One API 风格的中转站优化的预设配置。
"""

from typing import Any, Dict, List, Optional, Type

import httpx

from src.services.provider_ops.actions import BalanceAction, ProviderAction
from src.services.provider_ops.architectures.base import ProviderArchitecture, ProviderConnector
from src.services.provider_ops.types import ConnectorAuthType, ProviderActionType


class OneApiConnector(ProviderConnector):
    """
    One API 专用连接器

    特点：
    - 使用 Bearer Token 认证
    - 不需要额外的 Header
    """

    auth_type = ConnectorAuthType.API_KEY
    display_name = "One API Key"

    def __init__(self, base_url: str, config: Optional[Dict[str, Any]] = None):
        super().__init__(base_url, config)
        self._api_key: Optional[str] = None

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
        if self._api_key:
            request.headers["Authorization"] = f"Bearer {self._api_key}"
        return request

    @classmethod
    def get_credentials_schema(cls) -> Dict[str, Any]:
        """获取凭据配置 schema"""
        return {
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "title": "访问令牌 (API Key)",
                    "description": "One API 的访问令牌",
                },
            },
            "required": ["api_key"],
        }


class OneApiArchitecture(ProviderArchitecture):
    """
    One API 架构预设

    针对 One API 风格的中转站优化的预设配置。

    特点：
    - 使用 Bearer Token 认证
    - 验证端点: /api/user/self
    - 不需要额外的 Header
    """

    architecture_id = "one_api"
    display_name = "One API"
    description = "One API 风格中转站的预设配置"

    supported_connectors: List[Type[ProviderConnector]] = [
        OneApiConnector,
    ]

    supported_actions: List[Type[ProviderAction]] = [
        BalanceAction,
    ]

    default_action_configs: Dict[ProviderActionType, Dict[str, Any]] = {
        ProviderActionType.QUERY_BALANCE: {
            "endpoint": "/api/user/self",
            "method": "GET",
            "response_mapping": {
                "total_granted": "data.quota",
                "total_used": "data.used_quota",
            },
        },
    }

    def get_credentials_schema(self) -> Dict[str, Any]:
        """One API 只需要 api_key"""
        return OneApiConnector.get_credentials_schema()
