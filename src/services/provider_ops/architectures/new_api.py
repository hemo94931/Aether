"""
New API 架构

针对 New API 风格的中转站优化的预设配置。
"""

from typing import Any, Dict, List, Optional, Type

import httpx

from src.services.provider_ops.actions import BalanceAction, CheckinAction, ProviderAction
from src.services.provider_ops.architectures.base import ProviderArchitecture, ProviderConnector
from src.services.provider_ops.types import ConnectorAuthType, ProviderActionType


class NewApiConnector(ProviderConnector):
    """
    New API 专用连接器

    特点：
    - 使用 Bearer Token 认证
    - 需要 New-Api-User Header 传递用户 ID
    """

    auth_type = ConnectorAuthType.API_KEY
    display_name = "New API Key"

    def __init__(self, base_url: str, config: Optional[Dict[str, Any]] = None):
        super().__init__(base_url, config)
        self._api_key: Optional[str] = None
        self._user_id: Optional[str] = None

    async def connect(self, credentials: Dict[str, Any]) -> bool:
        """建立连接"""
        api_key = credentials.get("api_key")
        if not api_key:
            self._set_error("API Key 不能为空")
            return False

        user_id = credentials.get("user_id")
        if not user_id:
            self._set_error("用户 ID 不能为空")
            return False

        self._api_key = api_key
        self._user_id = str(user_id)
        self._set_connected()
        return True

    async def disconnect(self) -> None:
        """断开连接"""
        self._api_key = None
        self._user_id = None
        self._set_disconnected()

    async def is_authenticated(self) -> bool:
        """检查是否已认证"""
        return self._api_key is not None and self._user_id is not None

    def _apply_auth(self, request: httpx.Request) -> httpx.Request:
        """为请求应用认证信息"""
        if self._api_key:
            request.headers["Authorization"] = f"Bearer {self._api_key}"
        if self._user_id:
            request.headers["New-Api-User"] = self._user_id
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
                    "description": "New API 的访问令牌",
                },
                "user_id": {
                    "type": "string",
                    "title": "用户 ID",
                    "description": "New API 用户 ID，用于 New-Api-User Header",
                },
            },
            "required": ["api_key", "user_id"],
        }


class NewApiArchitecture(ProviderArchitecture):
    """
    New API 架构预设

    针对 New API 风格的中转站优化的预设配置。

    特点：
    - 使用 Bearer Token 认证
    - 需要 New-Api-User Header 传递用户 ID
    - 验证端点: /api/user/self
    - quota 单位通常是 1/500000 美元
    """

    architecture_id = "new_api"
    display_name = "New API"
    description = "New API 风格中转站的预设配置"

    supported_connectors: List[Type[ProviderConnector]] = [
        NewApiConnector,
    ]

    supported_actions: List[Type[ProviderAction]] = [
        BalanceAction,
        CheckinAction,
    ]

    default_action_configs: Dict[ProviderActionType, Dict[str, Any]] = {
        ProviderActionType.QUERY_BALANCE: {
            "endpoint": "/api/user/self",
            "method": "GET",
            "quota_divisor": 500000,  # New API 的 quota 单位是 1/500000 美元
            "response_mapping": {
                "total_granted": "data.quota",
                "total_used": "data.used_quota",
                "total_available": "data.quota",  # New API 通常只返回剩余额度
            },
        },
        ProviderActionType.CHECKIN: {
            "endpoint": "/api/user/checkin",
            "method": "POST",
            "success_field": "success",
            "message_field": "message",
        },
    }

    def get_credentials_schema(self) -> Dict[str, Any]:
        """New API 需要 api_key 和 user_id"""
        return NewApiConnector.get_credentials_schema()

    def build_verify_headers(
        self,
        config: Dict[str, Any],
        credentials: Dict[str, Any],
    ) -> Dict[str, str]:
        """
        构建 New API 的验证请求 Headers

        New API 特有：需要 New-Api-User Header 传递用户 ID
        """
        headers = super().build_verify_headers(config, credentials)

        # New API 特有的 header
        user_id = credentials.get("user_id", "")
        if user_id:
            headers["New-Api-User"] = str(user_id)

        return headers
