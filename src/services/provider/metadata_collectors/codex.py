"""
Codex Provider 元数据采集器

从响应头解析 Codex 额度/限流信息：
- x-codex-plan-type: 套餐类型
- x-codex-primary-*: 主限额窗口（通常 7 天）
- x-codex-secondary-*: 次级限额窗口（通常 5 小时）
- x-codex-credits-*: 积分信息
"""

from typing import Any, ClassVar

from src.services.provider.metadata_collectors import MetadataCollector


def _safe_float(value: str | None) -> float | None:
    if value is None:
        return None
    try:
        return float(value)
    except (ValueError, TypeError):
        return None


def _safe_int(value: str | None) -> int | None:
    if value is None:
        return None
    try:
        return int(float(value))
    except (ValueError, TypeError):
        return None


def _safe_bool(value: str | None) -> bool | None:
    if value is None:
        return None
    return value.lower() in ("true", "1", "yes")


class CodexMetadataCollector(MetadataCollector):
    """Codex 额度/限流元数据采集器"""

    # 支持 codex 类型，通过响应头判断是否是 Codex
    PROVIDER_TYPES: ClassVar[list[str]] = ["codex"]

    def parse_headers(self, headers: dict[str, str]) -> dict[str, Any] | None:
        # 大小写不敏感查找
        lower_headers = {k.lower(): v for k, v in headers.items()}

        plan_type = lower_headers.get("x-codex-plan-type")
        if plan_type is None:
            # 没有 Codex 特征头，跳过
            return None

        result: dict[str, Any] = {"plan_type": plan_type}

        # 主限额窗口（7 天）
        primary_used = _safe_float(lower_headers.get("x-codex-primary-used-percent"))
        if primary_used is not None:
            result["primary_used_percent"] = primary_used
        primary_reset_seconds = _safe_int(lower_headers.get("x-codex-primary-reset-after-seconds"))
        if primary_reset_seconds is not None:
            result["primary_reset_seconds"] = primary_reset_seconds
        primary_reset_at = _safe_int(lower_headers.get("x-codex-primary-reset-at"))
        if primary_reset_at is not None:
            result["primary_reset_at"] = primary_reset_at
        primary_window = _safe_int(lower_headers.get("x-codex-primary-window-minutes"))
        if primary_window is not None:
            result["primary_window_minutes"] = primary_window

        # 次级限额窗口（5 小时）
        secondary_used = _safe_float(lower_headers.get("x-codex-secondary-used-percent"))
        if secondary_used is not None:
            result["secondary_used_percent"] = secondary_used
        secondary_reset_seconds = _safe_int(
            lower_headers.get("x-codex-secondary-reset-after-seconds")
        )
        if secondary_reset_seconds is not None:
            result["secondary_reset_seconds"] = secondary_reset_seconds
        secondary_reset_at = _safe_int(lower_headers.get("x-codex-secondary-reset-at"))
        if secondary_reset_at is not None:
            result["secondary_reset_at"] = secondary_reset_at
        secondary_window = _safe_int(lower_headers.get("x-codex-secondary-window-minutes"))
        if secondary_window is not None:
            result["secondary_window_minutes"] = secondary_window

        # 积分信息
        has_credits = _safe_bool(lower_headers.get("x-codex-credits-has-credits"))
        if has_credits is not None:
            result["has_credits"] = has_credits
        credits_balance = _safe_float(lower_headers.get("x-codex-credits-balance"))
        if credits_balance is not None:
            result["credits_balance"] = credits_balance

        return result
