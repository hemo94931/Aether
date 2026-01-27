"""
格式兼容性检查

用于候选筛选时判断端点是否可以处理客户端请求格式。

转换逻辑：
1. 格式完全匹配 -> 透传（无需转换）
2. 同族格式透传（CLAUDE/CLAUDE_CLI、GEMINI/GEMINI_CLI 格式相同，只是认证不同）
3. 需要转换的情况 -> 检查全局开关 + 端点配置 + 转换器能力
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Optional, Tuple

if TYPE_CHECKING:
    from src.core.api_format.conversion.registry import FormatConversionRegistry

from src.core.api_format.utils import get_base_format

logger = logging.getLogger(__name__)


def is_format_compatible(
    client_format: str,
    endpoint_api_format: str,
    endpoint_format_acceptance_config: Optional[dict],
    is_stream: bool,
    global_conversion_enabled: bool,
    registry: Optional["FormatConversionRegistry"] = None,
) -> Tuple[bool, bool, Optional[str]]:
    """
    检查端点是否兼容客户端格式

    Args:
        client_format: 客户端请求格式
        endpoint_api_format: 端点的 API 格式
        endpoint_format_acceptance_config: 端点的格式接受配置
        is_stream: 是否是流式请求
        global_conversion_enabled: 全局格式转换开关（来自环境变量 FORMAT_CONVERSION_ENABLED，默认 True）
        registry: 转换器注册表（可选，默认使用全局单例）

    Returns:
        (is_compatible, needs_conversion, skip_reason)
        - is_compatible: 是否兼容
        - needs_conversion: 是否需要转换
        - skip_reason: 不兼容时的原因
    """
    # 延迟导入避免循环依赖
    if registry is None:
        from src.core.api_format.conversion.registry import (
            format_conversion_registry,
            register_default_normalizers,
        )

        register_default_normalizers()
        registry = format_conversion_registry

    provider_format = endpoint_api_format.upper()
    client_format_upper = client_format.upper()

    # 1. 格式完全匹配 -> 透传（无需转换）
    if provider_format == client_format_upper:
        return True, False, None

    # 2. 同族格式检查
    provider_base = get_base_format(provider_format)
    client_base = get_base_format(client_format_upper)

    is_same_family = provider_base == client_base
    if is_same_family and provider_base != "OPENAI":
        # CLAUDE/CLAUDE_CLI、GEMINI/GEMINI_CLI 等：格式相同，只是认证不同，可透传
        # 注意：这些格式之间没有定义转换器，因为数据格式完全相同
        return True, False, None

    # 3. 需要转换的情况（OPENAI/OPENAI_CLI 同族转换 或 跨格式转换）
    # 检查全局开关（来自环境变量，默认开启）
    if not global_conversion_enabled:
        return False, False, "全局格式转换未启用（环境变量 FORMAT_CONVERSION_ENABLED=false）"

    # 4. 检查端点配置（核心控制）
    if endpoint_format_acceptance_config is None:
        return False, False, "端点未配置格式转换"

    config = endpoint_format_acceptance_config
    if not isinstance(config, dict):
        return False, False, "端点格式配置无效"
    if not config.get("enabled", False):
        return False, False, "端点格式转换未启用"

    # 检查 reject_formats（优先）
    reject_formats = config.get("reject_formats", [])
    if client_format_upper in [f.upper() for f in reject_formats]:
        return False, False, f"端点拒绝 {client_format} 格式"

    # 检查 accept_formats
    accept_formats = config.get("accept_formats", [])
    if accept_formats and client_format_upper not in [f.upper() for f in accept_formats]:
        return False, False, f"端点不接受 {client_format} 格式"

    # 检查流式转换
    if is_stream and not config.get("stream_conversion", True):
        return False, False, "端点不支持流式格式转换"

    # 5. 检查转换器能力
    if not registry.can_convert_full(
        client_format_upper,
        provider_format,
        require_stream=is_stream,
    ):
        return False, False, f"不存在 {client_format} <-> {provider_format} 的完整转换器"

    return True, True, None


__all__ = [
    "is_format_compatible",
]
