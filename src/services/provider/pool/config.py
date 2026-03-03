"""Account Pool configuration (provider-agnostic)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from src.core.logger import logger


@dataclass(frozen=True, slots=True)
class UnschedulableRule:
    """Keyword-based temporary unschedule rule."""

    keyword: str
    duration_minutes: int = 5


@dataclass(frozen=True, slots=True)
class PoolConfig:
    """Parsed pool configuration for any Provider.

    All transient state lives in Redis; this dataclass only holds
    the *configuration* that controls pool behaviour.
    """

    # -- Sticky Session -------------------------------------------------------
    sticky_session_ttl_seconds: int = 3600  # 1 hour
    # Key 优先模式下号池整体优先级（None 时回退 provider_priority）
    global_priority: int | None = None

    # -- Load-Aware Selection -------------------------------------------------
    load_threshold_percent: int = 80

    # -- LRU ------------------------------------------------------------------
    lru_enabled: bool = True

    # -- Rolling-Window Cost Tracking -----------------------------------------
    cost_window_seconds: int = 18000  # 5 hours
    cost_limit_per_key_tokens: int | None = None  # None = unlimited
    cost_soft_threshold_percent: int = 80

    # -- Cooldown Defaults ----------------------------------------------------
    rate_limit_cooldown_seconds: int = 300  # 429
    overload_cooldown_seconds: int = 30  # 529

    # -- OAuth Proactive Refresh ----------------------------------------------
    proactive_refresh_seconds: int = 180  # 3 minutes before expiry

    # -- Health Policy --------------------------------------------------------
    health_policy_enabled: bool = True

    # -- Temporary Unschedulable Rules ----------------------------------------
    unschedulable_rules: list[UnschedulableRule] = field(default_factory=list)

    # -- Stream Timeout Auto-Pause --------------------------------------------
    stream_timeout_threshold: int = 3  # N timeouts within window trigger cooldown
    stream_timeout_window_seconds: int = 1800  # 30 min counting window
    stream_timeout_cooldown_seconds: int = 300  # 5 min cooldown

    # -- Pluggable Strategies -------------------------------------------------
    strategies: tuple[str, ...] = ()


def parse_pool_config(provider_config: Any) -> PoolConfig | None:
    """Parse PoolConfig from ``Provider.config``.

    Only looks for the explicit ``pool_advanced`` key.  Returns ``None``
    when the provider has no pool section configured, meaning the caller
    should use the normal (non-pool) scheduling path.
    """
    config_dict = provider_config if isinstance(provider_config, dict) else {}

    raw_advanced = config_dict.get("pool_advanced")
    if raw_advanced is None:
        return None

    if not isinstance(raw_advanced, dict):
        # Could be a pre-validated Pydantic model; grab its dict.
        try:
            raw_advanced = raw_advanced.model_dump()  # type: ignore[union-attr]
        except Exception:
            logger.warning(
                "PoolConfig: advanced config type invalid ({}), falling back to defaults",
                type(raw_advanced).__name__,
            )
            return PoolConfig()

    rules: list[UnschedulableRule] = []
    raw_rules = raw_advanced.get("unschedulable_rules")
    if isinstance(raw_rules, list):
        for r in raw_rules:
            if isinstance(r, dict) and isinstance(r.get("keyword"), str):
                rules.append(
                    UnschedulableRule(
                        keyword=r["keyword"],
                        duration_minutes=int(r.get("duration_minutes", 5)),
                    )
                )

    def _int_or(key: str, default: int) -> int:
        v = raw_advanced.get(key)
        if v is None:
            return default
        try:
            return int(v)
        except (TypeError, ValueError):
            return default

    def _bool_or(key: str, default: bool) -> bool:
        v = raw_advanced.get(key)
        if v is None:
            return default
        return bool(v)

    def _opt_int(key: str) -> int | None:
        v = raw_advanced.get(key)
        if v is None:
            return None
        try:
            return int(v)
        except (TypeError, ValueError):
            return None

    return PoolConfig(
        sticky_session_ttl_seconds=_int_or("sticky_session_ttl_seconds", 3600),
        global_priority=_opt_int("global_priority"),
        load_threshold_percent=_int_or("load_threshold_percent", 80),
        lru_enabled=_bool_or("lru_enabled", True),
        cost_window_seconds=_int_or("cost_window_seconds", 18000),
        cost_limit_per_key_tokens=_opt_int("cost_limit_per_key_tokens"),
        cost_soft_threshold_percent=_int_or("cost_soft_threshold_percent", 80),
        rate_limit_cooldown_seconds=_int_or("rate_limit_cooldown_seconds", 300),
        overload_cooldown_seconds=_int_or("overload_cooldown_seconds", 30),
        proactive_refresh_seconds=_int_or("proactive_refresh_seconds", 180),
        health_policy_enabled=_bool_or("health_policy_enabled", True),
        unschedulable_rules=rules,
        stream_timeout_threshold=_int_or("stream_timeout_threshold", 3),
        stream_timeout_window_seconds=_int_or("stream_timeout_window_seconds", 1800),
        stream_timeout_cooldown_seconds=_int_or("stream_timeout_cooldown_seconds", 300),
        strategies=_parse_strategies(raw_advanced.get("strategies")),
    )


def _parse_strategies(raw: Any) -> tuple[str, ...]:
    """Parse strategy names from config (list[str] -> tuple[str, ...])."""
    if not isinstance(raw, list):
        return ()
    return tuple(str(s) for s in raw if isinstance(s, str) and s)
