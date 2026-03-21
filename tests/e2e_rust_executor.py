"""
aether-executor 本地端到端测试

测试流程:
1. 启动本地假上游 HTTP 服务
2. 启动 aether-executor（Unix Socket）
3. 用 Python RustExecutorClient 发送 ExecutionPlan
4. 验证执行结果与上游响应一致

运行:
  cargo build -p aether-executor
  uv run python tests/e2e_rust_executor.py
"""

from __future__ import annotations

import asyncio
import base64
import contextlib
import gzip
import json
import signal
import subprocess
import sys
from pathlib import Path

import httpx

from src.services.request.executor_plan import (
    ExecutionPlan,
    ExecutionPlanBody,
    ExecutionProxySnapshot,
    ExecutionPlanTimeouts,
)
from src.services.request.rust_executor_client import RustExecutorClient


async def _upstream_app(scope, receive, send) -> None:  # type: ignore[no-untyped-def]
    assert scope["type"] == "http"
    body = b""
    while True:
        message = await receive()
        if message["type"] != "http.request":
            continue
        body += message.get("body", b"")
        if not message.get("more_body", False):
            break

    header_map = {
        key.decode("latin1").lower(): value.decode("latin1")
        for key, value in scope.get("headers", [])
    }
    path = str(scope.get("path") or "")
    content_encoding = header_map.get("content-encoding")
    decoded_body = gzip.decompress(body) if content_encoding == "gzip" else body

    if path == "/v1/raw":
        response_body = json.dumps(
            {
                "ok": True,
                "path": path,
                "received_body_text": decoded_body.decode("utf-8", errors="replace"),
                "received_content_type": header_map.get("content-type"),
                "received_content_encoding": content_encoding,
            },
            ensure_ascii=False,
        ).encode("utf-8")
        await send(
            {
                "type": "http.response.start",
                "status": 200,
                "headers": [
                    [b"content-type", b"application/json"],
                    [b"x-upstream-test", b"true"],
                ],
            }
        )
        await send(
            {
                "type": "http.response.body",
                "body": response_body,
                "more_body": False,
            }
        )
        return

    if path == "/v1/download":
        await send(
            {
                "type": "http.response.start",
                "status": 200,
                "headers": [
                    [b"content-type", b"video/mp4"],
                    [b"x-download-test", b"true"],
                ],
            }
        )
        await send(
            {
                "type": "http.response.body",
                "body": b"video-download-bytes",
                "more_body": False,
            }
        )
        return

    if path == "/v1/delete" and scope.get("method") == "DELETE":
        await send(
            {
                "type": "http.response.start",
                "status": 204,
                "headers": [[b"x-delete-test", b"true"]],
            }
        )
        await send(
            {
                "type": "http.response.body",
                "body": b"",
                "more_body": False,
            }
        )
        return

    payload = json.loads(decoded_body.decode("utf-8"))
    if payload.get("stream") is True:
        stream_body = (
            "data: "
            + json.dumps(
                {
                    "id": "chatcmpl-stream-1",
                    "object": "chat.completion.chunk",
                    "model": payload.get("model"),
                    "choices": [
                        {
                            "index": 0,
                            "delta": {"role": "assistant", "content": "hello"},
                            "finish_reason": None,
                        }
                    ],
                },
                ensure_ascii=False,
            )
            + "\n\n"
            + "data: "
            + json.dumps(
                {
                    "id": "chatcmpl-stream-1",
                    "object": "chat.completion.chunk",
                    "model": payload.get("model"),
                    "choices": [
                        {
                            "index": 0,
                            "delta": {},
                            "finish_reason": "stop",
                        }
                    ],
                    "usage": {
                        "prompt_tokens": 3,
                        "completion_tokens": 1,
                        "total_tokens": 4,
                    },
                },
                ensure_ascii=False,
            )
            + "\n\n"
            + "data: [DONE]\n\n"
        ).encode("utf-8")
        await send(
            {
                "type": "http.response.start",
                "status": 200,
                "headers": [
                    [b"content-type", b"text/event-stream"],
                    [b"x-upstream-test", b"true"],
                ],
            }
        )
        await send(
            {
                "type": "http.response.body",
                "body": stream_body,
                "more_body": False,
            }
        )
        return

    response = {
        "ok": True,
        "received_model": payload.get("model"),
        "received_messages": payload.get("messages", []),
        "path": path,
        "received_content_encoding": content_encoding,
    }
    response_body = json.dumps(response, ensure_ascii=False).encode("utf-8")

    await send(
        {
            "type": "http.response.start",
            "status": 200,
            "headers": [
                [b"content-type", b"application/json"],
                [b"x-upstream-test", b"true"],
            ],
        }
    )
    await send(
        {
            "type": "http.response.body",
            "body": response_body,
            "more_body": False,
        }
    )


async def _relay_app(scope, receive, send) -> None:  # type: ignore[no-untyped-def]
    assert scope["type"] == "http"
    body = b""
    while True:
        message = await receive()
        if message["type"] != "http.request":
            continue
        body += message.get("body", b"")
        if not message.get("more_body", False):
            break

    if len(body) < 4:
        await send(
            {
                "type": "http.response.start",
                "status": 400,
                "headers": [[b"content-type", b"text/plain; charset=utf-8"]],
            }
        )
        await send({"type": "http.response.body", "body": b"invalid relay envelope", "more_body": False})
        return

    path = str(scope.get("path") or "")
    node_id = path.rsplit("/", 1)[-1]
    meta_len = int.from_bytes(body[:4], "big")
    if meta_len <= 0 or len(body) < 4 + meta_len:
        await send(
            {
                "type": "http.response.start",
                "status": 400,
                "headers": [[b"content-type", b"text/plain; charset=utf-8"]],
            }
        )
        await send({"type": "http.response.body", "body": b"invalid relay metadata", "more_body": False})
        return
    meta = json.loads(body[4 : 4 + meta_len].decode("utf-8"))
    request_json = json.loads(body[4 + meta_len :].decode("utf-8"))

    if request_json.get("stream") is True:
        stream_body = (
            "data: "
            + json.dumps(
                {
                    "id": "relay-stream-1",
                    "object": "chat.completion.chunk",
                    "model": request_json.get("model"),
                    "choices": [{"index": 0, "delta": {"content": "relay"}, "finish_reason": None}],
                },
                ensure_ascii=False,
            )
            + "\n\n"
            + "data: [DONE]\n\n"
        ).encode("utf-8")
        await send(
            {
                "type": "http.response.start",
                "status": 200,
                "headers": [
                    [b"content-type", b"text/event-stream"],
                    [b"x-relay-node", node_id.encode("utf-8")],
                ],
            }
        )
        await send({"type": "http.response.body", "body": stream_body, "more_body": False})
        return

    response_body = json.dumps(
        {
            "ok": True,
            "via_tunnel": True,
            "node_id": node_id,
            "meta_url": meta.get("url"),
            "received_model": request_json.get("model"),
        },
        ensure_ascii=False,
    ).encode("utf-8")
    await send(
        {
            "type": "http.response.start",
            "status": 200,
            "headers": [
                [b"content-type", b"application/json"],
                [b"x-relay-node", node_id.encode("utf-8")],
            ],
        }
    )
    await send(
        {
            "type": "http.response.body",
            "body": response_body,
            "more_body": False,
        }
    )


async def _start_proxy_server() -> tuple[asyncio.AbstractServer, int, asyncio.Future[str]]:
    loop = asyncio.get_running_loop()
    request_line_future: asyncio.Future[str] = loop.create_future()

    async def _handle_proxy(
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ) -> None:
        try:
            header_bytes = await reader.readuntil(b"\r\n\r\n")
            header_text = header_bytes.decode("latin-1")
            lines = header_text.split("\r\n")
            if lines and not request_line_future.done():
                request_line_future.set_result(lines[0])

            content_length = 0
            for line in lines[1:]:
                if not line:
                    continue
                if line.lower().startswith("content-length:"):
                    content_length = int(line.split(":", 1)[1].strip() or "0")
                    break

            if content_length:
                await reader.readexactly(content_length)

            body = json.dumps(
                {
                    "ok": True,
                    "via_proxy": True,
                    "request_line": lines[0] if lines else "",
                },
                ensure_ascii=False,
            ).encode("utf-8")
            writer.write(
                b"HTTP/1.1 200 OK\r\n"
                b"content-type: application/json\r\n"
                b"x-proxy-test: true\r\n"
                + f"content-length: {len(body)}\r\n\r\n".encode("ascii")
                + body
            )
            await writer.drain()
        finally:
            writer.close()
            with contextlib.suppress(Exception):
                await writer.wait_closed()

    server = await asyncio.start_server(_handle_proxy, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    return server, port, request_line_future


async def run_test() -> bool:
    repo_root = Path(__file__).resolve().parents[1]
    executor_binary = repo_root / "target" / "debug" / "aether-executor"
    if not executor_binary.is_file():
        print(f"FAIL: executor binary not found at {executor_binary}")
        print("  run: cargo build -p aether-executor")
        return False

    upstream_listener = await asyncio.start_server(lambda r, w: None, "127.0.0.1", 0)
    upstream_port = upstream_listener.sockets[0].getsockname()[1]
    upstream_listener.close()
    await upstream_listener.wait_closed()

    import uvicorn

    upstream_config = uvicorn.Config(
        _upstream_app,
        host="127.0.0.1",
        port=upstream_port,
        log_level="warning",
    )
    upstream_server = uvicorn.Server(upstream_config)
    upstream_task = asyncio.create_task(upstream_server.serve())

    relay_listener = await asyncio.start_server(lambda r, w: None, "127.0.0.1", 0)
    relay_port = relay_listener.sockets[0].getsockname()[1]
    relay_listener.close()
    await relay_listener.wait_closed()

    relay_config = uvicorn.Config(
        _relay_app,
        host="127.0.0.1",
        port=relay_port,
        log_level="warning",
    )
    relay_server = uvicorn.Server(relay_config)
    relay_task = asyncio.create_task(relay_server.serve())

    executor_socket = Path("/tmp/aether-executor-e2e.sock")
    executor_socket.unlink(missing_ok=True)
    executor_proc = subprocess.Popen(
        [
            str(executor_binary),
            "--transport",
            "unix_socket",
            "--unix-socket",
            str(executor_socket),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )

    try:
        print("[1/4] Waiting for upstream server ...")
        async with httpx.AsyncClient() as client:
            for _ in range(50):
                try:
                    resp = await client.post(
                        f"http://127.0.0.1:{upstream_port}/healthz",
                        json={"model": "probe"},
                        timeout=0.2,
                    )
                    if resp.status_code == 200:
                        break
                except Exception:
                    await asyncio.sleep(0.1)
            else:
                print("FAIL: upstream server did not start in time")
                return False

            for _ in range(50):
                try:
                    resp = await client.post(
                        f"http://127.0.0.1:{relay_port}/local/relay/probe",
                        content=(0).to_bytes(4, "big"),
                        timeout=0.2,
                    )
                    if resp.status_code in {400, 500}:
                        break
                except Exception:
                    await asyncio.sleep(0.1)
            else:
                print("FAIL: relay server did not start in time")
                return False

        print("[2/4] Waiting for aether-executor ...")
        for _ in range(100):
            if executor_socket.exists():
                try:
                    async with httpx.AsyncClient(
                        transport=httpx.AsyncHTTPTransport(uds=str(executor_socket)),
                        base_url="http://127.0.0.1:5219",
                    ) as client:
                        resp = await client.get("/health", timeout=0.2)
                    if resp.status_code == 200:
                        break
                except Exception:
                    pass
            await asyncio.sleep(0.1)
        else:
            print("FAIL: executor did not start in time")
            if executor_proc.stdout is not None:
                print(executor_proc.stdout.read())
            return False

        print("[3/4] Sending ExecutionPlan via RustExecutorClient ...")
        client = RustExecutorClient(
            transport="unix_socket",
            socket_path=str(executor_socket),
            base_url="http://127.0.0.1:5219",
            request_timeout=5.0,
        )
        result = await client.execute_sync_json(
            ExecutionPlan(
                request_id="req-e2e-1",
                candidate_id=None,
                provider_name="openai",
                provider_id="prov-1",
                endpoint_id="ep-1",
                key_id="key-1",
                method="POST",
                url=f"http://127.0.0.1:{upstream_port}/v1/chat/completions",
                headers={"content-type": "application/json"},
                body=ExecutionPlanBody(
                    json_body={
                        "model": "gpt-4.1",
                        "messages": [{"role": "user", "content": "hello"}],
                    }
                ),
                stream=False,
                provider_api_format="openai:chat",
                client_api_format="openai:chat",
                model_name="gpt-4.1",
                content_type="application/json",
                timeouts=ExecutionPlanTimeouts(
                    connect_ms=5_000,
                    read_ms=5_000,
                    write_ms=5_000,
                    pool_ms=5_000,
                    total_ms=5_000,
                ),
            )
        )

        print("[4/4] Verifying response ...")
        assert result.status_code == 200
        assert result.headers.get("x-upstream-test") == "true"
        assert result.response_json["ok"] is True
        assert result.response_json["received_model"] == "gpt-4.1"
        assert result.response_json["path"] == "/v1/chat/completions"

        print("[extra] Verifying tunnel relay sync execution ...")
        tunnel_result = await client.execute_sync_json(
            ExecutionPlan(
                request_id="req-e2e-tunnel-1",
                candidate_id=None,
                provider_name="openai",
                provider_id="prov-1",
                endpoint_id="ep-1",
                key_id="key-1",
                method="POST",
                url="https://upstream-via-relay.test/v1/chat/completions",
                headers={"content-type": "application/json"},
                body=ExecutionPlanBody(
                    json_body={
                        "model": "gpt-4.1",
                        "messages": [{"role": "user", "content": "hello tunnel"}],
                    }
                ),
                stream=False,
                provider_api_format="openai:chat",
                client_api_format="openai:chat",
                model_name="gpt-4.1",
                content_type="application/json",
                proxy=ExecutionProxySnapshot(
                    enabled=True,
                    mode="tunnel",
                    node_id="node-1",
                    label="relay-node",
                    extra={"hub_base_url": f"http://127.0.0.1:{relay_port}"},
                ),
                timeouts=ExecutionPlanTimeouts(
                    connect_ms=5_000,
                    read_ms=5_000,
                    write_ms=5_000,
                    pool_ms=5_000,
                    total_ms=5_000,
                ),
            )
        )
        assert tunnel_result.status_code == 200
        assert tunnel_result.headers.get("x-relay-node") == "node-1"
        assert tunnel_result.response_json["via_tunnel"] is True
        assert tunnel_result.response_json["meta_url"] == "https://upstream-via-relay.test/v1/chat/completions"

        print("[extra] Verifying HTTP proxy execution ...")
        proxy_server, proxy_port, proxy_request_line = await _start_proxy_server()
        try:
            proxied_result = await client.execute_sync_json(
                ExecutionPlan(
                    request_id="req-e2e-proxy-1",
                    candidate_id=None,
                    provider_name="openai",
                    provider_id="prov-1",
                    endpoint_id="ep-1",
                    key_id="key-1",
                    method="POST",
                    url=f"http://127.0.0.1:{upstream_port}/v1/chat/completions",
                    headers={"content-type": "application/json"},
                    body=ExecutionPlanBody(
                        json_body={
                            "model": "gpt-4.1",
                            "messages": [{"role": "user", "content": "proxy hello"}],
                        }
                    ),
                    stream=False,
                    provider_api_format="openai:chat",
                    client_api_format="openai:chat",
                    model_name="gpt-4.1",
                    content_type="application/json",
                    proxy=ExecutionProxySnapshot(
                        enabled=True,
                        mode="http",
                        label="local-proxy",
                        url=f"http://127.0.0.1:{proxy_port}",
                    ),
                    timeouts=ExecutionPlanTimeouts(
                        connect_ms=5_000,
                        read_ms=5_000,
                        write_ms=5_000,
                        pool_ms=5_000,
                        total_ms=5_000,
                    ),
                )
            )
            request_line = await asyncio.wait_for(proxy_request_line, timeout=5.0)
        finally:
            proxy_server.close()
            await proxy_server.wait_closed()

        assert proxied_result.status_code == 200
        assert proxied_result.headers.get("x-proxy-test") == "true"
        assert proxied_result.response_json["via_proxy"] is True
        assert request_line.startswith("POST http://127.0.0.1:")
        assert "/v1/chat/completions HTTP/1.1" in request_line

        print("[extra] Verifying upstream-stream raw byte path ...")
        stream_result = await client.execute_sync_json(
            ExecutionPlan(
                request_id="req-e2e-stream-1",
                candidate_id=None,
                provider_name="openai",
                provider_id="prov-1",
                endpoint_id="ep-1",
                key_id="key-1",
                method="POST",
                url=f"http://127.0.0.1:{upstream_port}/v1/chat/completions",
                headers={"content-type": "application/json"},
                body=ExecutionPlanBody(
                    json_body={
                        "model": "gpt-4.1",
                        "messages": [{"role": "user", "content": "hello stream"}],
                        "stream": True,
                    }
                ),
                stream=True,
                provider_api_format="openai:chat",
                client_api_format="openai:chat",
                model_name="gpt-4.1",
                content_type="application/json",
                timeouts=ExecutionPlanTimeouts(
                    connect_ms=5_000,
                    read_ms=5_000,
                    write_ms=5_000,
                    pool_ms=5_000,
                    total_ms=5_000,
                ),
            )
        )
        assert stream_result.status_code == 200
        assert stream_result.response_json is None
        assert stream_result.response_body_bytes is not None
        assert b"chat.completion.chunk" in stream_result.response_body_bytes
        assert b"[DONE]" in stream_result.response_body_bytes

        print("[extra] Verifying gzip request body path ...")
        gzip_result = await client.execute_sync_json(
            ExecutionPlan(
                request_id="req-e2e-gzip-1",
                candidate_id="cand-e2e-gzip-1",
                provider_name="openai",
                provider_id="prov-e2e",
                endpoint_id="ep-e2e",
                key_id="key-e2e",
                method="POST",
                url=f"http://127.0.0.1:{upstream_port}/v1/chat/completions",
                headers={"content-type": "application/json"},
                body=ExecutionPlanBody(
                    json_body={
                        "model": "gpt-4.1",
                        "messages": [{"role": "user", "content": "hello gzip"}],
                    }
                ),
                stream=False,
                provider_api_format="openai:chat",
                client_api_format="openai:chat",
                model_name="gpt-4.1",
                content_type="application/json",
                content_encoding="gzip",
                timeouts=ExecutionPlanTimeouts(connect_ms=5_000, total_ms=30_000),
            )
        )
        assert gzip_result.status_code == 200
        assert gzip_result.response_json is not None
        assert gzip_result.response_json["received_content_encoding"] == "gzip"
        assert gzip_result.response_json["received_model"] == "gpt-4.1"

        print("[extra] Verifying raw body bytes path ...")
        raw_result = await client.execute_sync_json(
            ExecutionPlan(
                request_id="req-e2e-raw-1",
                candidate_id="cand-e2e-raw-1",
                provider_name="openai",
                provider_id="prov-e2e",
                endpoint_id="ep-e2e",
                key_id="key-e2e",
                method="POST",
                url=f"http://127.0.0.1:{upstream_port}/v1/raw",
                headers={"content-type": "text/plain"},
                body=ExecutionPlanBody(
                    body_bytes_b64=base64.b64encode(b"hello raw body").decode("ascii")
                ),
                stream=False,
                provider_api_format="openai:chat",
                client_api_format="openai:chat",
                model_name="gpt-4.1",
                content_type="text/plain",
                timeouts=ExecutionPlanTimeouts(connect_ms=5_000, total_ms=30_000),
            )
        )
        assert raw_result.status_code == 200
        assert raw_result.response_json is not None
        assert raw_result.response_json["received_body_text"] == "hello raw body"
        assert raw_result.response_json["received_content_type"] == "text/plain"

        print("[extra] Verifying sync empty-body delete path ...")
        delete_result = await client.execute_sync_json(
            ExecutionPlan(
                request_id="req-e2e-delete-1",
                candidate_id=None,
                provider_name="openai",
                provider_id="prov-1",
                endpoint_id="ep-1",
                key_id="key-1",
                method="DELETE",
                url=f"http://127.0.0.1:{upstream_port}/v1/delete",
                headers={},
                body=ExecutionPlanBody(),
                stream=False,
                provider_api_format="openai:video",
                client_api_format="openai:video",
                model_name="sora-2",
                timeouts=ExecutionPlanTimeouts(
                    connect_ms=5_000,
                    read_ms=5_000,
                    write_ms=5_000,
                    pool_ms=5_000,
                    total_ms=5_000,
                ),
            )
        )
        assert delete_result.status_code == 204
        assert delete_result.response_json is None
        assert delete_result.response_body_bytes is None

        print("[extra] Verifying GET/no-body stream path ...")
        download_stream = await client.execute_stream(
            ExecutionPlan(
                request_id="req-e2e-download-1",
                candidate_id=None,
                provider_name="openai",
                provider_id="prov-1",
                endpoint_id="ep-1",
                key_id="key-1",
                method="GET",
                url=f"http://127.0.0.1:{upstream_port}/v1/download",
                headers={},
                body=ExecutionPlanBody(),
                stream=True,
                provider_api_format="openai:video",
                client_api_format="openai:video",
                model_name="sora-2",
                timeouts=ExecutionPlanTimeouts(
                    connect_ms=5_000,
                    read_ms=5_000,
                    write_ms=5_000,
                    pool_ms=5_000,
                    total_ms=5_000,
                ),
            )
        )
        try:
            download_chunks = [chunk async for chunk in download_stream.byte_iterator]
        finally:
            await download_stream.response_ctx.__aexit__(None, None, None)

        assert download_stream.status_code == 200
        assert download_stream.headers.get("x-download-test") == "true"
        assert b"".join(download_chunks) == b"video-download-bytes"

        print("[extra] Verifying native stream path ...")
        live_stream = await client.execute_stream(
            ExecutionPlan(
                request_id="req-e2e-live-stream-1",
                candidate_id=None,
                provider_name="openai",
                provider_id="prov-1",
                endpoint_id="ep-1",
                key_id="key-1",
                method="POST",
                url=f"http://127.0.0.1:{upstream_port}/v1/chat/completions",
                headers={"content-type": "application/json"},
                body=ExecutionPlanBody(
                    json_body={
                        "model": "gpt-4.1",
                        "messages": [{"role": "user", "content": "hello live stream"}],
                        "stream": True,
                    }
                ),
                stream=True,
                provider_api_format="openai:chat",
                client_api_format="openai:chat",
                model_name="gpt-4.1",
                content_type="application/json",
                timeouts=ExecutionPlanTimeouts(
                    connect_ms=5_000,
                    read_ms=5_000,
                    write_ms=5_000,
                    pool_ms=5_000,
                    total_ms=5_000,
                ),
            )
        )
        try:
            live_chunks = [chunk async for chunk in live_stream.byte_iterator]
        finally:
            await live_stream.response_ctx.__aexit__(None, None, None)

        assert live_stream.status_code == 200
        assert live_stream.headers.get("x-upstream-test") == "true"
        live_body = b"".join(live_chunks)
        assert live_body
        assert b"chat.completion.chunk" in live_body
        assert b"[DONE]" in live_body

        print("[extra] Verifying native tunnel stream path ...")
        tunnel_stream = await client.execute_stream(
            ExecutionPlan(
                request_id="req-e2e-tunnel-stream-1",
                candidate_id=None,
                provider_name="openai",
                provider_id="prov-1",
                endpoint_id="ep-1",
                key_id="key-1",
                method="POST",
                url="https://upstream-via-relay.test/v1/chat/completions",
                headers={"content-type": "application/json"},
                body=ExecutionPlanBody(
                    json_body={
                        "model": "gpt-4.1",
                        "messages": [{"role": "user", "content": "hello relay stream"}],
                        "stream": True,
                    }
                ),
                stream=True,
                provider_api_format="openai:chat",
                client_api_format="openai:chat",
                model_name="gpt-4.1",
                content_type="application/json",
                proxy=ExecutionProxySnapshot(
                    enabled=True,
                    mode="tunnel",
                    node_id="node-1",
                    label="relay-node",
                    extra={"hub_base_url": f"http://127.0.0.1:{relay_port}"},
                ),
                timeouts=ExecutionPlanTimeouts(
                    connect_ms=5_000,
                    read_ms=5_000,
                    write_ms=5_000,
                    pool_ms=5_000,
                    total_ms=5_000,
                ),
            )
        )
        try:
            tunnel_stream_chunks = [chunk async for chunk in tunnel_stream.byte_iterator]
        finally:
            await tunnel_stream.response_ctx.__aexit__(None, None, None)

        tunnel_stream_body = b"".join(tunnel_stream_chunks)
        assert tunnel_stream.status_code == 200
        assert tunnel_stream.headers.get("x-relay-node") == "node-1"
        assert b"relay-stream-1" in tunnel_stream_body
        assert b"[DONE]" in tunnel_stream_body
        print("ALL TESTS PASSED")
        return True

    finally:
        upstream_server.should_exit = True
        try:
            await asyncio.wait_for(upstream_task, timeout=5.0)
        except Exception:
            upstream_task.cancel()
            with contextlib.suppress(Exception):
                await upstream_task

        relay_server.should_exit = True
        try:
            await asyncio.wait_for(relay_task, timeout=5.0)
        except Exception:
            relay_task.cancel()
            with contextlib.suppress(Exception):
                await relay_task

        executor_proc.send_signal(signal.SIGTERM)
        try:
            executor_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            executor_proc.kill()
            executor_proc.wait()
        executor_socket.unlink(missing_ok=True)


if __name__ == "__main__":
    success = asyncio.run(run_test())
    sys.exit(0 if success else 1)
