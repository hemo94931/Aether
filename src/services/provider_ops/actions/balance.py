"""
余额查询操作
"""

import time
from typing import Any, Dict, Optional

import httpx

from src.services.provider_ops.actions.base import ProviderAction
from src.services.provider_ops.types import (
    ActionResult,
    ActionStatus,
    BalanceInfo,
    ProviderActionType,
)


class BalanceAction(ProviderAction):
    """
    余额查询操作

    支持可配置的 endpoint 和响应字段映射。
    """

    action_type = ProviderActionType.QUERY_BALANCE
    display_name = "查询余额"
    description = "查询账户余额信息"
    default_cache_ttl = 86400  # 24 小时

    async def execute(self, client: httpx.AsyncClient) -> ActionResult:
        """执行余额查询"""
        endpoint = self.config.get("endpoint", "/api/user/balance")
        method = self.config.get("method", "GET")
        mapping = self.config.get("response_mapping", {})

        start_time = time.time()

        try:
            response = await client.request(method, endpoint)
            response_time_ms = int((time.time() - start_time) * 1000)

            # 尝试解析 JSON
            try:
                data = response.json()
            except Exception:
                return self._make_error_result(
                    ActionStatus.PARSE_ERROR,
                    "响应不是有效的 JSON",
                )

            # 检查 HTTP 状态
            if response.status_code != 200:
                return self._handle_http_error(response, data)

            # 检查业务状态码（如果配置了）
            success_field = self.config.get("success_field")
            if success_field:
                is_success = self._extract_field(data, success_field)
                if is_success is False or is_success == 0:
                    message = self._extract_field(data, self.config.get("message_field", "message"))
                    return self._make_error_result(
                        ActionStatus.UNKNOWN_ERROR,
                        message or "业务状态码表示失败",
                        raw_response=data,
                    )

            # 解析余额信息
            balance = self._parse_balance(data, mapping)

            return self._make_success_result(
                data=balance,
                response_time_ms=response_time_ms,
                raw_response=data,
            )

        except httpx.TimeoutException:
            return self._make_error_result(
                ActionStatus.NETWORK_ERROR,
                "请求超时",
                retry_after_seconds=30,
            )
        except httpx.RequestError as e:
            return self._make_error_result(
                ActionStatus.NETWORK_ERROR,
                f"网络错误: {str(e)}",
                retry_after_seconds=30,
            )
        except Exception as e:
            return self._make_error_result(
                ActionStatus.UNKNOWN_ERROR,
                f"未知错误: {str(e)}",
            )

    def _parse_balance(self, data: Any, mapping: Dict[str, str]) -> BalanceInfo:
        """解析余额信息"""
        # 默认映射（常见字段名）
        default_mappings = {
            "total_granted": ["data.total_quota", "data.quota", "total_quota", "quota"],
            "total_used": ["data.used_quota", "data.used", "used_quota", "used"],
            "total_available": [
                "data.balance",
                "data.remaining",
                "data.available",
                "balance",
                "remaining",
            ],
        }

        # 获取 quota 除数（用于将原始值转换为美元，如 New API 的 1/500000）
        quota_divisor = self.config.get("quota_divisor", 1)

        def get_value(field: str, default_paths: list) -> Optional[float]:
            # 优先使用用户配置的映射
            if field in mapping:
                value = self._extract_field(data, mapping[field])
                if value is not None:
                    raw = self._to_float(value)
                    return raw / quota_divisor if raw is not None else None

            # 尝试默认映射
            for path in default_paths:
                value = self._extract_field(data, path)
                if value is not None:
                    raw = self._to_float(value)
                    return raw / quota_divisor if raw is not None else None

            return None

        total_granted = get_value("total_granted", default_mappings["total_granted"])
        total_used = get_value("total_used", default_mappings["total_used"])
        total_available = get_value("total_available", default_mappings["total_available"])

        # 如果只有部分数据，尝试计算
        if total_available is None and total_granted is not None and total_used is not None:
            total_available = total_granted - total_used
        if total_used is None and total_granted is not None and total_available is not None:
            total_used = total_granted - total_available
        if total_granted is None and total_used is not None and total_available is not None:
            total_granted = total_used + total_available

        # 提取额外字段
        extra = {}
        for key, path in mapping.items():
            if key not in ["total_granted", "total_used", "total_available", "expires_at"]:
                value = self._extract_field(data, path)
                if value is not None:
                    extra[key] = value

        return BalanceInfo(
            total_granted=total_granted,
            total_used=total_used,
            total_available=total_available,
            currency=self.config.get("currency", "USD"),
            extra=extra,
        )

    def _to_float(self, value: Any) -> Optional[float]:
        """转换为浮点数"""
        if value is None:
            return None
        try:
            return float(value)
        except (TypeError, ValueError):
            return None

    @classmethod
    def get_config_schema(cls) -> Dict[str, Any]:
        """获取操作配置 schema"""
        return {
            "type": "object",
            "properties": {
                "endpoint": {
                    "type": "string",
                    "title": "API 路径",
                    "description": "余额查询 API 路径",
                    "default": "/api/user/balance",
                },
                "method": {
                    "type": "string",
                    "title": "请求方法",
                    "enum": ["GET", "POST"],
                    "default": "GET",
                },
                "quota_divisor": {
                    "type": "number",
                    "title": "额度除数",
                    "description": "将原始额度值转换为美元的除数（如 New API 为 500000）",
                    "default": 1,
                },
                "success_field": {
                    "type": "string",
                    "title": "成功状态字段",
                    "description": "响应中表示成功的字段路径（如 success, code）",
                },
                "message_field": {
                    "type": "string",
                    "title": "消息字段",
                    "description": "响应中的消息字段路径",
                    "default": "message",
                },
                "response_mapping": {
                    "type": "object",
                    "title": "响应字段映射",
                    "description": "响应字段到余额字段的映射",
                    "properties": {
                        "total_granted": {
                            "type": "string",
                            "title": "总额度字段",
                            "description": "响应中总额度的字段路径",
                        },
                        "total_used": {
                            "type": "string",
                            "title": "已用额度字段",
                            "description": "响应中已用额度的字段路径",
                        },
                        "total_available": {
                            "type": "string",
                            "title": "可用余额字段",
                            "description": "响应中可用余额的字段路径",
                        },
                    },
                },
                "currency": {
                    "type": "string",
                    "title": "货币单位",
                    "default": "USD",
                },
            },
            "required": ["endpoint"],
        }
