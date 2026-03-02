from __future__ import annotations

from src.api.admin.provider_oauth import (
    _PROVIDER_OAUTH_BATCH_IMPORT_PROXY_TIMEOUT_SECONDS,
    _PROVIDER_OAUTH_DEFAULT_TIMEOUT_SECONDS,
    _resolve_batch_import_timeout_seconds,
)


def test_batch_import_timeout_without_proxy_uses_default() -> None:
    assert _resolve_batch_import_timeout_seconds(None) == _PROVIDER_OAUTH_DEFAULT_TIMEOUT_SECONDS
    assert (
        _resolve_batch_import_timeout_seconds({"enabled": False})
        == _PROVIDER_OAUTH_DEFAULT_TIMEOUT_SECONDS
    )


def test_batch_import_timeout_with_proxy_uses_extended_value() -> None:
    assert (
        _resolve_batch_import_timeout_seconds({"node_id": "node-1", "enabled": True})
        == _PROVIDER_OAUTH_BATCH_IMPORT_PROXY_TIMEOUT_SECONDS
    )
    assert (
        _resolve_batch_import_timeout_seconds({"node_id": "node-1"})
        == _PROVIDER_OAUTH_BATCH_IMPORT_PROXY_TIMEOUT_SECONDS
    )
    assert (
        _resolve_batch_import_timeout_seconds("http://legacy-proxy")  # type: ignore[arg-type]
        == _PROVIDER_OAUTH_BATCH_IMPORT_PROXY_TIMEOUT_SECONDS
    )
