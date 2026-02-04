"""
OpenAI CLI Adapter - 基于通用 CLI Adapter 基类的简化实现

继承 CliAdapterBase，只需配置 FORMAT_ID 和 HANDLER_CLASS。
"""

from __future__ import annotations

import uuid
from typing import Any

import httpx

from src.api.handlers.base.cli_adapter_base import CliAdapterBase, register_cli_adapter
from src.api.handlers.base.cli_handler_base import CliMessageHandlerBase
from src.api.handlers.openai.adapter import OpenAIChatAdapter
from src.config.settings import config
from src.core.api_format import ApiFamily


@register_cli_adapter
class OpenAICliAdapter(CliAdapterBase):
    """
    OpenAI CLI API 适配器

    处理 /v1/responses 端点的请求。
    """

    FORMAT_ID = "openai:cli"
    API_FAMILY = ApiFamily.OPENAI
    BILLING_TEMPLATE = "openai"  # 使用 OpenAI 计费模板
    name = "openai.cli"

    @property
    def HANDLER_CLASS(self) -> type[CliMessageHandlerBase]:
        """延迟导入 Handler 类避免循环依赖"""
        from src.api.handlers.openai_cli.handler import OpenAICliMessageHandler

        return OpenAICliMessageHandler

    def __init__(self, allowed_api_formats: list[str] | None = None):
        super().__init__(allowed_api_formats)

    # =========================================================================
    # 模型列表查询
    # =========================================================================

    @classmethod
    async def fetch_models(
        cls,
        client: httpx.AsyncClient,
        base_url: str,
        api_key: str,
        extra_headers: dict[str, str] | None = None,
    ) -> tuple[list, str | None]:
        """查询 OpenAI 兼容 API 支持的模型列表（带 CLI User-Agent）"""
        # 复用 OpenAIChatAdapter 的实现，添加 CLI User-Agent
        cli_headers = {"User-Agent": config.internal_user_agent_openai_cli}
        if extra_headers:
            cli_headers.update(extra_headers)
        models, error = await OpenAIChatAdapter.fetch_models(client, base_url, api_key, cli_headers)
        # 更新 api_format 为 CLI 格式
        for m in models:
            m["api_format"] = cls.FORMAT_ID
        return models, error

    @classmethod
    def build_endpoint_url(
        cls, base_url: str, request_data: dict[str, Any], model_name: str | None = None
    ) -> str:
        """构建OpenAI CLI API端点URL（使用 Responses API）

        对于 Codex OAuth 端点（如 chatgpt.com/backend-api/codex），直接追加 /responses；
        对于标准 OpenAI API，使用 /v1/responses。
        """
        base_url = base_url.rstrip("/")
        # Codex OAuth 端点：chatgpt.com/backend-api/codex -> /responses
        if cls._is_codex_url(base_url):
            return f"{base_url}/responses"
        # 标准 OpenAI API
        if base_url.endswith("/v1"):
            return f"{base_url}/responses"
        else:
            return f"{base_url}/v1/responses"

    @classmethod
    def _is_codex_url(cls, base_url: str) -> bool:
        """判断是否是 Codex OAuth 端点"""
        return "/backend-api/codex" in base_url or base_url.endswith("/codex")

    # build_request_body 使用基类实现
    # OpenAI CLI normalizer 会自动添加 instructions 字段

    @classmethod
    def build_request_body(
        cls,
        request_data: dict[str, Any] | None = None,
        *,
        base_url: str | None = None,
    ) -> dict[str, Any]:
        """构建测试请求体（Codex 端点需要强制 stream=true 等特性）"""
        from src.api.handlers.base.request_builder import build_test_request_body

        target_variant = "codex" if base_url and cls._is_codex_url(base_url) else None
        return build_test_request_body(
            cls.FORMAT_ID,
            request_data,
            target_variant=target_variant,
        )

    @classmethod
    def get_cli_user_agent(cls) -> str | None:
        """获取OpenAI CLI User-Agent"""
        return config.internal_user_agent_openai_cli

    @classmethod
    def get_cli_extra_headers(cls, *, base_url: str | None = None) -> dict[str, str]:
        """
        获取额外请求头

        对于 Codex OAuth 端点，添加特定头部（缺少可能导致 Cloudflare 拦截）。
        对于标准 OpenAI API 端点，仅添加 User-Agent。
        """
        headers: dict[str, str] = {}

        # User-Agent
        cli_user_agent = cls.get_cli_user_agent()
        if cli_user_agent:
            headers["User-Agent"] = cli_user_agent

        # 仅 Codex 端点添加特定头部
        if base_url and cls._is_codex_url(base_url):
            headers["x-oai-web-search-eligible"] = "true"
            headers["session_id"] = str(uuid.uuid4())
            headers["accept"] = "text/event-stream"
            headers["originator"] = "codex_cli_rs"

        return headers


__all__ = ["OpenAICliAdapter"]
