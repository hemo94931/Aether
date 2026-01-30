"""
视频任务后台轮询服务
"""

from __future__ import annotations

import asyncio
from datetime import datetime, timedelta, timezone
from uuid import uuid4

from sqlalchemy.orm import Session

from src.api.handlers.base.request_builder import ProviderAuthInfo, get_provider_auth
from src.api.handlers.base.video_handler_base import sanitize_error_message
from src.clients.http_client import HTTPClientPool
from src.clients.redis_client import get_redis_client
from src.core.api_format import APIFormat, build_upstream_headers, get_extra_headers_from_endpoint
from src.core.api_format.conversion.internal_video import InternalVideoPollResult, VideoStatus
from src.core.api_format.conversion.normalizers.gemini import GeminiNormalizer
from src.core.api_format.conversion.normalizers.openai import OpenAINormalizer
from src.core.crypto import crypto_service
from src.core.logger import logger
from src.database import create_session
from src.models.database import ProviderAPIKey, ProviderEndpoint, VideoTask
from src.services.system.scheduler import get_scheduler

# 永久性错误指示词（用于降级判断，不应重试）
_PERMANENT_ERROR_INDICATORS = frozenset(
    {
        "not found",
        "404",
        "unauthorized",
        "401",
        "forbidden",
        "403",
        "invalid request",
        "invalid api key",
        "does not exist",
    }
)


class PollHTTPError(RuntimeError):
    """HTTP 轮询错误，携带状态码便于区分临时/永久错误"""

    def __init__(self, status_code: int, message: str):
        super().__init__(message)
        self.status_code = status_code


class VideoTaskPollerService:
    """后台轮询视频生成任务状态"""

    LOCK_KEY = "video_task_poller:lock"
    LOCK_TTL = 60
    BATCH_SIZE = 50
    MAX_BACKOFF_SECONDS = 300
    # 连续失败告警阈值
    CONSECUTIVE_FAILURE_ALERT_THRESHOLD = 5

    def __init__(self) -> None:
        self._lock = asyncio.Lock()
        self.redis = None
        self._openai_normalizer = OpenAINormalizer()
        self._gemini_normalizer = GeminiNormalizer()
        # 追踪连续失败次数（用于告警）
        self._consecutive_failures = 0

    async def start(self) -> None:
        if self.redis is None:
            self.redis = await get_redis_client(require_redis=False)

        scheduler = get_scheduler()
        scheduler.add_interval_job(
            self.poll_pending_tasks,
            seconds=10,
            job_id="video_task_poller",
            name="视频任务轮询",
        )

    async def stop(self) -> None:
        """停止轮询服务"""
        scheduler = get_scheduler()
        scheduler.remove_job("video_task_poller")

    async def poll_pending_tasks(self) -> None:
        async with self._lock:
            token = await self._acquire_redis_lock()
            if token is None:
                return

            try:
                with create_session() as db:
                    now = datetime.now(timezone.utc)
                    tasks = (
                        db.query(VideoTask)
                        .filter(
                            VideoTask.status.in_(
                                [
                                    VideoStatus.SUBMITTED.value,
                                    VideoStatus.QUEUED.value,
                                    VideoStatus.PROCESSING.value,
                                ]
                            ),
                            VideoTask.next_poll_at <= now,
                            VideoTask.poll_count < VideoTask.max_poll_count,
                        )
                        .order_by(VideoTask.next_poll_at.asc())
                        .limit(self.BATCH_SIZE)
                        .all()
                    )

                    if not tasks:
                        # 无任务时重置连续失败计数
                        self._consecutive_failures = 0
                        return

                    batch_failures = 0
                    for task in tasks:
                        try:
                            await self._poll_single_task(db, task)
                        except Exception as exc:
                            batch_failures += 1
                            # 单个任务失败不影响其他任务处理
                            logger.exception(
                                "Unexpected error polling task %s: %s",
                                task.id,
                                sanitize_error_message(str(exc)),
                            )

                    # 更新连续失败计数并检查告警阈值
                    if batch_failures == len(tasks):
                        self._consecutive_failures += 1
                        if self._consecutive_failures >= self.CONSECUTIVE_FAILURE_ALERT_THRESHOLD:
                            logger.error(
                                "[ALERT] Video task poller: %d consecutive batches failed. "
                                "Provider connectivity or configuration issue suspected.",
                                self._consecutive_failures,
                            )
                    else:
                        self._consecutive_failures = 0

                    db.commit()
            finally:
                await self._release_redis_lock(token)

    async def _poll_single_task(self, db: Session, task: VideoTask) -> None:
        try:
            result = await self._poll_task_status(db, task)
            if result.status == VideoStatus.COMPLETED:
                task.status = VideoStatus.COMPLETED.value
                task.video_url = result.video_url
                task.video_expires_at = result.expires_at
                task.completed_at = datetime.now(timezone.utc)
                task.progress_percent = 100
                # 存储多视频 URL（Gemini sampleCount > 1 时）
                if result.video_urls:
                    task.video_urls = result.video_urls
            elif result.status == VideoStatus.FAILED:
                task.status = VideoStatus.FAILED.value
                task.error_code = result.error_code
                task.error_message = result.error_message
                task.completed_at = datetime.now(timezone.utc)
            else:
                task.poll_count += 1
                task.progress_percent = result.progress_percent
                task.next_poll_at = datetime.now(timezone.utc) + timedelta(
                    seconds=task.poll_interval_seconds
                )
        except Exception as exc:
            task.poll_count += 1
            error_msg = sanitize_error_message(str(exc))
            logger.warning("Poll error for task %s: %s", task.id, error_msg)
            task.progress_message = f"Poll error: {error_msg}"

            # 区分临时性错误和永久性错误
            status_code = exc.status_code if isinstance(exc, PollHTTPError) else None
            is_permanent = self._is_permanent_error(exc, status_code=status_code)
            if is_permanent:
                task.status = VideoStatus.FAILED.value
                task.error_code = "poll_permanent_error"
                task.error_message = error_msg
                task.completed_at = datetime.now(timezone.utc)
            else:
                # 临时性错误：指数退避重试
                backoff = min(
                    task.poll_interval_seconds * (2 ** min(task.retry_count, 5)),
                    self.MAX_BACKOFF_SECONDS,
                )
                task.retry_count += 1
                task.next_poll_at = datetime.now(timezone.utc) + timedelta(seconds=backoff)

        # 检查是否超过最大轮询次数（超时）
        task.updated_at = datetime.now(timezone.utc)
        if task.poll_count >= task.max_poll_count and task.status not in [
            VideoStatus.COMPLETED.value,
            VideoStatus.FAILED.value,
            VideoStatus.CANCELLED.value,
        ]:
            task.status = VideoStatus.FAILED.value
            task.error_code = "poll_timeout"
            task.error_message = f"Task timed out after {task.poll_count} polls"
            task.completed_at = datetime.now(timezone.utc)

    def _is_permanent_error(self, exc: Exception, status_code: int | None = None) -> bool:
        """判断是否为永久性错误（不应重试）"""
        # 优先使用 HTTP 状态码判断
        if status_code is not None:
            # 4xx 客户端错误（除 429 限流）通常是永久性错误
            return 400 <= status_code < 500 and status_code != 429

        # 降级到字符串匹配
        error_msg = str(exc).lower()
        return any(indicator in error_msg for indicator in _PERMANENT_ERROR_INDICATORS)

    async def _poll_task_status(self, db: Session, task: VideoTask) -> InternalVideoPollResult:
        if not task.endpoint_id or not task.key_id:
            return InternalVideoPollResult(
                status=VideoStatus.FAILED,
                error_code="missing_provider_info",
                error_message="Task missing endpoint_id or key_id",
            )
        endpoint = self._get_endpoint(db, task.endpoint_id)
        key = self._get_key(db, task.key_id)
        if not key.api_key:
            return InternalVideoPollResult(
                status=VideoStatus.FAILED,
                error_code="provider_config_error",
                error_message="Provider key not properly configured",
            )
        try:
            upstream_key = crypto_service.decrypt(key.api_key)
        except Exception:
            logger.warning("Failed to decrypt provider key for task %s", task.id)
            return InternalVideoPollResult(
                status=VideoStatus.FAILED,
                error_code="decryption_error",
                error_message="Failed to decrypt provider key",
            )

        if (task.provider_api_format or "").upper() == "GEMINI":
            auth_info = await get_provider_auth(endpoint, key)
            return await self._poll_gemini(task, endpoint, upstream_key, auth_info)
        return await self._poll_openai(task, endpoint, upstream_key)

    async def _poll_openai(
        self,
        task: VideoTask,
        endpoint: ProviderEndpoint,
        upstream_key: str,
    ) -> InternalVideoPollResult:
        if not task.external_task_id:
            return InternalVideoPollResult(
                status=VideoStatus.FAILED,
                error_code="missing_external_task_id",
                error_message="Task missing external_task_id",
            )
        url = self._build_openai_url(endpoint.base_url, task.external_task_id)
        headers = self._build_headers(APIFormat.OPENAI, upstream_key, endpoint)

        client = await HTTPClientPool.get_default_client_async()
        response = await client.get(url, headers=headers)
        if response.status_code >= 400:
            raise PollHTTPError(
                response.status_code,
                sanitize_error_message(response.text or "Poll error"),
            )

        payload = response.json()
        return self._openai_normalizer.video_poll_to_internal(payload)

    async def _poll_gemini(
        self,
        task: VideoTask,
        endpoint: ProviderEndpoint,
        upstream_key: str,
        auth_info: ProviderAuthInfo | None,
    ) -> InternalVideoPollResult:
        if not task.external_task_id:
            return InternalVideoPollResult(
                status=VideoStatus.FAILED,
                error_code="missing_external_task_id",
                error_message="Task missing external_task_id",
            )
        operation_name = task.external_task_id
        if not operation_name.startswith("operations/"):
            operation_name = f"operations/{operation_name}"
        url = self._build_gemini_url(endpoint.base_url, operation_name)
        headers = self._build_headers(APIFormat.GEMINI, upstream_key, endpoint, auth_info)

        client = await HTTPClientPool.get_default_client_async()
        response = await client.get(url, headers=headers)
        if response.status_code >= 400:
            raise PollHTTPError(
                response.status_code,
                sanitize_error_message(response.text or "Poll error"),
            )

        payload = response.json()
        return self._gemini_normalizer.video_poll_to_internal(payload)

    def _build_openai_url(self, base_url: str | None, task_id: str) -> str:
        base = (base_url or "https://api.openai.com").rstrip("/")
        if base.endswith("/v1"):
            return f"{base}/videos/{task_id}"
        return f"{base}/v1/videos/{task_id}"

    def _build_gemini_url(self, base_url: str | None, operation_name: str) -> str:
        base = (base_url or "https://generativelanguage.googleapis.com").rstrip("/")
        if base.endswith("/v1beta"):
            base = base[: -len("/v1beta")]
        return f"{base}/v1beta/{operation_name}"

    def _build_headers(
        self,
        api_format: APIFormat,
        upstream_key: str,
        endpoint: ProviderEndpoint,
        auth_info: ProviderAuthInfo | None = None,
    ) -> dict[str, str]:
        extra_headers = get_extra_headers_from_endpoint(endpoint)
        headers = build_upstream_headers(
            {},
            api_format,
            upstream_key,
            endpoint_headers=extra_headers,
        )
        if auth_info:
            headers.pop("x-goog-api-key", None)
            headers[auth_info.auth_header] = auth_info.auth_value
        return headers

    def _get_endpoint(self, db: Session, endpoint_id: str) -> ProviderEndpoint:
        endpoint = db.query(ProviderEndpoint).filter(ProviderEndpoint.id == endpoint_id).first()
        if not endpoint:
            raise RuntimeError("Provider endpoint not found")
        return endpoint

    def _get_key(self, db: Session, key_id: str) -> ProviderAPIKey:
        key = db.query(ProviderAPIKey).filter(ProviderAPIKey.id == key_id).first()
        if not key:
            raise RuntimeError("Provider key not found")
        return key

    async def _acquire_redis_lock(self) -> str | None:
        if not self.redis:
            return "no_redis"
        token = str(uuid4())
        acquired = await self.redis.set(self.LOCK_KEY, token, nx=True, ex=self.LOCK_TTL)
        return token if acquired else None

    async def _release_redis_lock(self, token: str) -> None:
        if not self.redis or token == "no_redis":
            return
        script = """
        if redis.call('GET', KEYS[1]) == ARGV[1] then
            return redis.call('DEL', KEYS[1])
        end
        return 0
        """
        await self.redis.eval(script, 1, self.LOCK_KEY, token)


_video_task_poller: VideoTaskPollerService | None = None


def get_video_task_poller() -> VideoTaskPollerService:
    global _video_task_poller
    if _video_task_poller is None:
        _video_task_poller = VideoTaskPollerService()
    return _video_task_poller
