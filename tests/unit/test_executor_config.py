from __future__ import annotations

import pytest

from src.config.settings import Config


def test_executor_config_defaults_to_rust_and_unix_socket(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for key in (
        "EXECUTOR_BACKEND",
        "EXECUTOR_TRANSPORT",
        "EXECUTOR_SOCKET_PATH",
        "EXECUTOR_BASE_URL",
        "EXECUTOR_REQUEST_TIMEOUT",
        "HTTP_REQUEST_TIMEOUT",
    ):
        monkeypatch.delenv(key, raising=False)

    cfg = Config()

    assert cfg.executor_backend == "rust"
    assert cfg.executor_transport == "unix_socket"
    assert cfg.executor_socket_path == "/tmp/aether-executor.sock"
    assert cfg.executor_base_url == "http://127.0.0.1:5219"
    assert cfg.executor_request_timeout == cfg.http_request_timeout


def test_executor_config_accepts_env_override(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("EXECUTOR_BACKEND", "RUST")
    monkeypatch.setenv("EXECUTOR_TRANSPORT", "TCP")
    monkeypatch.setenv("EXECUTOR_SOCKET_PATH", "/var/run/aether.sock")
    monkeypatch.setenv("EXECUTOR_BASE_URL", "http://127.0.0.1:9311")
    monkeypatch.setenv("EXECUTOR_REQUEST_TIMEOUT", "12.5")

    cfg = Config()

    assert cfg.executor_backend == "rust"
    assert cfg.executor_transport == "tcp"
    assert cfg.executor_socket_path == "/var/run/aether.sock"
    assert cfg.executor_base_url == "http://127.0.0.1:9311"
    assert cfg.executor_request_timeout == 12.5
