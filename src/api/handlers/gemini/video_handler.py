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
from src.api.handlers.base.video_handler_base import (
    VideoHandlerBase,
    normalize_gemini_operation_id,
    sanitize_error_message,
)
from src.clients.http_client import HTTPClientPool
from src.config.settings import config
from src.core.api_format import (
    ApiFamily,
    EndpointKind,
    build_upstream_headers_for_endpoint,
    get_extra_headers_from_endpoint,
    make_signature_key,
)
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
from src.services.billing.rule_service import BillingRuleLookupResult, BillingRuleService
from src.services.cache.aware_scheduler import ProviderCandidate


class GeminiVeoHandler(VideoHandlerBase):
    FORMAT_ID = "gemini:video"
    API_FAMILY = ApiFamily.GEMINI
    ENDPOINT_KIND = EndpointKind.VIDEO

    DEFAULT_BASE_URL = "https://generativelanguage.googleapis.com"

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

        async def _submit(candidate: ProviderCandidate) -> Any:
            upstream_key, endpoint, _key, auth_info = await self._resolve_upstream_key(candidate)
            upstream_url = self._build_upstream_url(endpoint.base_url, internal_request.model)
            headers = self._build_upstream_headers(
                original_headers, upstream_key, endpoint, auth_info
            )
            client = await HTTPClientPool.get_default_client_async()
            return await client.post(upstream_url, headers=headers, json=original_request_body)

        def _extract_task_id(payload: dict[str, Any]) -> str | None:
            value = payload.get("name")
            if not value:
                return None
            return normalize_gemini_operation_id(str(value))

        outcome_or_response = await self._submit_with_failover(
            api_format=self.FORMAT_ID,
            model_name=internal_request.model,
            task_type="video",
            submit_func=_submit,
            extract_external_task_id=_extract_task_id,
            supported_auth_types={"api_key", "vertex_ai"},
            allow_format_conversion=False,
            max_candidates=10,
        )
        if isinstance(outcome_or_response, JSONResponse):
            return outcome_or_response
        outcome = outcome_or_response

        # 冻结 billing_rule 配置（用于异步任务的成本一致性）
        # 复用 _select_candidate 中已查询的结果；billing_require_rule=false 时需补查
        rule_lookup = outcome.rule_lookup
        if rule_lookup is None:
            rule_lookup = BillingRuleService.find_rule(
                self.db,
                provider_id=outcome.candidate.provider.id,
                model_name=internal_request.model,
                task_type="video",
            )
        billing_rule_snapshot = self._build_billing_rule_snapshot(rule_lookup)

        external_task_id = outcome.external_task_id

        task = self._create_task_record(
            external_task_id=external_task_id,
            candidate=outcome.candidate,
            original_request_body=original_request_body,
            internal_request=internal_request,
            candidate_keys=outcome.candidate_keys,
            original_headers=original_headers,
            billing_rule_snapshot=billing_rule_snapshot,
        )
        try:
            self.db.add(task)
            self.db.flush()  # 先 flush 检测冲突
            self.db.commit()
            self.db.refresh(task)
            logger.info(
                f"[GeminiVeoHandler] Task created: id={task.id}, external_task_id={task.external_task_id}, user_id={task.user_id}"
            )
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

        # 直接从数据库返回任务状态（后台轮询服务会持续更新状态）
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
        endpoint_sig = make_signature_key(
            str(getattr(endpoint, "api_family", "")).strip().lower(),
            str(getattr(endpoint, "endpoint_kind", "")).strip().lower(),
        )
        headers = build_upstream_headers_for_endpoint(
            original_headers,
            endpoint_sig,
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
        candidate_keys: list[dict[str, Any]] | None = None,
        original_headers: dict[str, str] | None = None,
        billing_rule_snapshot: dict[str, Any] | None = None,
    ) -> VideoTask:
        now = datetime.now(timezone.utc)

        # 构建请求元数据（使用追踪信息）
        request_metadata = {
            "candidate_keys": candidate_keys or [],
            "selected_key_id": candidate.key.id,
            "selected_endpoint_id": candidate.endpoint.id,
            "client_ip": self.client_ip,
            "user_agent": self.user_agent,
            "request_id": self.request_id,
            "billing_rule_snapshot": billing_rule_snapshot,
        }
        # 记录请求头（脱敏处理）
        if original_headers:
            safe_headers = {
                k: v
                for k, v in original_headers.items()
                if k.lower() not in {"authorization", "x-api-key", "x-goog-api-key", "cookie"}
            }
            request_metadata["request_headers"] = safe_headers

        return VideoTask(
            id=str(uuid4()),
            external_task_id=external_task_id,
            user_id=self.user.id,
            api_key_id=self.api_key.id,
            provider_id=candidate.provider.id,
            endpoint_id=candidate.endpoint.id,
            key_id=candidate.key.id,
            client_api_format=self.FORMAT_ID,
            provider_api_format=make_signature_key(
                str(getattr(candidate.endpoint, "api_family", "")).strip().lower(),
                str(getattr(candidate.endpoint, "endpoint_kind", "")).strip().lower(),
            ),
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
            poll_interval_seconds=config.video_poll_interval_seconds,
            next_poll_at=now + timedelta(seconds=config.video_poll_interval_seconds),
            poll_count=0,
            max_poll_count=config.video_max_poll_count,
            submitted_at=now,
            request_metadata=request_metadata,
        )

    def _get_task_by_external_id(self, external_id: str) -> VideoTask:
        """按 external_task_id 查找任务（Gemini 使用 operations/{id} 格式）"""
        normalized_id = normalize_gemini_operation_id(external_id)

        logger.info(
            f"[GeminiVeoHandler] Looking for task: normalized_id={normalized_id}, user_id={self.user.id}"
        )

        task = (
            self.db.query(VideoTask)
            .filter(
                VideoTask.external_task_id == normalized_id,
                VideoTask.user_id == self.user.id,
            )
            .first()
        )
        if not task:
            logger.warning(
                f"[GeminiVeoHandler] Task not found: normalized_id={normalized_id}, user_id={self.user.id}"
            )
            raise HTTPException(status_code=404, detail="Video task not found")
        logger.info(
            f"[GeminiVeoHandler] Task found: id={task.id}, external_task_id={task.external_task_id}"
        )
        return task


__all__ = ["GeminiVeoHandler"]
