from __future__ import annotations

import json
import uuid
from datetime import datetime, timezone
from typing import Any

from sqlalchemy.orm import Session

from src.core.api_format.signature import normalize_signature_key
from src.core.logger import logger
from src.models.database import ApiKey, Provider, Usage, User
from src.services.billing.token_normalization import normalize_input_tokens_for_billing
from src.services.system.config import SystemConfigService
from src.services.usage._types import UsageCostInfo, UsageRecordParams
from src.services.usage.error_classifier import classify_error


class UsageRecordingMixin:
    """记录用量相关方法"""

    # Metadata pruning configuration (ordered by priority - drop first to last)
    _METADATA_PRUNE_KEYS: tuple[str, ...] = (
        "raw_response_ref",
        "poll_raw_response",
        "trace",
        "debug",
        "dimensions",
        "provider_response_headers",
        "client_response_headers",
    )

    # Keys to preserve even under aggressive pruning
    _METADATA_KEEP_KEYS: frozenset[str] = frozenset(
        {
            "billing_snapshot",
            "billing_updated_at",
            "perf",
            "_metadata_truncated",
        }
    )

    @staticmethod
    def _build_usage_params(
        *,
        db: Session,
        user: User | None,
        api_key: ApiKey | None,
        provider: str,
        model: str,
        input_tokens: int,
        output_tokens: int,
        cache_creation_input_tokens: int,
        cache_read_input_tokens: int,
        request_type: str,
        api_format: str | None,
        endpoint_api_format: str | None,
        has_format_conversion: bool,
        is_stream: bool,
        response_time_ms: int | None,
        first_byte_time_ms: int | None,
        status_code: int,
        error_message: str | None,
        metadata: dict[str, Any] | None,
        request_headers: dict[str, Any] | None,
        request_body: Any | None,
        provider_request_headers: dict[str, Any] | None,
        response_headers: dict[str, Any] | None,
        client_response_headers: dict[str, Any] | None,
        response_body: Any | None,
        request_id: str,
        provider_id: str | None,
        provider_endpoint_id: str | None,
        provider_api_key_id: str | None,
        status: str,
        target_model: str | None,
        cost: UsageCostInfo,
    ) -> dict[str, Any]:
        """构建 Usage 记录的参数字典（内部方法，避免代码重复）"""

        # 展开成本信息
        input_cost = cost.input_cost
        output_cost = cost.output_cost
        cache_creation_cost = cost.cache_creation_cost
        cache_read_cost = cost.cache_read_cost
        cache_cost = cost.cache_cost
        request_cost = cost.request_cost
        total_cost = cost.total_cost
        input_price = cost.input_price
        output_price = cost.output_price
        cache_creation_price = cost.cache_creation_price
        cache_read_price = cost.cache_read_price
        request_price = cost.request_price
        actual_rate_multiplier = cost.actual_rate_multiplier
        is_free_tier = cost.is_free_tier

        # 根据配置决定是否记录请求详情
        should_log_headers = SystemConfigService.should_log_headers(db)
        should_log_body = SystemConfigService.should_log_body(db)

        # 处理请求头（可能需要脱敏）
        processed_request_headers = None
        if should_log_headers and request_headers:
            processed_request_headers = SystemConfigService.mask_sensitive_headers(
                db, request_headers
            )

        # 处理提供商请求头（可能需要脱敏）
        processed_provider_request_headers = None
        if should_log_headers and provider_request_headers:
            processed_provider_request_headers = SystemConfigService.mask_sensitive_headers(
                db, provider_request_headers
            )

        # 处理请求体和响应体（可能需要截断）
        processed_request_body = None
        processed_response_body = None
        if should_log_body:
            if request_body:
                processed_request_body = SystemConfigService.truncate_body(
                    db, request_body, is_request=True
                )
            if response_body:
                processed_response_body = SystemConfigService.truncate_body(
                    db, response_body, is_request=False
                )

        # 处理响应头
        processed_response_headers = None
        if should_log_headers and response_headers:
            processed_response_headers = SystemConfigService.mask_sensitive_headers(
                db, response_headers
            )

        # 处理返回给客户端的响应头
        processed_client_response_headers = None
        if should_log_headers and client_response_headers:
            processed_client_response_headers = SystemConfigService.mask_sensitive_headers(
                db, client_response_headers
            )

        # 计算真实成本（表面成本 * 倍率），免费套餐实际费用为 0
        if is_free_tier:
            actual_input_cost = 0.0
            actual_output_cost = 0.0
            actual_cache_creation_cost = 0.0
            actual_cache_read_cost = 0.0
            actual_request_cost = 0.0
            actual_total_cost = 0.0
        else:
            actual_input_cost = input_cost * actual_rate_multiplier
            actual_output_cost = output_cost * actual_rate_multiplier
            actual_cache_creation_cost = cache_creation_cost * actual_rate_multiplier
            actual_cache_read_cost = cache_read_cost * actual_rate_multiplier
            actual_request_cost = request_cost * actual_rate_multiplier
            actual_total_cost = total_cost * actual_rate_multiplier

        error_category = None
        if status_code >= 400 or error_message or status in {"failed", "cancelled"}:
            error_category = classify_error(status_code, error_message, status).value

        return {
            "user_id": user.id if user else None,
            "api_key_id": api_key.id if api_key else None,
            "request_id": request_id,
            "provider_name": provider,
            "model": model,
            "target_model": target_model,
            "provider_id": provider_id,
            "provider_endpoint_id": provider_endpoint_id,
            "provider_api_key_id": provider_api_key_id,
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens,
            "cache_creation_input_tokens": cache_creation_input_tokens,
            "cache_read_input_tokens": cache_read_input_tokens,
            "input_cost_usd": input_cost,
            "output_cost_usd": output_cost,
            "cache_cost_usd": cache_cost,
            "cache_creation_cost_usd": cache_creation_cost,
            "cache_read_cost_usd": cache_read_cost,
            "request_cost_usd": request_cost,
            "total_cost_usd": total_cost,
            "actual_input_cost_usd": actual_input_cost,
            "actual_output_cost_usd": actual_output_cost,
            "actual_cache_creation_cost_usd": actual_cache_creation_cost,
            "actual_cache_read_cost_usd": actual_cache_read_cost,
            "actual_request_cost_usd": actual_request_cost,
            "actual_total_cost_usd": actual_total_cost,
            "rate_multiplier": actual_rate_multiplier,
            "input_price_per_1m": input_price,
            "output_price_per_1m": output_price,
            "cache_creation_price_per_1m": cache_creation_price,
            "cache_read_price_per_1m": cache_read_price,
            "price_per_request": request_price,
            "request_type": request_type,
            "api_format": api_format,
            "endpoint_api_format": endpoint_api_format,
            "has_format_conversion": has_format_conversion,
            "is_stream": is_stream,
            "status_code": status_code,
            "error_message": error_message,
            "error_category": error_category,
            "response_time_ms": response_time_ms,
            "first_byte_time_ms": first_byte_time_ms,
            "status": status,
            "request_metadata": metadata,
            "request_headers": processed_request_headers,
            "request_body": processed_request_body,
            "provider_request_headers": processed_provider_request_headers,
            "response_headers": processed_response_headers,
            "client_response_headers": processed_client_response_headers,
            "response_body": processed_response_body,
        }

    @staticmethod
    def _update_existing_usage(
        existing_usage: Usage,
        usage_params: dict[str, Any],
        target_model: str | None,
    ) -> None:
        """更新已存在的 Usage 记录（内部方法）"""
        # 更新关键字段
        existing_usage.provider_name = usage_params["provider_name"]
        existing_usage.model = usage_params["model"]
        existing_usage.request_type = usage_params["request_type"]
        existing_usage.api_format = usage_params["api_format"]
        existing_usage.endpoint_api_format = usage_params["endpoint_api_format"]
        existing_usage.has_format_conversion = usage_params["has_format_conversion"]
        existing_usage.is_stream = usage_params["is_stream"]
        existing_usage.status = usage_params["status"]
        existing_usage.status_code = usage_params["status_code"]
        existing_usage.error_message = usage_params["error_message"]
        existing_usage.error_category = usage_params.get("error_category")
        existing_usage.response_time_ms = usage_params["response_time_ms"]
        existing_usage.first_byte_time_ms = usage_params["first_byte_time_ms"]

        # 更新请求头和请求体（如果有新值）
        if usage_params["request_headers"] is not None:
            existing_usage.request_headers = usage_params["request_headers"]
        if usage_params["request_body"] is not None:
            existing_usage.request_body = usage_params["request_body"]
        if usage_params["provider_request_headers"] is not None:
            existing_usage.provider_request_headers = usage_params["provider_request_headers"]
        existing_usage.response_body = usage_params["response_body"]
        existing_usage.response_headers = usage_params["response_headers"]
        existing_usage.client_response_headers = usage_params["client_response_headers"]

        # 更新 token 和费用信息
        existing_usage.input_tokens = usage_params["input_tokens"]
        existing_usage.output_tokens = usage_params["output_tokens"]
        existing_usage.total_tokens = usage_params["total_tokens"]
        existing_usage.cache_creation_input_tokens = usage_params["cache_creation_input_tokens"]
        existing_usage.cache_read_input_tokens = usage_params["cache_read_input_tokens"]
        existing_usage.input_cost_usd = usage_params["input_cost_usd"]
        existing_usage.output_cost_usd = usage_params["output_cost_usd"]
        existing_usage.cache_cost_usd = usage_params["cache_cost_usd"]
        existing_usage.cache_creation_cost_usd = usage_params["cache_creation_cost_usd"]
        existing_usage.cache_read_cost_usd = usage_params["cache_read_cost_usd"]
        existing_usage.request_cost_usd = usage_params["request_cost_usd"]
        existing_usage.total_cost_usd = usage_params["total_cost_usd"]
        existing_usage.actual_input_cost_usd = usage_params["actual_input_cost_usd"]
        existing_usage.actual_output_cost_usd = usage_params["actual_output_cost_usd"]
        existing_usage.actual_cache_creation_cost_usd = usage_params[
            "actual_cache_creation_cost_usd"
        ]
        existing_usage.actual_cache_read_cost_usd = usage_params["actual_cache_read_cost_usd"]
        existing_usage.actual_request_cost_usd = usage_params["actual_request_cost_usd"]
        existing_usage.actual_total_cost_usd = usage_params["actual_total_cost_usd"]
        existing_usage.rate_multiplier = usage_params["rate_multiplier"]

        # 更新 Provider 侧追踪信息
        existing_usage.provider_id = usage_params["provider_id"]
        existing_usage.provider_endpoint_id = usage_params["provider_endpoint_id"]
        existing_usage.provider_api_key_id = usage_params["provider_api_key_id"]

        # 更新元数据（如 billing_snapshot/dimensions 等）
        if usage_params.get("request_metadata") is not None:
            existing_usage.request_metadata = usage_params["request_metadata"]

        # 更新模型映射信息
        if target_model is not None:
            existing_usage.target_model = target_model

    @classmethod
    def _sanitize_request_metadata(cls, metadata: dict[str, Any]) -> dict[str, Any]:
        """
        Best-effort metadata pruning to reduce DB/CPU/memory pressure.

        This is called right before persisting Usage rows (or updating request_metadata).
        Pruning order is defined by `_METADATA_PRUNE_KEYS` (first key is dropped first).
        """
        if not isinstance(metadata, dict) or not metadata:
            return {}

        from src.config.settings import config

        # Enforce global metadata size limit (best-effort)
        max_bytes = int(getattr(config, "usage_metadata_max_bytes", 0) or 0)
        if max_bytes <= 0:
            return metadata

        def _size(d: dict[str, Any]) -> int:
            try:
                return len(json.dumps(d, ensure_ascii=False, default=str))
            except Exception:
                return len(str(d))

        if _size(metadata) <= max_bytes:
            return metadata

        # Progressive pruning (configurable order)
        metadata["_metadata_truncated"] = True

        for k in cls._METADATA_PRUNE_KEYS:
            if k in metadata:
                metadata.pop(k, None)
                if _size(metadata) <= max_bytes:
                    return metadata

        # Fallback: keep only billing-related metadata
        reduced = {k: metadata.get(k) for k in cls._METADATA_KEEP_KEYS if k in metadata}
        return reduced

    @classmethod
    async def _prepare_usage_record(
        cls,
        params: UsageRecordParams,
    ) -> tuple[dict[str, Any], float]:
        """准备用量记录的共享逻辑

        此方法提取了 record_usage 和 record_usage_async 的公共处理逻辑：
        - 获取费率倍数
        - 计算成本
        - 构建 Usage 参数

        Args:
            params: 用量记录参数数据类

        Returns:
            (usage_params 字典, total_cost 总成本)
        """
        # 计费口径以 Provider 为准（优先 endpoint_api_format）
        billing_api_format: str | None = None
        if params.endpoint_api_format:
            try:
                billing_api_format = normalize_signature_key(str(params.endpoint_api_format))
            except Exception:
                billing_api_format = None
        if billing_api_format is None and params.api_format:
            try:
                billing_api_format = normalize_signature_key(str(params.api_format))
            except Exception:
                billing_api_format = None

        input_tokens_for_billing = normalize_input_tokens_for_billing(
            billing_api_format,
            params.input_tokens,
            params.cache_read_input_tokens,
        )

        # 获取费率倍数和是否免费套餐（传递 api_format 支持按格式配置的倍率）
        actual_rate_multiplier, is_free_tier = await cls._get_rate_multiplier_and_free_tier(
            params.db, params.provider_api_key_id, params.provider_id, billing_api_format
        )

        metadata = dict(params.metadata or {})
        is_failed_request = params.status_code >= 400 or params.error_message is not None

        # Helper: compute billing task_type (billing domain)
        billing_task_type = (params.request_type or "").lower()
        if billing_task_type not in {"chat", "cli", "video", "image", "audio"}:
            billing_task_type = "chat"

        # 使用新计费系统计算费用
        from src.services.billing.service import BillingService

        request_count = 0 if is_failed_request else 1
        dims: dict[str, Any] = {
            "input_tokens": input_tokens_for_billing,
            "output_tokens": params.output_tokens,
            "cache_creation_input_tokens": params.cache_creation_input_tokens,
            "cache_read_input_tokens": params.cache_read_input_tokens,
            "request_count": request_count,
        }
        if params.cache_ttl_minutes is not None:
            dims["cache_ttl_minutes"] = params.cache_ttl_minutes
        # If tiered pricing is disabled, force first tier by using tier-key=0.
        if not params.use_tiered_pricing:
            dims["total_input_context"] = 0

        billing = BillingService(params.db)
        result = billing.calculate(
            task_type=billing_task_type,
            model=params.model,
            provider_id=params.provider_id or "",
            dimensions=dims,
            strict_mode=None,
        )
        snap = result.snapshot

        breakdown = snap.cost_breakdown or {}
        input_cost = float(breakdown.get("input_cost", 0.0))
        output_cost = float(breakdown.get("output_cost", 0.0))
        cache_creation_cost = float(breakdown.get("cache_creation_cost", 0.0))
        cache_read_cost = float(breakdown.get("cache_read_cost", 0.0))
        request_cost = float(breakdown.get("request_cost", 0.0))
        cache_cost = cache_creation_cost + cache_read_cost
        total_cost = float(snap.total_cost or 0.0)

        rv = snap.resolved_variables or {}

        def _as_float(v: Any, d: float | None) -> float | None:
            try:
                if v is None:
                    return d
                return float(v)
            except Exception:
                return d

        input_price = _as_float(rv.get("input_price_per_1m"), 0.0) or 0.0
        output_price = _as_float(rv.get("output_price_per_1m"), 0.0) or 0.0
        cache_creation_price = _as_float(rv.get("cache_creation_price_per_1m"), None)
        cache_read_price = _as_float(rv.get("cache_read_price_per_1m"), None)
        request_price = _as_float(rv.get("price_per_request"), None)

        # Audit snapshot (pruned later by _sanitize_request_metadata)
        metadata["billing_snapshot"] = snap.to_dict()

        # Best-effort prune metadata to reduce DB/memory pressure.
        metadata = cls._sanitize_request_metadata(metadata)

        # 构建 Usage 参数
        usage_params = cls._build_usage_params(
            db=params.db,
            user=params.user,
            api_key=params.api_key,
            provider=params.provider,
            model=params.model,
            input_tokens=input_tokens_for_billing,
            output_tokens=params.output_tokens,
            cache_creation_input_tokens=params.cache_creation_input_tokens,
            cache_read_input_tokens=params.cache_read_input_tokens,
            request_type=params.request_type,
            api_format=params.api_format,
            endpoint_api_format=params.endpoint_api_format,
            has_format_conversion=params.has_format_conversion,
            is_stream=params.is_stream,
            response_time_ms=params.response_time_ms,
            first_byte_time_ms=params.first_byte_time_ms,
            status_code=params.status_code,
            error_message=params.error_message,
            metadata=metadata,
            request_headers=params.request_headers,
            request_body=params.request_body,
            provider_request_headers=params.provider_request_headers,
            response_headers=params.response_headers,
            client_response_headers=params.client_response_headers,
            response_body=params.response_body,
            request_id=params.request_id,
            provider_id=params.provider_id,
            provider_endpoint_id=params.provider_endpoint_id,
            provider_api_key_id=params.provider_api_key_id,
            status=params.status,
            target_model=params.target_model,
            cost=UsageCostInfo(
                input_cost=input_cost,
                output_cost=output_cost,
                cache_creation_cost=cache_creation_cost,
                cache_read_cost=cache_read_cost,
                cache_cost=cache_cost,
                request_cost=request_cost,
                total_cost=total_cost,
                input_price=input_price,
                output_price=output_price,
                cache_creation_price=cache_creation_price,
                cache_read_price=cache_read_price,
                request_price=request_price,
                actual_rate_multiplier=actual_rate_multiplier,
                is_free_tier=is_free_tier,
            ),
        )

        return usage_params, total_cost

    @classmethod
    async def _prepare_usage_records_batch(
        cls,
        params_list: list[UsageRecordParams],
    ) -> list[tuple[dict[str, Any], float, Exception | None]]:
        """批量并行准备用量记录（性能优化）

        并行调用 _prepare_usage_record，提高批量处理效率。

        Args:
            params_list: 用量记录参数列表

        Returns:
            列表，每项为 (usage_params, total_cost, exception)
            如果处理成功，exception 为 None
        """
        import asyncio

        async def prepare_single(
            params: UsageRecordParams,
        ) -> tuple[dict[str, Any], float, Exception | None]:
            try:
                usage_params, total_cost = await cls._prepare_usage_record(params)
                return (usage_params, total_cost, None)
            except Exception as e:
                return ({}, 0.0, e)

        if not params_list:
            return []

        # 避免一次性创建过多 task（并且 _prepare_usage_record 内部也可能包含并行调用）
        # 这里采用分批 gather 来限制并发量。
        chunk_size = 50
        results: list[tuple[dict[str, Any], float, Exception | None]] = []
        for i in range(0, len(params_list), chunk_size):
            chunk = params_list[i : i + chunk_size]
            chunk_results = await asyncio.gather(*(prepare_single(p) for p in chunk))
            results.extend(chunk_results)
        return results

    @classmethod
    async def record_usage_async(
        cls,
        db: Session,
        user: User | None,
        api_key: ApiKey | None,
        provider: str,
        model: str,
        input_tokens: int,
        output_tokens: int,
        cache_creation_input_tokens: int = 0,
        cache_read_input_tokens: int = 0,
        request_type: str = "chat",
        api_format: str | None = None,
        endpoint_api_format: str | None = None,
        has_format_conversion: bool = False,
        is_stream: bool = False,
        response_time_ms: int | None = None,
        first_byte_time_ms: int | None = None,
        status_code: int = 200,
        error_message: str | None = None,
        metadata: dict[str, Any] | None = None,
        request_headers: dict[str, Any] | None = None,
        request_body: Any | None = None,
        provider_request_headers: dict[str, Any] | None = None,
        response_headers: dict[str, Any] | None = None,
        client_response_headers: dict[str, Any] | None = None,
        response_body: Any | None = None,
        request_id: str | None = None,
        provider_id: str | None = None,
        provider_endpoint_id: str | None = None,
        provider_api_key_id: str | None = None,
        status: str = "completed",
        cache_ttl_minutes: int | None = None,
        use_tiered_pricing: bool = True,
        target_model: str | None = None,
    ) -> Usage:
        """异步记录使用量（简化版，仅插入新记录）

        此方法用于快速记录使用量，不更新用户/API Key 统计，不支持更新已存在的记录。
        适用于不需要更新统计信息的场景。

        如需完整功能（更新用户统计、支持更新已存在记录），请使用 record_usage()。
        """
        # 生成 request_id
        if request_id is None:
            request_id = str(uuid.uuid4())[:8]

        # 使用共享逻辑准备记录参数
        params = UsageRecordParams(
            db=db,
            user=user,
            api_key=api_key,
            provider=provider,
            model=model,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            cache_creation_input_tokens=cache_creation_input_tokens,
            cache_read_input_tokens=cache_read_input_tokens,
            request_type=request_type,
            api_format=api_format,
            endpoint_api_format=endpoint_api_format,
            has_format_conversion=has_format_conversion,
            is_stream=is_stream,
            response_time_ms=response_time_ms,
            first_byte_time_ms=first_byte_time_ms,
            status_code=status_code,
            error_message=error_message,
            metadata=metadata,
            request_headers=request_headers,
            request_body=request_body,
            provider_request_headers=provider_request_headers,
            response_headers=response_headers,
            client_response_headers=client_response_headers,
            response_body=response_body,
            request_id=request_id,
            provider_id=provider_id,
            provider_endpoint_id=provider_endpoint_id,
            provider_api_key_id=provider_api_key_id,
            status=status,
            cache_ttl_minutes=cache_ttl_minutes,
            use_tiered_pricing=use_tiered_pricing,
            target_model=target_model,
        )
        usage_params, _ = await cls._prepare_usage_record(params)

        # 创建 Usage 记录
        usage = Usage(**usage_params)
        db.add(usage)

        # 更新 GlobalModel 使用计数（原子操作）
        from sqlalchemy import update

        from src.models.database import GlobalModel

        db.execute(
            update(GlobalModel)
            .where(GlobalModel.name == model)
            .values(usage_count=GlobalModel.usage_count + 1)
        )

        # 更新 Provider 月度使用量（原子操作）
        if provider_id:
            actual_total_cost = usage_params["actual_total_cost_usd"]
            db.execute(
                update(Provider)
                .where(Provider.id == provider_id)
                .values(monthly_used_usd=Provider.monthly_used_usd + actual_total_cost)
            )

        # 结算标记：record_usage_async 写入的 Usage 通常为终态记录
        if status not in ("pending", "streaming"):
            usage.billing_status = "settled"
            usage.finalized_at = datetime.now(timezone.utc)

        db.commit()  # 立即提交事务，释放数据库锁
        return usage

    @classmethod
    async def record_usage(
        cls,
        db: Session,
        user: User | None,
        api_key: ApiKey | None,
        provider: str,
        model: str,
        input_tokens: int,
        output_tokens: int,
        cache_creation_input_tokens: int = 0,
        cache_read_input_tokens: int = 0,
        request_type: str = "chat",
        api_format: str | None = None,
        endpoint_api_format: str | None = None,
        has_format_conversion: bool = False,
        is_stream: bool = False,
        response_time_ms: int | None = None,
        first_byte_time_ms: int | None = None,
        status_code: int = 200,
        error_message: str | None = None,
        metadata: dict[str, Any] | None = None,
        request_headers: dict[str, Any] | None = None,
        request_body: Any | None = None,
        provider_request_headers: dict[str, Any] | None = None,
        response_headers: dict[str, Any] | None = None,
        client_response_headers: dict[str, Any] | None = None,
        response_body: Any | None = None,
        request_id: str | None = None,
        provider_id: str | None = None,
        provider_endpoint_id: str | None = None,
        provider_api_key_id: str | None = None,
        status: str = "completed",
        cache_ttl_minutes: int | None = None,
        use_tiered_pricing: bool = True,
        target_model: str | None = None,
    ) -> Usage:
        """记录使用量（完整版，支持更新已存在记录和用户统计）

        此方法支持：
        - 检查是否已存在相同 request_id 的记录（更新 vs 插入）
        - 更新用户/API Key 使用统计
        - 阶梯计费

        如只需简单插入新记录，可使用 record_usage_async()。
        """
        # 生成 request_id
        if request_id is None:
            request_id = str(uuid.uuid4())[:8]

        # 使用共享逻辑准备记录参数
        params = UsageRecordParams(
            db=db,
            user=user,
            api_key=api_key,
            provider=provider,
            model=model,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            cache_creation_input_tokens=cache_creation_input_tokens,
            cache_read_input_tokens=cache_read_input_tokens,
            request_type=request_type,
            api_format=api_format,
            endpoint_api_format=endpoint_api_format,
            has_format_conversion=has_format_conversion,
            is_stream=is_stream,
            response_time_ms=response_time_ms,
            first_byte_time_ms=first_byte_time_ms,
            status_code=status_code,
            error_message=error_message,
            metadata=metadata,
            request_headers=request_headers,
            request_body=request_body,
            provider_request_headers=provider_request_headers,
            response_headers=response_headers,
            client_response_headers=client_response_headers,
            response_body=response_body,
            request_id=request_id,
            provider_id=provider_id,
            provider_endpoint_id=provider_endpoint_id,
            provider_api_key_id=provider_api_key_id,
            status=status,
            cache_ttl_minutes=cache_ttl_minutes,
            use_tiered_pricing=use_tiered_pricing,
            target_model=target_model,
        )
        usage_params, total_cost = await cls._prepare_usage_record(params)

        # 检查是否已存在相同 request_id 的记录
        existing_usage = db.query(Usage).filter(Usage.request_id == request_id).first()
        if existing_usage:
            logger.debug(
                f"request_id {request_id} 已存在，更新现有记录 "
                f"(status: {existing_usage.status} -> {status})"
            )
            cls._update_existing_usage(existing_usage, usage_params, target_model)
            usage = existing_usage
        else:
            usage = Usage(**usage_params)
            db.add(usage)

        # 确保 user 和 api_key 在会话中
        if user and not db.object_session(user):
            user = db.merge(user)
        if api_key and not db.object_session(api_key):
            api_key = db.merge(api_key)

        # 使用原子更新避免并发竞态条件
        from sqlalchemy import func as sql_func
        from sqlalchemy import update

        from src.models.database import ApiKey as ApiKeyModel
        from src.models.database import GlobalModel
        from src.models.database import User as UserModel

        # 更新用户使用量（独立 Key 不计入创建者的使用记录）
        if user and not (api_key and api_key.is_standalone):
            db.execute(
                update(UserModel)
                .where(UserModel.id == user.id)
                .values(
                    used_usd=UserModel.used_usd + total_cost,
                    total_usd=UserModel.total_usd + total_cost,
                    updated_at=sql_func.now(),
                )
            )

        # 更新 API 密钥使用量
        if api_key:
            if api_key.is_standalone:
                db.execute(
                    update(ApiKeyModel)
                    .where(ApiKeyModel.id == api_key.id)
                    .values(
                        total_requests=ApiKeyModel.total_requests + 1,
                        total_cost_usd=ApiKeyModel.total_cost_usd + total_cost,
                        balance_used_usd=ApiKeyModel.balance_used_usd + total_cost,
                        last_used_at=sql_func.now(),
                        updated_at=sql_func.now(),
                    )
                )
            else:
                db.execute(
                    update(ApiKeyModel)
                    .where(ApiKeyModel.id == api_key.id)
                    .values(
                        total_requests=ApiKeyModel.total_requests + 1,
                        total_cost_usd=ApiKeyModel.total_cost_usd + total_cost,
                        last_used_at=sql_func.now(),
                        updated_at=sql_func.now(),
                    )
                )

        # 更新 GlobalModel 使用计数
        db.execute(
            update(GlobalModel)
            .where(GlobalModel.name == model)
            .values(usage_count=GlobalModel.usage_count + 1)
        )

        # 更新 Provider 月度使用量
        if provider_id:
            actual_total_cost = usage_params["actual_total_cost_usd"]
            db.execute(
                update(Provider)
                .where(Provider.id == provider_id)
                .values(monthly_used_usd=Provider.monthly_used_usd + actual_total_cost)
            )

        # 结算标记：终态请求写入 settled + finalized_at
        if status not in ("pending", "streaming"):
            usage.billing_status = "settled"
            usage.finalized_at = datetime.now(timezone.utc)

        # 提交事务
        try:
            db.commit()
        except Exception as e:
            logger.error("提交使用记录时出错: {}", e)
            db.rollback()
            raise

        return usage

    @classmethod
    async def record_usage_with_custom_cost(
        cls,
        *,
        db: Session,
        user: User | None,
        api_key: ApiKey | None,
        provider: str,
        model: str,
        request_type: str,
        total_cost_usd: float,
        request_cost_usd: float | None = None,
        input_tokens: int = 0,
        output_tokens: int = 0,
        cache_creation_input_tokens: int = 0,
        cache_read_input_tokens: int = 0,
        api_format: str | None = None,
        endpoint_api_format: str | None = None,
        has_format_conversion: bool = False,
        is_stream: bool = False,
        response_time_ms: int | None = None,
        first_byte_time_ms: int | None = None,
        status_code: int = 200,
        error_message: str | None = None,
        metadata: dict[str, Any] | None = None,
        request_headers: dict[str, Any] | None = None,
        request_body: Any | None = None,
        provider_request_headers: dict[str, Any] | None = None,
        response_headers: dict[str, Any] | None = None,
        client_response_headers: dict[str, Any] | None = None,
        response_body: Any | None = None,
        request_id: str | None = None,
        provider_id: str | None = None,
        provider_endpoint_id: str | None = None,
        provider_api_key_id: str | None = None,
        status: str = "completed",
        target_model: str | None = None,
    ) -> Usage:
        """
        记录"已计算好的"成本（用于 Video/Image/Audio 等异步任务的 FormulaEngine 计费结果）。

        说明：
        - 仍然会应用 ProviderAPIKey.rate_multipliers 计算 actual_* 成本
        - 会更新 User/APIKey/GlobalModel/Provider 的统计（与 record_usage 行为一致）
        - 若 request_id 已存在则更新记录（避免重复写入）
        """
        # 生成 request_id
        if request_id is None:
            request_id = str(uuid.uuid4())[:8]

        # 获取费率倍数与免费套餐
        actual_rate_multiplier, is_free_tier = await cls._get_rate_multiplier_and_free_tier(
            db, provider_api_key_id, provider_id, api_format
        )

        # 成本拆分：非 token 计费默认计入 request_cost
        input_cost = 0.0
        output_cost = 0.0
        cache_creation_cost = 0.0
        cache_read_cost = 0.0
        cache_cost = 0.0
        request_cost = (
            float(request_cost_usd) if request_cost_usd is not None else float(total_cost_usd)
        )
        total_cost = float(total_cost_usd)

        usage_params = cls._build_usage_params(
            db=db,
            user=user,
            api_key=api_key,
            provider=provider,
            model=model,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            cache_creation_input_tokens=cache_creation_input_tokens,
            cache_read_input_tokens=cache_read_input_tokens,
            request_type=request_type,
            api_format=api_format,
            endpoint_api_format=endpoint_api_format,
            has_format_conversion=has_format_conversion,
            is_stream=is_stream,
            response_time_ms=response_time_ms,
            first_byte_time_ms=first_byte_time_ms,
            status_code=status_code,
            error_message=error_message,
            metadata=metadata,
            request_headers=request_headers,
            request_body=request_body,
            provider_request_headers=provider_request_headers,
            response_headers=response_headers,
            client_response_headers=client_response_headers,
            response_body=response_body,
            request_id=request_id,
            provider_id=provider_id,
            provider_endpoint_id=provider_endpoint_id,
            provider_api_key_id=provider_api_key_id,
            status=status,
            target_model=target_model,
            cost=UsageCostInfo(
                input_cost=input_cost,
                output_cost=output_cost,
                cache_creation_cost=cache_creation_cost,
                cache_read_cost=cache_read_cost,
                cache_cost=cache_cost,
                request_cost=request_cost,
                total_cost=total_cost,
                actual_rate_multiplier=actual_rate_multiplier,
                is_free_tier=is_free_tier,
            ),
        )

        # Upsert（并发幂等：优先用 billing_status 作为结算闸门）
        from sqlalchemy import update

        existing_usage = db.query(Usage).filter(Usage.request_id == request_id).first()
        if existing_usage:
            # 避免重复记账：若已结算/作废，直接返回（防止并发重复加计数）
            if getattr(existing_usage, "billing_status", None) in ("settled", "void"):
                logger.debug(
                    "record_usage_with_custom_cost: request_id={} already finalized (billing_status={}), skip",
                    request_id,
                    getattr(existing_usage, "billing_status", None),
                )
                return existing_usage

            # 并发闸门：只有 billing_status='pending' 的那一次调用可以继续
            now = datetime.now(timezone.utc)
            claim = db.execute(
                update(Usage)
                .where(
                    Usage.request_id == request_id,
                    Usage.billing_status == "pending",
                )
                .values(billing_status="settled", finalized_at=now)
            )
            if claim.rowcount != 1:
                # 已被其他 worker 抢先处理（或被 VOID）
                latest = db.query(Usage).filter(Usage.request_id == request_id).first()
                return latest or existing_usage

            # 同步 ORM 对象（避免后续代码读到旧值）
            existing_usage.billing_status = "settled"
            existing_usage.finalized_at = now

            cls._update_existing_usage(existing_usage, usage_params, target_model)
            usage = existing_usage
        else:
            usage = Usage(**usage_params)
            db.add(usage)

        # 确保 user 和 api_key 在会话中（与 record_usage 保持一致）
        if user and not db.object_session(user):
            user = db.merge(user)
        if api_key and not db.object_session(api_key):
            api_key = db.merge(api_key)

        # 原子更新统计
        from sqlalchemy import func as sql_func
        from sqlalchemy import update

        from src.models.database import ApiKey as ApiKeyModel
        from src.models.database import GlobalModel
        from src.models.database import User as UserModel

        # 更新用户使用量（独立 Key 不计入创建者）
        if user and not (api_key and api_key.is_standalone):
            db.execute(
                update(UserModel)
                .where(UserModel.id == user.id)
                .values(
                    used_usd=UserModel.used_usd + total_cost,
                    total_usd=UserModel.total_usd + total_cost,
                    updated_at=sql_func.now(),
                )
            )

        # 更新 API 密钥使用量
        if api_key:
            if api_key.is_standalone:
                db.execute(
                    update(ApiKeyModel)
                    .where(ApiKeyModel.id == api_key.id)
                    .values(
                        total_requests=ApiKeyModel.total_requests + 1,
                        total_cost_usd=ApiKeyModel.total_cost_usd + total_cost,
                        balance_used_usd=ApiKeyModel.balance_used_usd + total_cost,
                        last_used_at=sql_func.now(),
                        updated_at=sql_func.now(),
                    )
                )
            else:
                db.execute(
                    update(ApiKeyModel)
                    .where(ApiKeyModel.id == api_key.id)
                    .values(
                        total_requests=ApiKeyModel.total_requests + 1,
                        total_cost_usd=ApiKeyModel.total_cost_usd + total_cost,
                        last_used_at=sql_func.now(),
                        updated_at=sql_func.now(),
                    )
                )

        # 更新 GlobalModel 使用计数
        db.execute(
            update(GlobalModel)
            .where(GlobalModel.name == model)
            .values(usage_count=GlobalModel.usage_count + 1)
        )

        # 更新 Provider 月度使用量（使用 actual_total_cost）
        if provider_id:
            actual_total_cost = usage_params["actual_total_cost_usd"]
            db.execute(
                update(Provider)
                .where(Provider.id == provider_id)
                .values(monthly_used_usd=Provider.monthly_used_usd + actual_total_cost)
            )

        # 结算标记：record_usage_with_custom_cost 写入/更新的 Usage 通常为终态记录
        if status not in ("pending", "streaming"):
            usage.billing_status = "settled"
            usage.finalized_at = datetime.now(timezone.utc)

        try:
            db.commit()
        except Exception as e:
            # 并发场景可能触发唯一约束冲突：降级为读取已存在记录
            try:
                from sqlalchemy.exc import IntegrityError

                if isinstance(e, IntegrityError):
                    db.rollback()
                    existing = db.query(Usage).filter(Usage.request_id == request_id).first()
                    if existing:
                        return existing
            except Exception:
                pass

            logger.error("提交使用记录时出错: {}", e)
            db.rollback()
            raise

        return usage

    @classmethod
    async def record_usage_batch(
        cls,
        db: Session,
        records: list[dict[str, Any]],
    ) -> list[Usage]:
        """批量记录使用量（高性能版，单次提交多条记录）

        此方法针对高并发场景优化，特点：
        - 批量插入 Usage 记录，减少 commit 次数
        - 聚合更新用户/API Key 统计（按 user_id/api_key_id 分组）
        - 聚合更新 GlobalModel 和 Provider 统计
        - 支持更新已存在的 pending/streaming 状态记录

        Args:
            db: 数据库会话
            records: 记录列表，每条记录包含 record_usage 所需的参数

        Returns:
            创建的 Usage 记录列表
        """
        if not records:
            return []

        from collections import defaultdict

        from sqlalchemy import update

        from src.models.database import ApiKey as ApiKeyModel
        from src.models.database import GlobalModel
        from src.models.database import User as UserModel

        # 分离需要更新和需要新建的记录
        request_ids = [r.get("request_id") for r in records if r.get("request_id")]
        existing_usages: dict[str, Usage] = {}
        records_to_update: list[dict[str, Any]] = []
        records_to_insert: list[dict[str, Any]] = []

        if request_ids:
            # 查询已存在的 Usage 记录（包括 pending/streaming 状态）
            from sqlalchemy.orm import selectinload

            existing_records = (
                db.query(Usage)
                .options(
                    selectinload(Usage.user),
                    selectinload(Usage.api_key),
                )
                .filter(Usage.request_id.in_(request_ids))
                .all()
            )
            existing_usages = {u.request_id: u for u in existing_records}

            for record in records:
                req_id = record.get("request_id")
                if req_id and req_id in existing_usages:
                    existing_usage = existing_usages[req_id]
                    # 以 billing_status 为幂等闸门：
                    # - pending: 允许更新（补全 tokens/cost/headers/body 等）
                    # - settled/void: 跳过（避免重复记账/重复覆盖）
                    #
                    # 注意：stream_telemetry 在 usage_queue_enabled 时会"直接更新 status"
                    # 来减少 UI 延迟，但不会同步更新 billing_status。
                    # 这会导致出现 status=completed 但 billing_status=pending 的中间态，
                    # 此时仍应允许 completed/failed/cancelled 事件落库补全详情。
                    billing_status = getattr(existing_usage, "billing_status", None)
                    if billing_status == "pending":
                        records_to_update.append(record)
                    else:
                        logger.debug(
                            "批量记录预过滤: 跳过已结算的 request_id={} (status={}, billing_status={})",
                            req_id,
                            getattr(existing_usage, "status", None),
                            billing_status,
                        )
                else:
                    records_to_insert.append(record)
        else:
            records_to_insert = list(records)

        if records_to_update:
            logger.debug(
                f"批量记录: 需要更新 {len(records_to_update)} 条已存在的 billing_status=pending 记录"
            )

        usages: list[Usage] = []
        user_costs: dict[str, float] = defaultdict(float)  # user_id -> total_cost
        apikey_stats: dict[str, dict[str, Any]] = defaultdict(
            lambda: {"requests": 0, "cost": 0.0, "is_standalone": False}
        )
        model_counts: dict[str, int] = defaultdict(int)  # model -> count
        provider_costs: dict[str, float] = defaultdict(float)  # provider_id -> cost

        # 合并所有需要处理的记录（用于预取 user/api_key）
        all_records = records_to_insert + records_to_update

        # 批量预取 User 和 ApiKey，避免 N+1 查询
        user_ids = {r.get("user_id") for r in all_records if r.get("user_id")}
        api_key_ids = {r.get("api_key_id") for r in all_records if r.get("api_key_id")}

        users_map: dict[str, User] = {}
        if user_ids:
            users = db.query(User).filter(User.id.in_(user_ids)).all()
            users_map = {str(u.id): u for u in users}

        api_keys_map: dict[str, ApiKey] = {}
        if api_key_ids:
            api_keys = db.query(ApiKey).filter(ApiKey.id.in_(api_key_ids)).all()
            api_keys_map = {str(k.id): k for k in api_keys}

        skipped_count = 0
        updated_count = 0
        total_count = len(all_records)

        # 辅助函数：构建 UsageRecordParams
        def build_params(record: dict[str, Any], request_id: str) -> UsageRecordParams:
            user_id = record.get("user_id")
            api_key_id = record.get("api_key_id")
            user = users_map.get(str(user_id)) if user_id else None
            api_key = api_keys_map.get(str(api_key_id)) if api_key_id else None

            return UsageRecordParams(
                db=db,
                user=user,
                api_key=api_key,
                provider=record.get("provider") or "unknown",
                model=record.get("model") or "unknown",
                input_tokens=int(record.get("input_tokens") or 0),
                output_tokens=int(record.get("output_tokens") or 0),
                cache_creation_input_tokens=int(record.get("cache_creation_input_tokens") or 0),
                cache_read_input_tokens=int(record.get("cache_read_input_tokens") or 0),
                request_type=record.get("request_type") or "chat",
                api_format=record.get("api_format"),
                endpoint_api_format=record.get("endpoint_api_format"),
                has_format_conversion=bool(record.get("has_format_conversion")),
                is_stream=bool(record.get("is_stream", True)),
                response_time_ms=record.get("response_time_ms"),
                first_byte_time_ms=record.get("first_byte_time_ms"),
                status_code=int(record.get("status_code") or 200),
                error_message=record.get("error_message"),
                metadata=record.get("metadata"),
                request_headers=record.get("request_headers"),
                request_body=record.get("request_body"),
                provider_request_headers=record.get("provider_request_headers"),
                response_headers=record.get("response_headers"),
                client_response_headers=record.get("client_response_headers"),
                response_body=record.get("response_body"),
                request_id=request_id,
                provider_id=record.get("provider_id"),
                provider_endpoint_id=record.get("provider_endpoint_id"),
                provider_api_key_id=record.get("provider_api_key_id"),
                status=record.get("status") or "completed",
                cache_ttl_minutes=record.get("cache_ttl_minutes"),
                use_tiered_pricing=record.get("use_tiered_pricing", True),
                target_model=record.get("target_model"),
            )

        # 构建所有参数并并行准备
        update_params_list: list[tuple[dict[str, Any], str, UsageRecordParams]] = []
        for record in records_to_update:
            request_id = record.get("request_id")
            if request_id and request_id in existing_usages:
                existing_usage = existing_usages[request_id]
                if existing_usage:
                    try:
                        params = build_params(record, request_id)
                        update_params_list.append((record, request_id, params))
                    except Exception as e:
                        skipped_count += 1
                        logger.warning("批量记录中参数构建失败: {}, request_id={}", e, request_id)

        insert_params_list: list[tuple[dict[str, Any], str, UsageRecordParams]] = []
        for record in records_to_insert:
            request_id = record.get("request_id") or str(uuid.uuid4())[:8]
            try:
                params = build_params(record, request_id)
                insert_params_list.append((record, request_id, params))
            except Exception as e:
                skipped_count += 1
                logger.warning("批量记录中参数构建失败: {}, request_id={}", e, request_id)

        # 并行准备所有记录（性能优化）
        all_params = [p for _, _, p in update_params_list] + [p for _, _, p in insert_params_list]
        if all_params:
            prepared_results = await cls._prepare_usage_records_batch(all_params)
        else:
            prepared_results = []

        # 分配准备结果
        update_results = prepared_results[: len(update_params_list)]
        insert_results = prepared_results[len(update_params_list) :]

        finalized_at = datetime.now(timezone.utc)
        terminal_statuses = {"completed", "failed", "cancelled"}

        # 1. 处理需要更新的记录
        for i, (record, request_id, params) in enumerate(update_params_list):
            try:
                usage_params, total_cost, exc = update_results[i]
                if exc:
                    raise exc

                # existing_usage 已在构建阶段验证存在
                existing_usage = existing_usages[request_id]
                user = params.user
                api_key = params.api_key

                # 更新已存在的 Usage 记录
                cls._update_existing_usage(existing_usage, usage_params, record.get("target_model"))
                # 结算标记：pending -> settled（幂等闸门由 prefilter 控制）
                if (
                    usage_params.get("status") in terminal_statuses
                    and getattr(existing_usage, "billing_status", None) == "pending"
                ):
                    existing_usage.billing_status = "settled"
                    if getattr(existing_usage, "finalized_at", None) is None:
                        existing_usage.finalized_at = finalized_at
                usages.append(existing_usage)
                updated_count += 1

                # 聚合统计
                model_name = record.get("model") or "unknown"
                model_counts[model_name] += 1

                provider_id = record.get("provider_id")
                if provider_id:
                    actual_cost = usage_params.get("actual_total_cost_usd", 0)
                    provider_costs[provider_id] += actual_cost

                if user and not (api_key and api_key.is_standalone):
                    user_costs[str(user.id)] += total_cost

                if api_key:
                    key_id = str(api_key.id)
                    apikey_stats[key_id]["requests"] += 1
                    apikey_stats[key_id]["cost"] += total_cost
                    apikey_stats[key_id]["is_standalone"] = api_key.is_standalone

            except Exception as e:
                skipped_count += 1
                logger.warning("批量记录中更新失败: {}, request_id={}", e, request_id)
                continue

        # 2. 处理需要新建的记录（批量插入）
        insert_mappings: list[dict[str, Any]] = []
        insert_request_ids: list[str] = []

        for i, (record, request_id, params) in enumerate(insert_params_list):
            try:
                usage_params, total_cost, exc = insert_results[i]
                if exc:
                    raise exc

                user = params.user
                api_key = params.api_key

                # 终态记录：补齐 settled/finalized_at；非终态：确保 billing_status=pending
                status = usage_params.get("status")
                if status in terminal_statuses:
                    if usage_params.get("billing_status") in (None, "pending"):
                        usage_params["billing_status"] = "settled"
                    usage_params.setdefault("finalized_at", finalized_at)
                elif usage_params.get("billing_status") is None:
                    usage_params["billing_status"] = "pending"

                insert_mappings.append(usage_params)
                insert_request_ids.append(request_id)

                # 聚合统计
                model_name = record.get("model") or "unknown"
                model_counts[model_name] += 1

                provider_id = record.get("provider_id")
                if provider_id:
                    actual_cost = usage_params.get("actual_total_cost_usd", 0)
                    provider_costs[provider_id] += actual_cost

                # 用户统计（独立 Key 不计入创建者）
                if user and not (api_key and api_key.is_standalone):
                    user_costs[str(user.id)] += total_cost

                # API Key 统计
                if api_key:
                    key_id = str(api_key.id)
                    apikey_stats[key_id]["requests"] += 1
                    apikey_stats[key_id]["cost"] += total_cost
                    apikey_stats[key_id]["is_standalone"] = api_key.is_standalone

            except Exception as e:
                skipped_count += 1
                logger.warning("批量记录中跳过无效记录: {}, request_id={}", e, request_id)
                continue

        if insert_mappings:
            try:
                db.bulk_insert_mappings(Usage, insert_mappings)

                # 仅用于保持返回值语义：将新建记录读回为 ORM 对象
                inserted_records = (
                    db.query(Usage).filter(Usage.request_id.in_(insert_request_ids)).all()
                )
                inserted_map = {u.request_id: u for u in inserted_records}
                for rid in insert_request_ids:
                    inserted_usage = inserted_map.get(rid)
                    if inserted_usage is not None:
                        usages.append(inserted_usage)
            except Exception as e:
                logger.error("批量插入 Usage 记录时出错: {}", e)
                db.rollback()
                raise

        # 统计跳过的记录，失败率超过 10% 时提升日志级别
        if skipped_count > 0:
            skip_ratio = skipped_count / total_count if total_count > 0 else 0
            if skip_ratio > 0.1:
                logger.error(
                    "批量记录失败率过高: {}/{} ({:.1f}%) 条记录被跳过",
                    skipped_count,
                    total_count,
                    skip_ratio * 100,
                )
            else:
                logger.warning("批量记录部分失败: {}/{} 条记录被跳过", skipped_count, total_count)

        # 批量更新 GlobalModel 使用计数
        for model_name, count in model_counts.items():
            db.execute(
                update(GlobalModel)
                .where(GlobalModel.name == model_name)
                .values(usage_count=GlobalModel.usage_count + count)
            )

        # 批量更新 Provider 月度使用量
        for provider_id, cost in provider_costs.items():
            if cost > 0:
                db.execute(
                    update(Provider)
                    .where(Provider.id == provider_id)
                    .values(monthly_used_usd=Provider.monthly_used_usd + cost)
                )

        # 批量更新用户使用量
        from sqlalchemy import func as sql_func

        for user_id, cost in user_costs.items():
            if cost > 0:
                db.execute(
                    update(UserModel)
                    .where(UserModel.id == user_id)
                    .values(
                        used_usd=UserModel.used_usd + cost,
                        total_usd=UserModel.total_usd + cost,
                        updated_at=sql_func.now(),
                    )
                )

        # 批量更新 API Key 统计
        for key_id, stats in apikey_stats.items():
            if stats["is_standalone"]:
                db.execute(
                    update(ApiKeyModel)
                    .where(ApiKeyModel.id == key_id)
                    .values(
                        total_requests=ApiKeyModel.total_requests + stats["requests"],
                        total_cost_usd=ApiKeyModel.total_cost_usd + stats["cost"],
                        balance_used_usd=ApiKeyModel.balance_used_usd + stats["cost"],
                        last_used_at=sql_func.now(),
                        updated_at=sql_func.now(),
                    )
                )
            else:
                db.execute(
                    update(ApiKeyModel)
                    .where(ApiKeyModel.id == key_id)
                    .values(
                        total_requests=ApiKeyModel.total_requests + stats["requests"],
                        total_cost_usd=ApiKeyModel.total_cost_usd + stats["cost"],
                        last_used_at=sql_func.now(),
                        updated_at=sql_func.now(),
                    )
                )

        # 单次提交所有更改
        try:
            db.commit()
            inserted_count = len(insert_mappings)
            total_written = updated_count + inserted_count
            if updated_count > 0:
                logger.debug("批量记录成功: 更新 {} 条, 新建 {} 条", updated_count, inserted_count)
            else:
                logger.debug("批量记录 {} 条使用记录成功", total_written)
        except Exception as e:
            logger.error("批量提交使用记录时出错: {}", e)
            db.rollback()
            raise

        return usages
