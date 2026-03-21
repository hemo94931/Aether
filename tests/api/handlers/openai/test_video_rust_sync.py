from __future__ import annotations

from types import SimpleNamespace
from unittest.mock import AsyncMock

import httpx
import pytest
from fastapi.responses import JSONResponse

import src.api.handlers.openai.video_handler as video_mod
from src.api.handlers.openai.video_handler import OpenAIVideoHandler


def _make_handler() -> OpenAIVideoHandler:
    return OpenAIVideoHandler(
        db=SimpleNamespace(),
        user=SimpleNamespace(id="user-1"),
        api_key=SimpleNamespace(id="api-key-1"),
        request_id="req-video-sync-test",
        client_ip="127.0.0.1",
        user_agent="pytest",
        start_time=0.0,
    )


@pytest.mark.asyncio
async def test_handle_create_task_uses_rust_sync_helper(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    handler = _make_handler()
    monkeypatch.setattr(video_mod.UsageService, "create_pending_usage", lambda *args, **kwargs: None)
    monkeypatch.setattr(
        handler._normalizer,
        "video_request_to_internal",
        lambda body: SimpleNamespace(model=str(body.get("model") or "sora-2")),
    )

    candidate = SimpleNamespace(provider=SimpleNamespace(name="provider-1", id="prov-1"))
    endpoint = SimpleNamespace(
        id="ep-1",
        api_family="openai",
        endpoint_kind="video",
        base_url="https://api.openai.com",
        body_rules=None,
    )
    provider_key = SimpleNamespace(id="key-1")

    monkeypatch.setattr(
        handler,
        "_resolve_upstream_key",
        AsyncMock(return_value=("upstream-key", endpoint, provider_key)),
    )
    monkeypatch.setattr(
        handler,
        "_build_upstream_url",
        lambda base_url: "https://api.openai.com/v1/videos",
    )
    monkeypatch.setattr(
        handler,
        "_build_upstream_headers",
        lambda original_headers, upstream_key, endpoint, **kwargs: {
            "authorization": f"Bearer {upstream_key}"
        },
    )
    monkeypatch.setattr(
        video_mod.HTTPClientPool,
        "get_default_client_async",
        AsyncMock(side_effect=AssertionError("python fallback should not run")),
    )

    async def _fake_rust_sync(**kwargs: object) -> httpx.Response:
        assert kwargs["method"] == "POST"
        assert kwargs["url"] == "https://api.openai.com/v1/videos"
        assert kwargs["provider_id"] == "prov-1"
        assert kwargs["endpoint_id"] == "ep-1"
        assert kwargs["key_id"] == "key-1"
        assert kwargs["body"] == {"model": "sora-2", "prompt": "hello"}
        return httpx.Response(
            200,
            request=httpx.Request("POST", str(kwargs["url"])),
            json={"id": "ext-1"},
        )

    create_failed = AsyncMock()
    monkeypatch.setattr(handler, "_try_rust_sync_http_response", _fake_rust_sync)
    monkeypatch.setattr(handler, "_create_failed_task_and_usage", create_failed)

    async def _fake_submit_with_failover(**kwargs: object) -> JSONResponse:
        response = await kwargs["submit_func"](candidate)
        assert response.status_code == 200
        assert response.json()["id"] == "ext-1"
        return JSONResponse(status_code=400, content={"error": {"message": "stop"}})

    monkeypatch.setattr(handler, "_submit_with_failover", _fake_submit_with_failover)

    response = await handler.handle_create_task(
        http_request=SimpleNamespace(),
        original_headers={},
        original_request_body={"model": "sora-2", "prompt": "hello"},
    )

    assert response.status_code == 400
    create_failed.assert_awaited_once()
