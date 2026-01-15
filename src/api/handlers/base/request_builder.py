"""
请求构建器 - 透传模式

透传模式 (Passthrough): CLI 和 Chat 等场景，原样转发请求体和头部
- 清理敏感头部：authorization, x-api-key, host, content-length 等
- 保留所有其他头部和请求体字段
- 适用于：Claude CLI、OpenAI CLI、Chat API 等场景

使用方式：
    builder = PassthroughRequestBuilder()
    payload, headers = builder.build(original_body, original_headers, endpoint, key)
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Any, Dict, FrozenSet, Optional, Tuple

from src.core.crypto import crypto_service
from src.core.headers import HeaderBuilder, UPSTREAM_DROP_HEADERS

# ==============================================================================
# 统一的头部配置常量
# ==============================================================================

# 兼容别名：历史代码使用 SENSITIVE_HEADERS 命名
SENSITIVE_HEADERS: FrozenSet[str] = UPSTREAM_DROP_HEADERS


# ==============================================================================
# 请求构建器
# ==============================================================================


class RequestBuilder(ABC):
    """请求构建器抽象基类"""

    @abstractmethod
    def build_payload(
        self,
        original_body: Dict[str, Any],
        *,
        mapped_model: Optional[str] = None,
        is_stream: bool = False,
    ) -> Dict[str, Any]:
        """构建请求体"""
        pass

    @abstractmethod
    def build_headers(
        self,
        original_headers: Dict[str, str],
        endpoint: Any,
        key: Any,
        *,
        extra_headers: Optional[Dict[str, str]] = None,
    ) -> Dict[str, str]:
        """构建请求头"""
        pass

    def build(
        self,
        original_body: Dict[str, Any],
        original_headers: Dict[str, str],
        endpoint: Any,
        key: Any,
        *,
        mapped_model: Optional[str] = None,
        is_stream: bool = False,
        extra_headers: Optional[Dict[str, str]] = None,
    ) -> Tuple[Dict[str, Any], Dict[str, str]]:
        """
        构建完整的请求（请求体 + 请求头）

        Returns:
            Tuple[payload, headers]
        """
        payload = self.build_payload(
            original_body,
            mapped_model=mapped_model,
            is_stream=is_stream,
        )
        headers = self.build_headers(
            original_headers,
            endpoint,
            key,
            extra_headers=extra_headers,
        )
        return payload, headers


class PassthroughRequestBuilder(RequestBuilder):
    """
    透传模式请求构建器

    适用于 CLI 等场景，尽量保持请求原样：
    - 请求体：直接复制，只修改必要字段（model, stream）
    - 请求头：清理敏感头部（黑名单），透传其他所有头部
    """

    def build_payload(
        self,
        original_body: Dict[str, Any],
        *,
        mapped_model: Optional[str] = None,  # noqa: ARG002 - 由 apply_mapped_model 处理
        is_stream: bool = False,  # noqa: ARG002 - 保留原始值，不自动添加
    ) -> Dict[str, Any]:
        """
        透传请求体 - 原样复制，不做任何修改

        透传模式下：
        - model: 由各 handler 的 apply_mapped_model 方法处理
        - stream: 保留客户端原始值（不同 API 处理方式不同）
        """
        return dict(original_body)

    def build_headers(
        self,
        original_headers: Dict[str, str],
        endpoint: Any,
        key: Any,
        *,
        extra_headers: Optional[Dict[str, str]] = None,
    ) -> Dict[str, str]:
        """
        透传请求头 - 清理敏感头部（黑名单），透传其他所有头部
        """
        from src.core.api_format_metadata import get_auth_config, resolve_api_format

        # 1. 根据 API 格式自动设置认证头
        decrypted_key = crypto_service.decrypt(key.api_key)
        api_format = getattr(endpoint, "api_format", None)
        resolved_format = resolve_api_format(api_format)
        auth_header, auth_type = (
            get_auth_config(resolved_format) if resolved_format else ("Authorization", "bearer")
        )

        auth_value = f"Bearer {decrypted_key}" if auth_type == "bearer" else decrypted_key
        protected_keys = {auth_header.lower(), "content-type"}

        builder = HeaderBuilder()

        # 2. 透传原始头部（排除敏感头部 - 黑名单模式）
        if original_headers:
            for name, value in original_headers.items():
                if name.lower() in SENSITIVE_HEADERS:
                    continue
                builder.add(name, value)

        # 3. 添加 endpoint 配置的额外头部（不能覆盖认证头/Content-Type）
        if endpoint.headers:
            builder.add_protected(endpoint.headers, protected_keys)

        # 4. 添加额外头部
        if extra_headers:
            builder.add_many(extra_headers)

        # 5. 设置认证头（最高优先级）
        builder.add(auth_header, auth_value)

        # 6. 确保有 Content-Type
        headers = builder.build()
        if not any(k.lower() == "content-type" for k in headers):
            headers["Content-Type"] = "application/json"

        return headers


# ==============================================================================
# 便捷函数
# ==============================================================================


def build_passthrough_request(
    original_body: Dict[str, Any],
    original_headers: Dict[str, str],
    endpoint: Any,
    key: Any,
) -> Tuple[Dict[str, Any], Dict[str, str]]:
    """
    构建透传模式的请求

    纯透传：原样复制请求体，只处理请求头（认证等）。
    model mapping 和 stream 由调用方自行处理（不同 API 格式处理方式不同）。
    """
    builder = PassthroughRequestBuilder()
    return builder.build(
        original_body,
        original_headers,
        endpoint,
        key,
    )
