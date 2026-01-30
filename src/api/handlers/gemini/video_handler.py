"""
Gemini Video Handler - Veo 视频生成实现
"""

from __future__ import annotations

from datetime import datetime, timedelta, timezone
from typing import Any, AsyncIterator
from uuid import uuid4

from fastapi import HTTPException, Request
from fastapi.responses import JSONResponse, Response, StreamingResponse
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from src.api.handlers.base.request_builder import get_provider_auth
from src.api.handlers.base.video_handler_base import VideoHandlerBase, sanitize_error_message
from src.clients.http_client import HTTPClientPool
from src.core.api_format import APIFormat, build_upstream_headers, get_extra_headers_from_endpoint
from src.core.api_format.conversion.internal_video import (
    InternalVideoRequest,
    InternalVideoTask,
    VideoStatus,
)
from src.core.api_format.conversion.normalizers.gemini import GeminiNormalizer
from src.core.api_format.headers import HOP_BY_HOP_HEADERS
from src.core.crypto import crypto_service
from src.core.logger import logger
from src.models.database import ApiKey, ProviderAPIKey, ProviderEndpoint, User, VideoTask
from src.services.cache.aware_scheduler import CacheAwareScheduler, ProviderCandidate


class GeminiVeoHandler(VideoHandlerBase):
    FORMAT_ID = "GEMINI"

    DEFAULT_BASE_URL = "https://generativelanguage.googleapis.com"
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
        self._normalizer = GeminiNormalizer()

    async def handle_create_task(
        self,
        *,
        http_request: Request,
        original_headers: dict[str, str],
        original_request_body: dict[str, Any],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        # 将路径中的 model 合并到请求体再解析
        model = path_params.get("model") if path_params else None
        request_with_model = {**original_request_body}
        if model:
            request_with_model["model"] = str(model)

        try:
            internal_request = self._normalizer.video_request_to_internal(request_with_model)
        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e))

        candidate = await self._select_candidate(internal_request.model)
        if not candidate:
            raise HTTPException(
                status_code=503, detail="No available provider for video generation"
            )

        upstream_key, endpoint, key, auth_info = await self._resolve_upstream_key(candidate)
        upstream_url = self._build_upstream_url(endpoint.base_url, internal_request.model)
        headers = self._build_upstream_headers(original_headers, upstream_key, endpoint, auth_info)

        client = await HTTPClientPool.get_default_client_async()
        response = await client.post(upstream_url, headers=headers, json=original_request_body)
        if response.status_code >= 400:
            return self._build_error_response(response)

        payload = response.json()
        external_task_id = str(payload.get("name") or "")
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
        # Gemini 使用 operations/{id} 格式，需要按 external_task_id 查找
        task = self._get_task_by_external_id(task_id)
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
        return JSONResponse({"operations": items})

    async def handle_cancel_task(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        task = self._get_task_by_external_id(task_id)
        if not task.external_task_id:
            raise HTTPException(status_code=500, detail="Task missing external_task_id")
        endpoint, key = self._get_endpoint_and_key(task)
        if not key.api_key:
            raise HTTPException(status_code=500, detail="Provider key not configured")
        upstream_key = crypto_service.decrypt(key.api_key)

        operation_name = task.external_task_id
        if not operation_name.startswith("operations/"):
            operation_name = f"operations/{operation_name}"
        upstream_url = self._build_cancel_url(endpoint.base_url, operation_name)
        auth_info = await get_provider_auth(endpoint, key)
        headers = self._build_upstream_headers(original_headers, upstream_key, endpoint, auth_info)

        client = await HTTPClientPool.get_default_client_async()
        response = await client.post(upstream_url, headers=headers, json={})
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
        task = self._get_task_by_external_id(task_id)

        # 根据任务状态返回不同的错误码
        if not task.video_url:
            if task.status in (
                VideoStatus.PENDING.value,
                VideoStatus.SUBMITTED.value,
                VideoStatus.QUEUED.value,
                VideoStatus.PROCESSING.value,
            ):
                # 任务仍在处理中，返回 202 Accepted
                raise HTTPException(
                    status_code=202,
                    detail=f"Video is still processing (status: {task.status})",
                )
            if task.status == VideoStatus.FAILED.value:
                raise HTTPException(
                    status_code=422,
                    detail=f"Video generation failed: {task.error_message or 'Unknown error'}",
                )
            # 其他状态（如 CANCELLED）
            raise HTTPException(status_code=404, detail="Video not available")

        # 检查视频是否已过期
        if task.video_expires_at:
            now = datetime.now(timezone.utc)
            if task.video_expires_at < now:
                raise HTTPException(status_code=410, detail="Video URL has expired")

        # 代理下载而非直接重定向，避免暴露上游存储 URL
        client = await HTTPClientPool.get_default_client_async()
        try:
            request = client.build_request("GET", task.video_url)
            # 视频下载可能较大，设置 5 分钟超时
            response = await client.send(request, stream=True, timeout=300.0)
        except Exception as exc:
            logger.error(
                "[VideoDownload] Upstream fetch failed user=%s task=%s: %s",
                self.user.id,
                task.id,
                sanitize_error_message(str(exc)),
            )
            raise HTTPException(status_code=502, detail="Failed to fetch video")

        if response.status_code >= 400:
            await response.aclose()
            raise HTTPException(status_code=response.status_code, detail="Upstream error")

        async def _iter_bytes() -> AsyncIterator[bytes]:
            try:
                async for chunk in response.aiter_bytes():
                    yield chunk
            finally:
                await response.aclose()

        safe_headers = {
            k: v for k, v in response.headers.items() if k.lower() not in HOP_BY_HOP_HEADERS
        }
        return StreamingResponse(
            _iter_bytes(),
            status_code=response.status_code,
            headers=safe_headers,
            media_type=response.headers.get("content-type", "video/mp4"),
        )

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    async def _select_candidate(self, model_name: str) -> ProviderCandidate | None:
        scheduler = CacheAwareScheduler()
        candidates, _ = await scheduler.list_all_candidates(
            db=self.db,
            api_format=APIFormat.GEMINI,
            model_name=model_name,
            affinity_key=str(self.api_key.id),
            user_api_key=self.api_key,
            max_candidates=10,
        )
        for candidate in candidates:
            auth_type = getattr(candidate.key, "auth_type", "api_key") or "api_key"
            if auth_type in {"api_key", "vertex_ai"}:
                return candidate
        return None

    async def _resolve_upstream_key(
        self, candidate: ProviderCandidate
    ) -> tuple[str, ProviderEndpoint, ProviderAPIKey, Any | None]:
        try:
            upstream_key = crypto_service.decrypt(candidate.key.api_key)
        except Exception as exc:
            logger.error(
                "Failed to decrypt provider key id=%s: %s",
                candidate.key.id,
                sanitize_error_message(str(exc)),
            )
            raise HTTPException(status_code=500, detail="Failed to decrypt provider key")

        auth_info = await get_provider_auth(candidate.endpoint, candidate.key)
        return upstream_key, candidate.endpoint, candidate.key, auth_info

    def _build_upstream_url(self, base_url: str | None, model: str) -> str:
        base = (base_url or self.DEFAULT_BASE_URL).rstrip("/")
        if base.endswith("/v1beta"):
            base = base[: -len("/v1beta")]
        return f"{base}/v1beta/models/{model}:predictLongRunning"

    def _build_cancel_url(self, base_url: str | None, operation_name: str) -> str:
        base = (base_url or self.DEFAULT_BASE_URL).rstrip("/")
        if base.endswith("/v1beta"):
            base = base[: -len("/v1beta")]
        return f"{base}/v1beta/{operation_name}:cancel"

    def _build_upstream_headers(
        self,
        original_headers: dict[str, str],
        upstream_key: str,
        endpoint: ProviderEndpoint,
        auth_info: Any | None,
    ) -> dict[str, str]:
        extra_headers = get_extra_headers_from_endpoint(endpoint)
        headers = build_upstream_headers(
            original_headers,
            APIFormat.GEMINI,
            upstream_key,
            endpoint_headers=extra_headers,
        )
        if auth_info:
            # 覆盖为 OAuth2 Bearer（Vertex AI）
            headers.pop("x-goog-api-key", None)
            headers[auth_info.auth_header] = auth_info.auth_value
        return headers

    def _format_error_payload(self, error: dict[str, Any], status_code: int) -> dict[str, Any]:
        """Gemini 风格错误格式"""
        return {
            "code": error.get("code", status_code),
            "message": sanitize_error_message(error.get("message", "Request failed")),
            "status": error.get("status", "BAD_GATEWAY"),
        }

    def _create_task_record(
        self,
        *,
        external_task_id: str,
        candidate: ProviderCandidate,
        original_request_body: dict[str, Any],
        internal_request: Any,
    ) -> VideoTask:
        now = datetime.now(timezone.utc)
        return VideoTask(
            id=str(uuid4()),
            external_task_id=external_task_id,
            user_id=self.user.id,
            api_key_id=self.api_key.id,
            provider_id=candidate.provider.id,
            endpoint_id=candidate.endpoint.id,
            key_id=candidate.key.id,
            client_api_format="GEMINI",
            provider_api_format=str(candidate.endpoint.api_format),
            format_converted=False,
            model=internal_request.model,
            prompt=internal_request.prompt,
            original_request_body=original_request_body,
            converted_request_body=original_request_body,
            duration_seconds=internal_request.duration_seconds,
            resolution=internal_request.resolution,
            aspect_ratio=internal_request.aspect_ratio,
            status=VideoStatus.SUBMITTED.value,
            progress_percent=0,
            poll_interval_seconds=self.POLL_INTERVAL_SECONDS,
            next_poll_at=now + timedelta(seconds=self.POLL_INTERVAL_SECONDS),
            poll_count=0,
            max_poll_count=self.MAX_POLL_COUNT,
            submitted_at=now,
        )

    def _get_task_by_external_id(self, external_id: str) -> VideoTask:
        """按 external_task_id 查找任务（Gemini 使用 operations/{id} 格式）"""
        normalized_id = external_id
        if not normalized_id.startswith("operations/"):
            normalized_id = f"operations/{normalized_id}"

        task = (
            self.db.query(VideoTask)
            .filter(
                VideoTask.external_task_id == normalized_id,
                VideoTask.user_id == self.user.id,
            )
            .first()
        )
        if not task:
            raise HTTPException(status_code=404, detail="Video task not found")
        return task


__all__ = ["GeminiVeoHandler"]
