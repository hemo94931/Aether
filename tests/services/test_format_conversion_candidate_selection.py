from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest

from src.core.api_format.conversion import register_default_normalizers
from src.services.cache.aware_scheduler import (
    CacheAwareScheduler,
    ProviderCandidate,
    _sort_endpoints_by_family_priority,
)


def _mock_key(key_id: str, api_formats: list[str]) -> MagicMock:
    key = MagicMock()
    key.id = key_id
    key.is_active = True
    key.api_formats = api_formats
    key.cache_ttl_minutes = 1
    key.internal_priority = 1
    return key


def _mock_endpoint(api_format: str, config: dict | None = None) -> MagicMock:
    endpoint = MagicMock()
    endpoint.id = f"ep_{api_format.lower().replace(':', '_')}"
    endpoint.is_active = True
    endpoint.api_format = api_format
    endpoint.api_family = api_format.split(":", 1)[0]
    endpoint.endpoint_kind = api_format.split(":", 1)[1]
    endpoint.format_acceptance_config = config
    return endpoint


@pytest.mark.asyncio
async def test_build_candidates_allows_cross_format_when_endpoint_accepts_and_overrides_off() -> (
    None
):
    register_default_normalizers()

    scheduler = CacheAwareScheduler()
    scheduler._check_model_support = AsyncMock(return_value=(True, None, None, {"m"}))  # type: ignore[method-assign]
    scheduler._check_key_availability = MagicMock(return_value=(True, None, None))  # type: ignore[method-assign]

    provider = MagicMock()
    provider.name = "p1"
    provider.enable_format_conversion = False
    provider.endpoints = [
        _mock_endpoint(
            "openai:chat",
            {"enabled": True, "accept_formats": ["claude:chat"], "stream_conversion": True},
        )
    ]
    provider.api_keys = [_mock_key("k1", ["openai:chat"])]

    candidates = await scheduler._build_candidates(
        db=MagicMock(),
        providers=[provider],
        client_format="claude:chat",
        model_name="dummy-model",
        affinity_key=None,
        global_conversion_enabled=True,  # 全局开关开启
    )

    assert len(candidates) == 1
    assert candidates[0].needs_conversion is True
    assert candidates[0].provider_api_format == "openai:chat"


@pytest.mark.asyncio
async def test_build_candidates_allows_cross_format_when_global_off_but_endpoint_enabled() -> None:
    """
    分层开关设计：全局 OFF 时回退到端点配置
    - 全局 OFF + 端点 enabled=True -> 允许（端点覆盖全局默认）
    """
    register_default_normalizers()

    scheduler = CacheAwareScheduler()
    scheduler._check_model_support = AsyncMock(return_value=(True, None, None, {"m"}))  # type: ignore[method-assign]
    scheduler._check_key_availability = MagicMock(return_value=(True, None, None))  # type: ignore[method-assign]

    provider = MagicMock()
    provider.name = "p1"
    provider.enable_format_conversion = False
    provider.endpoints = [
        _mock_endpoint(
            "openai:chat",
            {"enabled": True, "accept_formats": ["claude:chat"], "stream_conversion": True},
        )
    ]
    provider.api_keys = [_mock_key("k1", ["openai:chat"])]

    candidates = await scheduler._build_candidates(
        db=MagicMock(),
        providers=[provider],
        client_format="claude:chat",
        model_name="dummy-model",
        affinity_key=None,
        global_conversion_enabled=False,  # 全局开关关闭，但端点配置允许
    )

    # 新设计：全局 OFF 时回退到端点配置，端点 enabled=True 则允许
    assert len(candidates) == 1
    assert candidates[0].needs_conversion is True
    assert candidates[0].provider_api_format == "openai:chat"


@pytest.mark.asyncio
async def test_build_candidates_blocks_cross_format_when_global_off_and_endpoint_not_configured() -> (
    None
):
    """
    分层开关设计：全局 OFF + 端点未配置 -> 阻止
    """
    register_default_normalizers()

    scheduler = CacheAwareScheduler()
    scheduler._check_model_support = AsyncMock(return_value=(True, None, None, {"m"}))  # type: ignore[method-assign]
    scheduler._check_key_availability = MagicMock(return_value=(True, None, None))  # type: ignore[method-assign]

    provider = MagicMock()
    provider.name = "p1"
    provider.enable_format_conversion = False
    provider.endpoints = [_mock_endpoint("openai:chat", None)]  # 端点未配置格式接受策略
    provider.api_keys = [_mock_key("k1", ["openai:chat"])]

    candidates = await scheduler._build_candidates(
        db=MagicMock(),
        providers=[provider],
        client_format="claude:chat",
        model_name="dummy-model",
        affinity_key=None,
        global_conversion_enabled=False,  # 全局开关关闭
    )

    # 全局 OFF + 端点未配置 -> 阻止
    assert candidates == []


@pytest.mark.asyncio
async def test_build_candidates_includes_cross_format_when_enabled() -> None:
    register_default_normalizers()

    scheduler = CacheAwareScheduler()
    scheduler._check_model_support = AsyncMock(return_value=(True, None, None, {"m"}))  # type: ignore[method-assign]
    scheduler._check_key_availability = MagicMock(return_value=(True, None, None))  # type: ignore[method-assign]

    provider = MagicMock()
    provider.name = "p1"
    provider.enable_format_conversion = False
    provider.endpoints = [
        # 端点未配置/未启用格式接受策略，但 DB 全局覆盖开启应强制允许
        _mock_endpoint("openai:chat", None)
    ]
    provider.api_keys = [_mock_key("k1", ["openai:chat"])]

    candidates = await scheduler._build_candidates(
        db=MagicMock(),
        providers=[provider],
        client_format="claude:chat",
        model_name="dummy-model",
        affinity_key=None,
        global_conversion_enabled=True,  # 全局开关开启：跳过端点检查
    )

    assert len(candidates) == 1
    assert candidates[0].needs_conversion is True
    assert candidates[0].provider_api_format == "openai:chat"


@pytest.mark.asyncio
async def test_exact_matches_rank_before_convertible() -> None:
    register_default_normalizers()

    scheduler = CacheAwareScheduler()
    scheduler._check_model_support = AsyncMock(return_value=(True, None, None, {"m"}))  # type: ignore[method-assign]
    scheduler._check_key_availability = MagicMock(return_value=(True, None, None))  # type: ignore[method-assign]

    provider = MagicMock()
    provider.name = "p1"
    provider.enable_format_conversion = False
    # 故意把 OPENAI 放在 endpoints[0]，验证排序仍然是 CLAUDE（exact）在前
    provider.endpoints = [
        _mock_endpoint(
            "openai:chat",
            {"enabled": True, "accept_formats": ["claude:chat"], "stream_conversion": True},
        ),
        _mock_endpoint("claude:chat", None),
    ]
    provider.api_keys = [
        _mock_key("k_openai", ["openai:chat"]),
        _mock_key("k_claude", ["claude:chat"]),
    ]

    candidates = await scheduler._build_candidates(
        db=MagicMock(),
        providers=[provider],
        client_format="claude:chat",
        model_name="dummy-model",
        affinity_key=None,
        global_conversion_enabled=True,  # 全局开关开启
    )

    assert len(candidates) == 2
    assert candidates[0].needs_conversion is False
    assert candidates[0].provider_api_format == "claude:chat"
    assert candidates[1].needs_conversion is True
    assert candidates[1].provider_api_format == "openai:chat"


def test_sort_endpoints_by_family_priority_orders_openai_claude_gemini() -> None:
    eps = [
        _mock_endpoint("gemini:chat"),
        _mock_endpoint("claude:chat"),
        _mock_endpoint("openai:chat"),
    ]
    result = _sort_endpoints_by_family_priority(eps)
    assert [e.api_family for e in result] == ["openai", "claude", "gemini"]


def test_sort_endpoints_by_family_priority_unknown_family_sorted_last() -> None:
    eps = [
        _mock_endpoint("unknown:chat"),
        _mock_endpoint("openai:chat"),
    ]
    result = _sort_endpoints_by_family_priority(eps)
    assert [e.api_family for e in result] == ["openai", "unknown"]


def test_sort_endpoints_by_family_priority_stable_for_same_family() -> None:
    ep1 = _mock_endpoint("openai:chat")
    ep1.base_url = "url_1"
    ep2 = _mock_endpoint("openai:chat")
    ep2.base_url = "url_2"
    result = _sort_endpoints_by_family_priority([ep1, ep2])
    assert result[0].base_url == "url_1"
    assert result[1].base_url == "url_2"


def test_sort_endpoints_by_family_priority_empty_list() -> None:
    assert _sort_endpoints_by_family_priority([]) == []


def _group_and_sort_endpoints(
    client_family: str, client_kind: str, endpoints: list[MagicMock]
) -> list[Any]:
    preferred, preferred_other, fallback, fallback_other = [], [], [], []
    for ep in endpoints:
        same_family = ep.api_family == client_family
        same_kind = ep.endpoint_kind == client_kind
        if same_family and same_kind:
            preferred.append(ep)
        elif same_kind:
            preferred_other.append(ep)
        elif same_family:
            fallback.append(ep)
        else:
            fallback_other.append(ep)

    return (
        _sort_endpoints_by_family_priority(preferred)
        + _sort_endpoints_by_family_priority(preferred_other)
        + _sort_endpoints_by_family_priority(fallback)
        + _sort_endpoints_by_family_priority(fallback_other)
    )


def _mock_candidate_for_load_balance(
    key_id: str,
    *,
    needs_conversion: bool,
    provider_priority: int = 1,
    internal_priority: int = 1,
) -> ProviderCandidate:
    provider = MagicMock()
    provider.id = f"provider_{key_id}"
    provider.provider_priority = provider_priority

    key = MagicMock()
    key.id = key_id
    key.internal_priority = internal_priority
    key.global_priority_by_format = {"openai:chat": 1}

    endpoint = MagicMock()
    endpoint.id = f"endpoint_{key_id}"

    return ProviderCandidate(
        provider=provider,
        endpoint=endpoint,
        key=key,
        needs_conversion=needs_conversion,
    )


def test_load_balance_exact_first_enabled(monkeypatch: pytest.MonkeyPatch) -> None:
    scheduler = CacheAwareScheduler()
    scheduler.priority_mode = scheduler.PRIORITY_MODE_PROVIDER

    candidates = [
        _mock_candidate_for_load_balance("exact_1", needs_conversion=False),
        _mock_candidate_for_load_balance("conv_1", needs_conversion=True),
        _mock_candidate_for_load_balance("exact_2", needs_conversion=False),
        _mock_candidate_for_load_balance("conv_2", needs_conversion=True),
    ]

    monkeypatch.setattr(
        "src.services.cache.aware_scheduler.random.shuffle",
        lambda items: items.reverse(),
    )

    result = scheduler._apply_load_balance(
        candidates,
        api_format="openai:chat",
        exact_first=True,
    )

    assert [c.key.id for c in result] == ["exact_2", "exact_1", "conv_2", "conv_1"]


def test_load_balance_exact_first_disabled(monkeypatch: pytest.MonkeyPatch) -> None:
    scheduler = CacheAwareScheduler()
    scheduler.priority_mode = scheduler.PRIORITY_MODE_PROVIDER

    candidates = [
        _mock_candidate_for_load_balance("exact_1", needs_conversion=False),
        _mock_candidate_for_load_balance("conv_1", needs_conversion=True),
        _mock_candidate_for_load_balance("exact_2", needs_conversion=False),
        _mock_candidate_for_load_balance("conv_2", needs_conversion=True),
    ]

    monkeypatch.setattr(
        "src.services.cache.aware_scheduler.random.shuffle",
        lambda items: items.reverse(),
    )

    result = scheduler._apply_load_balance(
        candidates,
        api_format="openai:chat",
        exact_first=False,
    )

    assert [c.key.id for c in result] == ["conv_2", "exact_2", "conv_1", "exact_1"]


def test_group_and_sort_endpoints_client_openai_chat() -> None:
    endpoints = [
        _mock_endpoint("gemini:cli"),
        _mock_endpoint("claude:chat"),
        _mock_endpoint("openai:cli"),
        _mock_endpoint("gemini:chat"),
        _mock_endpoint("claude:cli"),
        _mock_endpoint("openai:chat"),
    ]
    result = _group_and_sort_endpoints("openai", "chat", endpoints)
    assert [(e.api_family, e.endpoint_kind) for e in result] == [
        ("openai", "chat"),
        ("claude", "chat"),
        ("gemini", "chat"),
        ("openai", "cli"),
        ("claude", "cli"),
        ("gemini", "cli"),
    ]


def test_group_and_sort_endpoints_client_claude_chat() -> None:
    endpoints = [
        _mock_endpoint("gemini:cli"),
        _mock_endpoint("claude:chat"),
        _mock_endpoint("openai:cli"),
        _mock_endpoint("gemini:chat"),
        _mock_endpoint("claude:cli"),
        _mock_endpoint("openai:chat"),
    ]
    result = _group_and_sort_endpoints("claude", "chat", endpoints)
    assert [(e.api_family, e.endpoint_kind) for e in result] == [
        ("claude", "chat"),
        ("openai", "chat"),
        ("gemini", "chat"),
        ("claude", "cli"),
        ("openai", "cli"),
        ("gemini", "cli"),
    ]


def test_group_and_sort_endpoints_client_openai_cli() -> None:
    endpoints = [
        _mock_endpoint("gemini:cli"),
        _mock_endpoint("claude:chat"),
        _mock_endpoint("openai:cli"),
        _mock_endpoint("gemini:chat"),
        _mock_endpoint("claude:cli"),
        _mock_endpoint("openai:chat"),
    ]
    result = _group_and_sort_endpoints("openai", "cli", endpoints)
    assert [(e.api_family, e.endpoint_kind) for e in result] == [
        ("openai", "cli"),
        ("claude", "cli"),
        ("gemini", "cli"),
        ("openai", "chat"),
        ("claude", "chat"),
        ("gemini", "chat"),
    ]
