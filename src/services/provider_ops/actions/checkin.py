"""
签到操作
"""

import time
from typing import Any, Dict

import httpx

from src.services.provider_ops.actions.base import ProviderAction
from src.services.provider_ops.types import (
    ActionResult,
    ActionStatus,
    CheckinInfo,
    ProviderActionType,
)


class CheckinAction(ProviderAction):
    """
    签到操作

    支持可配置的 endpoint 和响应字段映射。
    """

    action_type = ProviderActionType.CHECKIN
    display_name = "签到"
    description = "每日签到领取额度"
    default_cache_ttl = 3600  # 签到结果缓存 1 小时

    async def execute(self, client: httpx.AsyncClient) -> ActionResult:
        """执行签到"""
        endpoint = self.config.get("endpoint", "/api/user/checkin")
        method = self.config.get("method", "POST")

        start_time = time.time()

        try:
            # 构建请求
            request_body = self.config.get("request_body", {})

            if method == "POST":
                response = await client.post(endpoint, json=request_body or None)
            else:
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

            # 解析签到结果
            checkin_info, status, message = self._parse_checkin_result(data)

            if status == ActionStatus.SUCCESS:
                return self._make_success_result(
                    data=checkin_info,
                    message=message,
                    response_time_ms=response_time_ms,
                    raw_response=data,
                )
            else:
                return self._make_error_result(
                    status,
                    message,
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

    def _parse_checkin_result(
        self, data: Any
    ) -> tuple[CheckinInfo, ActionStatus, str | None]:
        """
        解析签到结果

        Returns:
            (CheckinInfo, 状态, 消息)
        """
        mapping = self.config.get("response_mapping", {})

        # 检查成功状态
        success_field = self.config.get("success_field", "success")
        is_success = self._extract_field(data, success_field)

        # 获取消息
        message_field = self.config.get("message_field", "message")
        message = self._extract_field(data, message_field)
        if message is not None:
            message = str(message)

        # 检查是否已签到
        already_checked_indicators = self.config.get(
            "already_checked_indicators", ["already", "已签到", "今日已签", "重复签到"]
        )
        if message:
            for indicator in already_checked_indicators:
                if indicator.lower() in message.lower():
                    return (
                        CheckinInfo(message=message),
                        ActionStatus.ALREADY_DONE,
                        message,
                    )

        # 判断是否成功
        if is_success is False or is_success == 0:
            return (
                CheckinInfo(message=message),
                ActionStatus.UNKNOWN_ERROR,
                message or "签到失败",
            )

        # 解析签到信息
        reward = None
        reward_field = mapping.get("reward") or self.config.get("reward_field")
        if reward_field:
            reward_value = self._extract_field(data, reward_field)
            if reward_value is not None:
                try:
                    reward = float(reward_value)
                except (TypeError, ValueError):
                    pass

        streak_days = None
        streak_field = mapping.get("streak_days") or self.config.get("streak_field")
        if streak_field:
            streak_value = self._extract_field(data, streak_field)
            if streak_value is not None:
                try:
                    streak_days = int(streak_value)
                except (TypeError, ValueError):
                    pass

        # 提取额外字段
        extra = {}
        for key, path in mapping.items():
            if key not in ["reward", "streak_days", "message"]:
                value = self._extract_field(data, path)
                if value is not None:
                    extra[key] = value

        checkin_info = CheckinInfo(
            reward=reward,
            streak_days=streak_days,
            message=message,
            extra=extra,
        )

        return (checkin_info, ActionStatus.SUCCESS, message or "签到成功")

    @classmethod
    def get_config_schema(cls) -> Dict[str, Any]:
        """获取操作配置 schema"""
        return {
            "type": "object",
            "properties": {
                "endpoint": {
                    "type": "string",
                    "title": "API 路径",
                    "description": "签到 API 路径",
                    "default": "/api/user/checkin",
                },
                "method": {
                    "type": "string",
                    "title": "请求方法",
                    "enum": ["GET", "POST"],
                    "default": "POST",
                },
                "request_body": {
                    "type": "object",
                    "title": "请求体",
                    "description": "签到请求的 JSON 体（可选）",
                },
                "success_field": {
                    "type": "string",
                    "title": "成功状态字段",
                    "description": "响应中表示成功的字段路径",
                    "default": "success",
                },
                "message_field": {
                    "type": "string",
                    "title": "消息字段",
                    "description": "响应中的消息字段路径",
                    "default": "message",
                },
                "reward_field": {
                    "type": "string",
                    "title": "奖励字段",
                    "description": "响应中奖励额度的字段路径",
                },
                "streak_field": {
                    "type": "string",
                    "title": "连续签到天数字段",
                    "description": "响应中连续签到天数的字段路径",
                },
                "already_checked_indicators": {
                    "type": "array",
                    "title": "已签到标识",
                    "description": "消息中表示已签到的关键词",
                    "items": {"type": "string"},
                    "default": ["already", "已签到", "今日已签", "重复签到"],
                },
                "response_mapping": {
                    "type": "object",
                    "title": "响应字段映射",
                    "description": "响应字段到签到信息的映射",
                },
            },
            "required": ["endpoint"],
        }
