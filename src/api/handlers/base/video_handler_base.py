"""
Video Handler 基类

定义视频生成相关操作的统一接口。
"""

from __future__ import annotations

import re
from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any

from fastapi import HTTPException, Request
from fastapi.responses import JSONResponse, Response, StreamingResponse
from sqlalchemy.orm import Session

from src.core.api_format.conversion.internal_video import InternalVideoTask, VideoStatus
from src.models.database import ApiKey, ProviderAPIKey, ProviderEndpoint, User, VideoTask

if TYPE_CHECKING:
    import httpx

# 敏感信息匹配正则（预编译提升性能）
_SENSITIVE_PATTERN = re.compile(
    r"(api[_-]?key|token|bearer|authorization)[=:\s]+\S+",
    re.IGNORECASE,
)


def sanitize_error_message(message: str, max_length: int = 200) -> str:
    """
    移除错误消息中可能包含的敏感信息

    Args:
        message: 原始错误消息
        max_length: 最大长度，默认 200

    Returns:
        脱敏后的消息
    """
    if not message:
        return "Request failed"
    # 先脱敏再截断，确保敏感信息不会因截断位置而泄露
    sanitized = _SENSITIVE_PATTERN.sub("[REDACTED]", message)
    return sanitized[:max_length]


class VideoHandlerBase(ABC):
    """视频处理器基类"""

    FORMAT_ID: str = ""

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
        self.db = db
        self.user = user
        self.api_key = api_key
        self.request_id = request_id
        self.client_ip = client_ip
        self.user_agent = user_agent
        self.start_time = start_time
        self.allowed_api_formats = allowed_api_formats or [self.FORMAT_ID]

    @abstractmethod
    async def handle_create_task(
        self,
        *,
        http_request: Request,
        original_headers: dict[str, str],
        original_request_body: dict[str, Any],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        """创建视频任务"""

    @abstractmethod
    async def handle_get_task(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        """获取视频任务状态"""

    @abstractmethod
    async def handle_list_tasks(
        self,
        *,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        """列出任务"""

    @abstractmethod
    async def handle_cancel_task(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        """取消任务"""

    @abstractmethod
    async def handle_download_content(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> Response | StreamingResponse:
        """下载视频内容"""

    def _build_error_response(self, response: "httpx.Response") -> JSONResponse:
        """
        构建脱敏后的错误响应

        子类可重写 _format_error_payload 自定义错误格式。
        """
        content_type = response.headers.get("content-type", "")
        if "application/json" in content_type:
            try:
                error_data = response.json()
                if isinstance(error_data, dict) and "error" in error_data:
                    payload = self._format_error_payload(error_data["error"], response.status_code)
                    return JSONResponse(
                        status_code=response.status_code,
                        content={"error": payload},
                    )
            except ValueError, KeyError, TypeError:
                pass
        message = sanitize_error_message(response.text or "Upstream error")
        fallback_payload = self._format_error_payload({"message": message}, response.status_code)
        return JSONResponse(
            status_code=response.status_code,
            content={"error": fallback_payload},
        )

    def _format_error_payload(self, error: dict[str, Any], status_code: int) -> dict[str, Any]:
        """
        格式化错误负载，子类可重写以匹配特定 API 格式

        默认返回 OpenAI 风格格式。
        """
        return {
            "type": error.get("type", "upstream_error"),
            "message": sanitize_error_message(error.get("message", "Request failed")),
        }

    def _get_task(self, task_id: str) -> VideoTask:
        task = (
            self.db.query(VideoTask)
            .filter(VideoTask.id == task_id, VideoTask.user_id == self.user.id)
            .first()
        )
        if not task:
            raise HTTPException(status_code=404, detail="Video task not found")
        return task

    def _get_endpoint_and_key(self, task: VideoTask) -> tuple[ProviderEndpoint, ProviderAPIKey]:
        endpoint = (
            self.db.query(ProviderEndpoint).filter(ProviderEndpoint.id == task.endpoint_id).first()
        )
        key = self.db.query(ProviderAPIKey).filter(ProviderAPIKey.id == task.key_id).first()
        if not endpoint or not key:
            raise HTTPException(status_code=500, detail="Provider endpoint or key not found")
        return endpoint, key

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
            created_at=task.created_at,
            completed_at=task.completed_at,
            error_code=task.error_code,
            error_message=task.error_message,
            extra={"model": task.model},
        )


__all__ = ["VideoHandlerBase", "sanitize_error_message"]
