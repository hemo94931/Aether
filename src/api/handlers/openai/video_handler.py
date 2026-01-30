"""
OpenAI Video Handler - Sora 视频生成实现
"""

from __future__ import annotations

import json
from datetime import datetime, timedelta, timezone
from typing import Any, AsyncIterator
from uuid import uuid4

from fastapi import HTTPException, Request
from fastapi.responses import JSONResponse, Response, StreamingResponse
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from src.api.handlers.base.video_handler_base import VideoHandlerBase, sanitize_error_message
from src.clients.http_client import HTTPClientPool
from src.core.api_format import APIFormat, build_upstream_headers, get_extra_headers_from_endpoint
from src.core.api_format.conversion.internal_video import (
    InternalVideoRequest,
    InternalVideoTask,
    VideoStatus,
)
from src.core.api_format.conversion.normalizers.openai import OpenAINormalizer
from src.core.api_format.headers import HOP_BY_HOP_HEADERS
from src.core.crypto import crypto_service
from src.core.logger import logger
from src.models.database import ApiKey, ProviderAPIKey, ProviderEndpoint, User, VideoTask
from src.services.cache.aware_scheduler import CacheAwareScheduler, ProviderCandidate


class OpenAIVideoHandler(VideoHandlerBase):
    FORMAT_ID = "OPENAI"

    DEFAULT_BASE_URL = "https://api.openai.com"
    POLL_INTERVAL_SECONDS = 10
    MAX_POLL_COUNT = 360

    def __init__(
        self,
        db: Session,
        user: User,
        api_key: ApiKey,
        request_id: str,
        client_ip: str,
        user_agent: str,
        start_time: float,
        allowed_api_formats: list[str] | None = None,
    ):
        super().__init__(
            db=db,
            user=user,
            api_key=api_key,
            request_id=request_id,
            client_ip=client_ip,
            user_agent=user_agent,
            start_time=start_time,
            allowed_api_formats=allowed_api_formats,
        )
        self._normalizer = OpenAINormalizer()

    async def handle_create_task(
        self,
        *,
        http_request: Request,
        original_headers: dict[str, str],
        original_request_body: dict[str, Any],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        try:
            internal_request = self._normalizer.video_request_to_internal(original_request_body)
        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e))
        candidate = await self._select_candidate(internal_request.model)
        if not candidate:
            raise HTTPException(
                status_code=503, detail="No available provider for video generation"
            )

        upstream_key, endpoint, provider_key = await self._resolve_upstream_key(candidate)
        upstream_url = self._build_upstream_url(endpoint.base_url)
        headers = self._build_upstream_headers(original_headers, upstream_key, endpoint)

        client = await HTTPClientPool.get_default_client_async()
        response = await client.post(upstream_url, headers=headers, json=original_request_body)
        if response.status_code >= 400:
            return self._build_error_response(response)

        payload = response.json()
        external_task_id = str(payload.get("id") or "")
        if not external_task_id:
            raise HTTPException(status_code=502, detail="Upstream returned empty task id")

        task = self._create_task_record(
            external_task_id=external_task_id,
            candidate=candidate,
            original_request_body=original_request_body,
            internal_request=internal_request,
        )
        try:
            self.db.add(task)
            self.db.flush()  # 先 flush 检测冲突
            self.db.commit()
            self.db.refresh(task)
        except IntegrityError:
            self.db.rollback()
            raise HTTPException(status_code=409, detail="Task already exists")

        internal_task = InternalVideoTask(
            id=task.id,
            external_id=external_task_id,
            status=VideoStatus.SUBMITTED,
            created_at=task.created_at,
            original_request=internal_request,
        )
        response_body = self._normalizer.video_task_from_internal(internal_task)
        return JSONResponse(response_body)

    async def handle_get_task(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        task = self._get_task(task_id)
        internal_task = self._task_to_internal(task)
        response_body = self._normalizer.video_task_from_internal(internal_task)
        return JSONResponse(response_body)

    async def handle_list_tasks(
        self,
        *,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        tasks = (
            self.db.query(VideoTask)
            .filter(VideoTask.user_id == self.user.id)
            .order_by(VideoTask.created_at.desc())
            .limit(100)
            .all()
        )
        items = [
            self._normalizer.video_task_from_internal(self._task_to_internal(t)) for t in tasks
        ]
        return JSONResponse({"object": "list", "data": items})

    async def handle_cancel_task(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        task = self._get_task(task_id)
        if not task.external_task_id:
            raise HTTPException(status_code=500, detail="Task missing external_task_id")
        endpoint, key = self._get_endpoint_and_key(task)
        if not key.api_key:
            raise HTTPException(status_code=500, detail="Provider key not configured")
        upstream_key = crypto_service.decrypt(key.api_key)

        upstream_url = self._build_upstream_url(endpoint.base_url, task.external_task_id)
        headers = self._build_upstream_headers(original_headers, upstream_key, endpoint)

        client = await HTTPClientPool.get_default_client_async()
        response = await client.delete(upstream_url, headers=headers)
        if response.status_code >= 400:
            return self._build_error_response(response)

        task.status = VideoStatus.CANCELLED.value
        task.updated_at = datetime.now(timezone.utc)
        self.db.commit()
        return JSONResponse({})

    async def handle_download_content(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> Response | StreamingResponse:
        task = self._get_task(task_id)

        # 根据本地任务状态提前返回适当错误（避免不必要的上游请求）
        if task.status in (
            VideoStatus.PENDING.value,
            VideoStatus.SUBMITTED.value,
            VideoStatus.QUEUED.value,
            VideoStatus.PROCESSING.value,
        ):
            raise HTTPException(
                status_code=202,
                detail=f"Video is still processing (status: {task.status})",
            )
        if task.status == VideoStatus.FAILED.value:
            raise HTTPException(
                status_code=422,
                detail=f"Video generation failed: {task.error_message or 'Unknown error'}",
            )
        if task.status == VideoStatus.CANCELLED.value:
            raise HTTPException(status_code=404, detail="Video task was cancelled")

        if not task.external_task_id:
            raise HTTPException(status_code=500, detail="Task missing external_task_id")
        endpoint, key = self._get_endpoint_and_key(task)
        if not key.api_key:
            raise HTTPException(status_code=500, detail="Provider key not configured")
        upstream_key = crypto_service.decrypt(key.api_key)

        upstream_url = self._build_upstream_url(
            endpoint.base_url, f"{task.external_task_id}/content"
        )
        headers = self._build_upstream_headers(original_headers, upstream_key, endpoint)

        client = await HTTPClientPool.get_default_client_async()
        try:
            # 使用 httpx 的 stream 方法并正确管理上下文
            # 视频下载可能较大，设置 5 分钟超时
            request = client.build_request("GET", upstream_url, headers=headers)
            response = await client.send(request, stream=True, timeout=300.0)
        except Exception as exc:
            logger.warning(
                "[VideoDownload] Upstream connection failed task=%s: %s",
                task_id,
                sanitize_error_message(str(exc)),
            )
            raise HTTPException(status_code=502, detail="Upstream connection failed") from exc

        if response.status_code >= 400:
            error_body = await response.aread()
            await response.aclose()
            content_type = response.headers.get("content-type", "")
            if "application/json" in content_type:
                try:
                    data = json.loads(error_body)
                    # 脱敏：移除可能的敏感信息
                    if isinstance(data, dict) and isinstance(data.get("error"), dict):
                        if "message" in data["error"]:
                            data["error"]["message"] = sanitize_error_message(
                                str(data["error"]["message"])
                            )
                    return JSONResponse(status_code=response.status_code, content=data)
                except json.JSONDecodeError:
                    pass
            message = sanitize_error_message(error_body.decode(errors="ignore"))
            return JSONResponse(
                status_code=response.status_code,
                content={"error": {"type": "upstream_error", "message": message}},
            )

        async def _iter_bytes() -> AsyncIterator[bytes]:
            try:
                async for chunk in response.aiter_bytes():
                    yield chunk
            finally:
                await response.aclose()

        # 过滤 hop-by-hop 和系统管理头部
        safe_headers = {
            k: v for k, v in response.headers.items() if k.lower() not in HOP_BY_HOP_HEADERS
        }
        return StreamingResponse(
            _iter_bytes(),
            status_code=response.status_code,
            headers=safe_headers,
            media_type=response.headers.get("content-type", "application/octet-stream"),
        )

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    async def _select_candidate(self, model_name: str) -> ProviderCandidate | None:
        scheduler = CacheAwareScheduler()
        candidates, _ = await scheduler.list_all_candidates(
            db=self.db,
            api_format=APIFormat.OPENAI,
            model_name=model_name,
            affinity_key=str(self.api_key.id),
            user_api_key=self.api_key,
            max_candidates=10,
        )
        for candidate in candidates:
            auth_type = getattr(candidate.key, "auth_type", "api_key") or "api_key"
            if auth_type == "api_key":
                return candidate
        return None

    async def _resolve_upstream_key(
        self, candidate: ProviderCandidate
    ) -> tuple[str, ProviderEndpoint, ProviderAPIKey]:
        try:
            upstream_key = crypto_service.decrypt(candidate.key.api_key)
        except Exception as exc:
            logger.error(
                "Failed to decrypt provider key id=%s: %s",
                candidate.key.id,
                sanitize_error_message(str(exc)),
            )
            raise HTTPException(status_code=500, detail="Failed to decrypt provider key")
        return upstream_key, candidate.endpoint, candidate.key

    def _build_upstream_url(self, base_url: str | None, suffix: str | None = None) -> str:
        base = (base_url or self.DEFAULT_BASE_URL).rstrip("/")
        if base.endswith("/v1"):
            url = f"{base}/videos"
        else:
            url = f"{base}/v1/videos"
        if suffix:
            return f"{url}/{suffix}"
        return url

    def _build_upstream_headers(
        self, original_headers: dict[str, str], upstream_key: str, endpoint: ProviderEndpoint
    ) -> dict[str, str]:
        extra_headers = get_extra_headers_from_endpoint(endpoint)
        return build_upstream_headers(
            original_headers,
            APIFormat.OPENAI,
            upstream_key,
            endpoint_headers=extra_headers,
        )

    # _build_error_response 继承自基类 VideoHandlerBase

    def _create_task_record(
        self,
        *,
        external_task_id: str,
        candidate: ProviderCandidate,
        original_request_body: dict[str, Any],
        internal_request: InternalVideoRequest,
    ) -> VideoTask:
        now = datetime.now(timezone.utc)
        size = internal_request.extra.get("original_size")
        return VideoTask(
            id=str(uuid4()),
            external_task_id=external_task_id,
            user_id=self.user.id,
            api_key_id=self.api_key.id,
            provider_id=candidate.provider.id,
            endpoint_id=candidate.endpoint.id,
            key_id=candidate.key.id,
            client_api_format="OPENAI",
            provider_api_format=str(candidate.endpoint.api_format),
            format_converted=False,
            model=internal_request.model,
            prompt=internal_request.prompt,
            original_request_body=original_request_body,
            converted_request_body=original_request_body,
            duration_seconds=internal_request.duration_seconds,
            resolution=internal_request.resolution,
            aspect_ratio=internal_request.aspect_ratio,
            size=size,
            status=VideoStatus.SUBMITTED.value,
            progress_percent=0,
            poll_interval_seconds=self.POLL_INTERVAL_SECONDS,
            next_poll_at=now + timedelta(seconds=self.POLL_INTERVAL_SECONDS),
            poll_count=0,
            max_poll_count=self.MAX_POLL_COUNT,
            submitted_at=now,
        )

    def _task_to_internal(self, task: VideoTask) -> InternalVideoTask:
        try:
            status = VideoStatus(task.status)
        except ValueError:
            status = VideoStatus.PENDING
        return InternalVideoTask(
            id=task.id,
            external_id=task.external_task_id,
            status=status,
            progress_percent=task.progress_percent or 0,
            progress_message=task.progress_message,
            video_url=task.video_url,
            video_urls=task.video_urls or [],
            thumbnail_url=task.thumbnail_url,
            video_duration_seconds=task.duration_seconds,
            video_size_bytes=task.video_size_bytes,
            created_at=task.created_at,
            completed_at=task.completed_at,
            expires_at=task.video_expires_at,
            error_code=task.error_code,
            error_message=task.error_message,
            extra={"model": task.model, "size": task.size, "seconds": task.duration_seconds},
        )


__all__ = ["OpenAIVideoHandler"]
