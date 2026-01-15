"""
统一的请求头处理模块

职责：
1. 请求头规范化（大小写统一）
2. 客户端 API Key 提取
3. 能力需求检测
4. 上游请求头构建
5. 响应头过滤
6. 日志脱敏
"""

from __future__ import annotations

from typing import AbstractSet, Any, Dict, FrozenSet, Optional, Set

from .api_format_metadata import get_auth_config, get_extra_headers, get_protected_keys
from .enums import APIFormat


# =============================================================================
# 头部常量定义
# =============================================================================

# 转发给上游时需要剔除的头部（系统管理 + 认证替换）
UPSTREAM_DROP_HEADERS: FrozenSet[str] = frozenset(
    {
        # 认证头 - 会被替换为 Provider 的认证
        "authorization",
        "x-api-key",
        "x-goog-api-key",
        # 系统管理头 - 由 HTTP 客户端重新生成
        "host",
        "content-length",
        "transfer-encoding",
        "connection",
        # 编码头 - 避免客户端请求 brotli/zstd 但 httpx 不支持
        "accept-encoding",
    }
)

# 最小必脱敏集合（编译时常量，用于快速路径）
# 完整脱敏应使用 SystemConfigService.get_sensitive_headers()
CORE_REDACT_HEADERS: FrozenSet[str] = frozenset(
    {
        "authorization",
        "x-api-key",
        "x-goog-api-key",
    }
)

# Hop-by-hop 头部 (RFC 7230)
HOP_BY_HOP_HEADERS: FrozenSet[str] = frozenset(
    {
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    }
)

# 响应时需要过滤的头部（body-dependent + hop-by-hop）
RESPONSE_DROP_HEADERS: FrozenSet[str] = (
    frozenset(
        {
            "content-length",
            "content-encoding",
            "transfer-encoding",
            "content-type",
        }
    )
    | HOP_BY_HOP_HEADERS
)


# =============================================================================
# 请求头规范化
# =============================================================================


def normalize_headers(headers: Dict[str, str]) -> Dict[str, str]:
    """
    将请求头 key 统一为小写

    用于处理 context.original_headers 的大小写敏感问题。
    """

    return {k.lower(): v for k, v in headers.items()}


def get_header_value(headers: Dict[str, str], key: str, default: str = "") -> str:
    """
    大小写不敏感地获取请求头值

    Args:
        headers: 原始请求头（可能大小写不一致）
        key: 要获取的 key（任意大小写）
        default: 未找到时的默认值

    Returns:
        头部值，未找到返回 default
    """

    key_lower = key.lower()
    for k, v in headers.items():
        if k.lower() == key_lower:
            return v
    return default


# =============================================================================
# 客户端 API Key 提取
# =============================================================================


def extract_client_api_key(headers: Dict[str, str], api_format: APIFormat) -> Optional[str]:
    """
    从客户端请求头提取 API Key

    自动处理大小写，根据 API 格式使用正确的认证头和类型。

    Args:
        headers: 原始请求头（自动处理大小写）
        api_format: API 格式

    Returns:
        提取的 API Key，未找到返回 None
    """

    auth_header, auth_type = get_auth_config(api_format)
    value = get_header_value(headers, auth_header)
    if not value:
        return None

    if auth_type == "bearer":
        # Bearer token 格式: "Bearer <token>"
        if value.lower().startswith("bearer "):
            return value[7:]  # 移除 "Bearer " 前缀
        return None

    # 直接 header 格式
    return value


# =============================================================================
# 能力需求检测
# =============================================================================


def detect_capabilities(
    headers: Dict[str, str],
    api_format: APIFormat,
    request_body: Optional[Dict[str, Any]] = None,  # noqa: ARG001 - 预留给部分格式使用
) -> Dict[str, bool]:
    """
    从请求头检测能力需求

    当前支持:
    - Claude/Claude CLI: anthropic-beta 头中的 context-1m

    Args:
        headers: 原始请求头（自动处理大小写）
        api_format: API 格式
        request_body: 请求体（部分格式可能需要）

    Returns:
        能力需求字典，如 {"context_1m": True}
    """

    requirements: Dict[str, bool] = {}

    if api_format in (APIFormat.CLAUDE, APIFormat.CLAUDE_CLI):
        beta_header = get_header_value(headers, "anthropic-beta")
        if "context-1m" in beta_header.lower():
            requirements["context_1m"] = True

    return requirements


# =============================================================================
# 上游请求头构建
# =============================================================================


class HeaderBuilder:
    """
    请求头构建器

    使用 lower-case key 索引确保唯一性和确定的优先级。
    优先级（后者覆盖前者）：原始头部 < endpoint 头部 < extra 头部 < 认证头
    """

    def __init__(self) -> None:
        # key: (original_case_key, value)
        self._headers: Dict[str, tuple[str, str]] = {}

    def add(self, key: str, value: str) -> "HeaderBuilder":
        """添加单个头部（会覆盖同名头部）"""
        self._headers[key.lower()] = (key, value)
        return self

    def add_many(self, headers: Dict[str, str]) -> "HeaderBuilder":
        """批量添加头部"""
        for k, v in headers.items():
            self.add(k, v)
        return self

    def add_protected(self, headers: Dict[str, str], protected_keys: AbstractSet[str]) -> "HeaderBuilder":
        """
        添加头部但保护指定的 key 不被覆盖

        用于 endpoint.headers 不能覆盖认证头的场景。
        """
        protected_lower = {k.lower() for k in protected_keys}
        for k, v in headers.items():
            if k.lower() not in protected_lower:
                self.add(k, v)
        return self

    def remove(self, keys: FrozenSet[str]) -> "HeaderBuilder":
        """移除指定的头部"""
        for k in keys:
            self._headers.pop(k.lower(), None)
        return self

    def build(self) -> Dict[str, str]:
        """构建最终的头部字典"""
        return {original_key: value for original_key, value in self._headers.values()}


def build_upstream_headers(
    original_headers: Dict[str, str],
    api_format: APIFormat,
    provider_api_key: str,
    *,
    endpoint_headers: Optional[Dict[str, str]] = None,
    extra_headers: Optional[Dict[str, str]] = None,
    drop_headers: Optional[FrozenSet[str]] = None,
) -> Dict[str, str]:
    """
    构建发送给上游 Provider 的请求头

    优先级（后者覆盖前者）：
    1. 原始头部（排除 drop_headers）
    2. endpoint 配置头部
    3. extra_headers
    4. 认证头（最高优先级，始终设置）

    Args:
        original_headers: 客户端原始请求头
        api_format: API 格式
        provider_api_key: Provider 的 API Key（已解密）
        endpoint_headers: Endpoint 配置的额外头部
        extra_headers: 调用方传入的额外头部
        drop_headers: 需要剔除的头部集合（None 使用默认值，空集合表示不剔除）

    Returns:
        构建好的请求头字典
    """

    # 使用 is None 判断，允许显式传空集合
    if drop_headers is None:
        drop_headers = UPSTREAM_DROP_HEADERS

    auth_header, auth_type = get_auth_config(api_format)
    auth_value = f"Bearer {provider_api_key}" if auth_type == "bearer" else provider_api_key

    # 认证头是受保护的，不能被 endpoint_headers 覆盖
    protected_keys = {auth_header.lower(), "content-type"}

    builder = HeaderBuilder()

    # 1. 添加原始头部（排除 drop_headers）
    for k, v in original_headers.items():
        if k.lower() not in drop_headers:
            builder.add(k, v)

    # 2. 添加 endpoint 头部（保护认证头）
    if endpoint_headers:
        builder.add_protected(endpoint_headers, protected_keys)

    # 3. 添加 extra_headers
    if extra_headers:
        builder.add_many(extra_headers)

    # 4. 设置认证头（最高优先级）
    builder.add(auth_header, auth_value)

    # 5. 确保 Content-Type
    result = builder.build()
    if not any(k.lower() == "content-type" for k in result):
        result["Content-Type"] = "application/json"

    return result


def merge_headers_with_protection(
    base_headers: Dict[str, str],
    extra_headers: Optional[Dict[str, str]],
    protected_keys: FrozenSet[str] | Set[str],
) -> Dict[str, str]:
    """
    合并头部但保护指定的 key 不被覆盖

    等价于原 build_safe_headers 的功能。

    Args:
        base_headers: 基础头部
        extra_headers: 要合并的额外头部
        protected_keys: 受保护的 key 集合

    Returns:
        合并后的头部
    """
    if not extra_headers:
        return dict(base_headers)

    builder = HeaderBuilder()
    builder.add_many(base_headers)
    builder.add_protected(extra_headers, protected_keys)
    return builder.build()


# =============================================================================
# 响应头过滤
# =============================================================================


def filter_response_headers(
    headers: Optional[Dict[str, str]],
    drop_headers: Optional[FrozenSet[str]] = None,
) -> Dict[str, str]:
    """
    过滤上游响应头中不应透传给客户端的字段

    Args:
        headers: 上游响应头
        drop_headers: 要剔除的头部集合（None 使用默认值）

    Returns:
        过滤后的头部
    """
    if not headers:
        return {}

    if drop_headers is None:
        drop_headers = RESPONSE_DROP_HEADERS

    return {k: v for k, v in headers.items() if k.lower() not in drop_headers}


# =============================================================================
# 日志脱敏
# =============================================================================


def redact_headers_for_log(
    headers: Dict[str, str],
    redact_keys: Optional[FrozenSet[str]] = None,
) -> Dict[str, str]:
    """
    将敏感头部值替换为 *** 用于日志记录

    Args:
        headers: 原始头部
        redact_keys: 要脱敏的 key 集合（None 使用 CORE_REDACT_HEADERS）

    Returns:
        脱敏后的头部

    Note:
        完整的脱敏应该使用 SystemConfigService.get_sensitive_headers()
        来获取用户配置的敏感头列表。
    """
    if redact_keys is None:
        redact_keys = CORE_REDACT_HEADERS

    return {k: "***" if k.lower() in redact_keys else v for k, v in headers.items()}


# =============================================================================
# 兼容层（向后兼容，逐步废弃）
# =============================================================================

# 兼容 request_builder.py 的 SENSITIVE_HEADERS
SENSITIVE_HEADERS = UPSTREAM_DROP_HEADERS


# =============================================================================
# Adapter 统一接口
# =============================================================================


def build_adapter_base_headers(
    api_format: APIFormat,
    api_key: str,
    *,
    include_extra: bool = True,
) -> Dict[str, str]:
    """
    根据 API 格式构建基础请求头

    包含：认证头 + Content-Type + 格式特定的额外头部（如 anthropic-version）

    Args:
        api_format: API 格式
        api_key: API Key（已解密）
        include_extra: 是否包含格式特定的额外头部（默认 True）

    Returns:
        基础请求头字典
    """
    auth_header, auth_type = get_auth_config(api_format)
    auth_value = f"Bearer {api_key}" if auth_type == "bearer" else api_key

    headers: Dict[str, str] = {
        auth_header: auth_value,
        "Content-Type": "application/json",
    }

    if include_extra:
        extra = get_extra_headers(api_format)
        if extra:
            headers.update(extra)

    return headers


def build_adapter_headers(
    api_format: APIFormat,
    api_key: str,
    extra_headers: Optional[Dict[str, str]] = None,
) -> Dict[str, str]:
    """
    构建完整的 Adapter 请求头

    在基础头部上合并 extra_headers，同时保护关键头部不被覆盖。

    Args:
        api_format: API 格式
        api_key: API Key（已解密）
        extra_headers: 调用方传入的额外头部

    Returns:
        完整的请求头字典
    """
    base = build_adapter_base_headers(api_format, api_key)

    if not extra_headers:
        return base

    protected = get_protected_keys(api_format)
    return merge_headers_with_protection(base, extra_headers, protected)


def get_adapter_protected_keys(api_format: APIFormat) -> tuple[str, ...]:
    """
    获取 Adapter 的受保护头部 key

    用于 get_protected_header_keys() 方法返回值。

    Args:
        api_format: API 格式

    Returns:
        受保护的头部 key 元组
    """
    return tuple(get_protected_keys(api_format))

