from __future__ import annotations

import pytest

from src.config.settings import Config


def test_executor_config_defaults_to_rust_and_unix_socket(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for key in (
        "EXECUTION_RUNTIME_BACKEND",
        "EXECUTION_RUNTIME_TRANSPORT",
        "EXECUTION_RUNTIME_SOCKET_PATH",
        "EXECUTION_RUNTIME_BASE_URL",
        "EXECUTION_RUNTIME_REQUEST_TIMEOUT",
        "EXECUTOR_BACKEND",
        "EXECUTOR_TRANSPORT",
        "EXECUTOR_SOCKET_PATH",
        "EXECUTOR_BASE_URL",
        "EXECUTOR_REQUEST_TIMEOUT",
        "HTTP_REQUEST_TIMEOUT",
    ):
        monkeypatch.delenv(key, raising=False)

    cfg = Config()

    assert cfg.execution_runtime_backend == "rust"
    assert cfg.execution_runtime_transport == "unix_socket"
    assert cfg.execution_runtime_socket_path == "/tmp/aether-executor.sock"
    assert cfg.execution_runtime_base_url == "http://127.0.0.1:5219"
    assert cfg.execution_runtime_request_timeout == cfg.http_request_timeout
    assert cfg.executor_backend == "rust"
    assert cfg.executor_transport == "unix_socket"
    assert cfg.executor_socket_path == "/tmp/aether-executor.sock"
    assert cfg.executor_base_url == "http://127.0.0.1:5219"
    assert cfg.executor_request_timeout == cfg.http_request_timeout


def test_executor_config_accepts_env_override(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("EXECUTION_RUNTIME_BACKEND", "RUST")
    monkeypatch.setenv("EXECUTION_RUNTIME_TRANSPORT", "TCP")
    monkeypatch.setenv("EXECUTION_RUNTIME_SOCKET_PATH", "/var/run/aether.sock")
    monkeypatch.setenv("EXECUTION_RUNTIME_BASE_URL", "http://127.0.0.1:9311")
    monkeypatch.setenv("EXECUTION_RUNTIME_REQUEST_TIMEOUT", "12.5")

    cfg = Config()

    assert cfg.execution_runtime_backend == "rust"
    assert cfg.execution_runtime_transport == "tcp"
    assert cfg.execution_runtime_socket_path == "/var/run/aether.sock"
    assert cfg.execution_runtime_base_url == "http://127.0.0.1:9311"
    assert cfg.execution_runtime_request_timeout == 12.5
    assert cfg.executor_backend == "rust"
    assert cfg.executor_transport == "tcp"
    assert cfg.executor_socket_path == "/var/run/aether.sock"
    assert cfg.executor_base_url == "http://127.0.0.1:9311"
    assert cfg.executor_request_timeout == 12.5


def test_executor_config_legacy_envs_still_work(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("EXECUTOR_BACKEND", "RUST")
    monkeypatch.setenv("EXECUTOR_TRANSPORT", "TCP")
    monkeypatch.setenv("EXECUTOR_SOCKET_PATH", "/tmp/legacy-aether.sock")
    monkeypatch.setenv("EXECUTOR_BASE_URL", "http://127.0.0.1:9322")
    monkeypatch.setenv("EXECUTOR_REQUEST_TIMEOUT", "14.5")

    cfg = Config()

    assert cfg.execution_runtime_backend == "rust"
    assert cfg.execution_runtime_transport == "tcp"
    assert cfg.execution_runtime_socket_path == "/tmp/legacy-aether.sock"
    assert cfg.execution_runtime_base_url == "http://127.0.0.1:9322"
    assert cfg.execution_runtime_request_timeout == 14.5
    assert cfg.executor_backend == "rust"
    assert cfg.executor_transport == "tcp"
    assert cfg.executor_socket_path == "/tmp/legacy-aether.sock"
    assert cfg.executor_base_url == "http://127.0.0.1:9322"
    assert cfg.executor_request_timeout == 14.5
