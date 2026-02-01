from types import SimpleNamespace
from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest

from src.config.settings import config
from src.core.api_format.conversion.internal_video import VideoStatus
from src.services.billing.formula_engine import BillingIncompleteError
from src.services.task.impl.video_telemetry import VideoTelemetry


def _make_task(**overrides: Any) -> SimpleNamespace:
    task = SimpleNamespace(
        id="t1",
        user_id="u1",
        api_key_id="ak1",
        provider_id="p1",
        endpoint_id="e1",
        key_id="k1",
        external_task_id="ext-1",
        client_api_format="openai:video",
        provider_api_format="openai:video",
        format_converted=False,
        model="sora",
        original_request_body={"model": "sora"},
        duration_seconds=4,
        resolution="720p",
        aspect_ratio="16:9",
        size="1024x1024",
        retry_count=0,
        video_size_bytes=None,
        video_url="https://example.com/v.mp4",
        video_urls=["https://example.com/v.mp4"],
        submitted_at=None,
        completed_at=None,
        error_code=None,
        error_message=None,
        status=VideoStatus.COMPLETED.value,
        request_metadata={"request_id": "req-1", "poll_raw_response": {"foo": "bar"}},
    )
    for k, v in overrides.items():
        setattr(task, k, v)
    return task


def _make_db() -> MagicMock:
    db = MagicMock()

    user_obj = SimpleNamespace(id="u1")
    api_key_obj = SimpleNamespace(id="ak1")
    provider_obj = SimpleNamespace(id="p1", name="prov1")

    q_user = MagicMock()
    q_user.filter.return_value.first.return_value = user_obj
    q_key = MagicMock()
    q_key.filter.return_value.first.return_value = api_key_obj
    q_provider = MagicMock()
    q_provider.filter.return_value.first.return_value = provider_obj

    db.query.side_effect = [q_user, q_key, q_provider]
    return db


@pytest.mark.asyncio
async def test_video_telemetry_failed_records_cost_zero(monkeypatch: pytest.MonkeyPatch) -> None:
    db = _make_db()
    task = _make_task(status=VideoStatus.FAILED.value, error_message="boom")

    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.DimensionCollectorService.collect_dimensions",
        lambda _self, **_kwargs: {"duration_seconds": 4},
    )
    record = AsyncMock()
    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.UsageService.record_usage_with_custom_cost",
        record,
    )

    telemetry = VideoTelemetry(db)
    await telemetry.record_terminal_usage(task)

    # billing_snapshot should be written back to task.request_metadata
    assert task.request_metadata["billing_snapshot"]["billed_reason"] == "task_failed"

    assert record.await_count == 1
    kwargs = record.await_args.kwargs
    assert kwargs["total_cost_usd"] == 0.0
    assert kwargs["status"] == "failed"


@pytest.mark.asyncio
async def test_video_telemetry_completed_no_rule(monkeypatch: pytest.MonkeyPatch) -> None:
    db = _make_db()
    task = _make_task(status=VideoStatus.COMPLETED.value)

    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.DimensionCollectorService.collect_dimensions",
        lambda _self, **_kwargs: {"duration_seconds": 4},
    )
    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.BillingRuleService.find_rule",
        lambda *_args, **_kwargs: None,
    )
    record = AsyncMock()
    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.UsageService.record_usage_with_custom_cost",
        record,
    )

    telemetry = VideoTelemetry(db)
    await telemetry.record_terminal_usage(task)

    assert task.request_metadata["billing_snapshot"]["status"] == "no_rule"

    kwargs = record.await_args.kwargs
    assert kwargs["total_cost_usd"] == 0.0
    assert kwargs["status"] == "completed"


@pytest.mark.asyncio
async def test_video_telemetry_strict_mode_missing_required_marks_failed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    db = _make_db()
    task = _make_task(status=VideoStatus.COMPLETED.value)

    # provide a billing rule so formula path is taken
    rule = SimpleNamespace(
        id="r1",
        name="video",
        expression="duration_seconds",
        variables={},
        dimension_mappings={},
    )
    lookup = SimpleNamespace(rule=rule, scope="model", effective_task_type="video")

    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.DimensionCollectorService.collect_dimensions",
        lambda _self, **_kwargs: {"duration_seconds": None},
    )
    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.BillingRuleService.find_rule",
        lambda *_args, **_kwargs: lookup,
    )
    record = AsyncMock()
    monkeypatch.setattr(
        "src.services.task.impl.video_telemetry.UsageService.record_usage_with_custom_cost",
        record,
    )

    old = config.billing_strict_mode
    try:
        config.billing_strict_mode = True
        telemetry = VideoTelemetry(db)
        telemetry._formula_engine.evaluate = MagicMock(
            side_effect=BillingIncompleteError(
                "Missing required dimensions", missing_required=["duration_seconds"]
            )
        )
        await telemetry.record_terminal_usage(task)
    finally:
        config.billing_strict_mode = old

    assert task.status == VideoStatus.FAILED.value
    assert task.video_url is None
    assert task.video_urls is None
    assert "billing_incomplete" in (task.error_code or "")

    kwargs = record.await_args.kwargs
    assert kwargs["total_cost_usd"] == 0.0
    assert kwargs["status"] == "failed"
