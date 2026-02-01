"""
Video Handler 基类

定义视频生成相关操作的统一接口。
"""

from __future__ import annotations

import re
from abc import ABC, abstractmethod
from collections.abc import Awaitable, Callable
from typing import TYPE_CHECKING, Any

from fastapi import HTTPException, Request
from fastapi.responses import JSONResponse, Response, StreamingResponse
from sqlalchemy.orm import Session

from src.config.settings import config
from src.core.api_format.conversion.internal_video import InternalVideoTask, VideoStatus
from src.core.exceptions import ProviderNotAvailableException
from src.core.logger import logger
from src.models.database import ApiKey, ProviderAPIKey, ProviderEndpoint, User, VideoTask
from src.services.billing.rule_service import BillingRuleLookupResult
from src.services.cache.aware_scheduler import ProviderCandidate

if TYPE_CHECKING:
    import httpx

    from src.services.task.orchestrator import SubmitOutcome

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


def normalize_gemini_operation_id(operation_id: str) -> str:
    """
    规范化 Gemini operation ID，确保以 "operations/" 开头

    Gemini API 返回的任务 ID 格式可能是 "operations/xxx" 或 "xxx"，
    此函数统一规范化为 "operations/xxx" 格式。

    Args:
        operation_id: 原始 operation ID

    Returns:
        规范化后的 operation ID
    """
    if not operation_id.startswith("operations/"):
        return f"operations/{operation_id}"
    return operation_id


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

    async def handle_remix_task(
        self,
        *,
        task_id: str,
        http_request: Request,
        original_headers: dict[str, str],
        original_request_body: dict[str, Any],
        query_params: dict[str, str] | None = None,
        path_params: dict[str, Any] | None = None,
    ) -> JSONResponse:
        """Remix 任务（基于已完成视频创建新视频）- 可选实现"""
        raise HTTPException(status_code=501, detail="Remix not supported for this provider")

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
            except (ValueError, KeyError, TypeError):
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

    def _build_billing_rule_snapshot(
        self, rule_lookup: BillingRuleLookupResult | None
    ) -> dict[str, Any]:
        """
        构建 billing_rule 快照，用于冻结到视频任务的 request_metadata 中。

        快照确保异步任务完成时使用创建时刻的计费规则，避免规则变更导致成本计算不一致。
        """
        if not rule_lookup:
            return {"status": "no_rule"}

        rule = rule_lookup.rule
        return {
            "status": "ok",
            "scope": rule_lookup.scope,
            "effective_task_type": rule_lookup.effective_task_type,
            "rule_id": rule.id,
            "rule_name": rule.name,
            "expression": rule.expression,
            "variables": rule.variables,
            "dimension_mappings": rule.dimension_mappings,
        }

    async def _submit_with_failover(
        self,
        *,
        api_format: str,
        model_name: str,
        task_type: str,
        submit_func: Callable[[ProviderCandidate], Awaitable["httpx.Response"]],
        extract_external_task_id: Callable[[dict[str, Any]], str | None],
        supported_auth_types: set[str] | None,
        allow_format_conversion: bool = False,
        capability_requirements: dict[str, bool] | None = None,
        max_candidates: int = 10,
    ) -> "SubmitOutcome | JSONResponse":
        """
        提交阶段故障转移（只负责拿到 external_task_id）。

        返回：
        - 成功：SubmitOutcome
        - 上游客户端错误：直接返回脱敏后的 JSONResponse（保留 API 格式差异）

        失败时：
        - 无可用候选 / 全部失败：抛 HTTPException(503)
        """
        # 延迟导入，避免 handler 基类层引入过多依赖导致循环
        from src.services.task.orchestrator import (
            AllCandidatesFailedError,
            AsyncTaskOrchestrator,
            SubmitOutcome,
            UpstreamClientRequestError,
        )

        orchestrator = AsyncTaskOrchestrator(self.db)
        try:
            return await orchestrator.submit_with_failover(
                api_format=api_format,
                model_name=model_name,
                affinity_key=str(self.api_key.id),
                user_api_key=self.api_key,
                request_id=self.request_id,
                task_type=task_type,
                submit_func=submit_func,
                extract_external_task_id=extract_external_task_id,
                supported_auth_types=supported_auth_types,
                allow_format_conversion=allow_format_conversion,
                capability_requirements=capability_requirements,
                max_candidates=max_candidates,
            )
        except UpstreamClientRequestError as exc:
            return self._build_error_response(exc.response)
        except AllCandidatesFailedError as exc:
            detail = "No available provider for video generation"
            if config.billing_require_rule:
                detail = "No available provider with billing rule for video generation"
            # 记录候选信息到日志
            logger.warning(
                "[VideoHandler] All candidates failed: reason=%s, candidate_keys=%s",
                exc.reason,
                exc.candidate_keys,
            )
            # 创建带有 candidate_keys 的 HTTPException
            http_exc = HTTPException(status_code=503, detail=detail)
            http_exc.candidate_keys = exc.candidate_keys  # type: ignore[attr-defined]
            raise http_exc
        except ProviderNotAvailableException:
            detail = "No available provider for video generation"
            if config.billing_require_rule:
                detail = "No available provider with billing rule for video generation"
            raise HTTPException(status_code=503, detail=detail)


__all__ = ["VideoHandlerBase", "normalize_gemini_operation_id", "sanitize_error_message"]
