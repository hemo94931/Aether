"""
API 格式检测

提供从请求头、响应内容等检测 API 格式的函数。
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from starlette.requests import Request

from src.core.api_format.enums import APIFormat, AuthMethod, EndpointType
from src.core.api_format.metadata import API_FORMAT_DEFINITIONS, ApiFormatDefinition


def _extract_api_key_by_definition(
    headers: dict[str, str],
    query_params: dict[str, str] | None,
    definition: ApiFormatDefinition,
) -> tuple[str | None, str]:
    """
    根据格式定义从请求中提取 API Key

    Args:
        headers: 请求头字典（key 小写）
        query_params: 查询参数字典（可选）
        definition: API 格式定义

    Returns:
        (api_key, auth_method) 元组：
        - api_key: 提取到的 API Key，或 None
        - auth_method: 认证方式 ("header" 或 "query")
    """
    auth_header = definition.auth_header.lower()
    auth_type = definition.auth_type

    # Gemini 格式：query 参数优先（与 Google SDK 行为一致）
    if definition.api_format in (APIFormat.GEMINI, APIFormat.GEMINI_CLI):
        # 1. 优先检查 ?key= 参数
        query_key = query_params.get("key") if query_params else None
        if query_key:
            return query_key, "query"
        # 2. 再检查 x-goog-api-key 请求头
        header_value = headers.get(auth_header)
        if header_value:
            return header_value, "header"
        return None, "header"

    # 其他格式：从 header 提取
    header_value = headers.get(auth_header)
    if not header_value:
        return None, "header"

    if auth_type == "bearer":
        # Bearer token: "Bearer xxx"
        if header_value.lower().startswith("bearer "):
            return header_value[7:].strip(), "header"
        return None, "header"
    else:
        # header 类型: 直接使用值
        return header_value, "header"


@dataclass(frozen=True)
class RequestContext:
    """请求上下文 - 三维度信息"""

    data_format: APIFormat
    endpoint_type: EndpointType
    auth_method: AuthMethod
    credentials: str | None


def _detect_endpoint_type(path: str) -> EndpointType:
    normalized = path.lower()

    if normalized.startswith("/upload/v1beta/files") or normalized.startswith("/v1beta/files"):
        return EndpointType.FILES
    if normalized.startswith("/v1/videos") or (
        normalized.startswith("/v1beta/") and "predictlongrunning" in normalized
    ):
        return EndpointType.VIDEO
    # Gemini operations (视频轮询) 也归类为 VIDEO
    if normalized.startswith("/v1beta/operations"):
        return EndpointType.VIDEO
    if normalized.startswith("/v1/models"):
        return EndpointType.MODELS
    if "/embeddings" in normalized:
        return EndpointType.EMBEDDING
    if "/images" in normalized:
        return EndpointType.IMAGE
    if "/audio" in normalized:
        return EndpointType.AUDIO
    return EndpointType.CHAT


def _detect_data_format(
    path: str, headers: dict[str, str], query_params: dict[str, str] | None
) -> APIFormat:
    normalized = path.lower()

    if normalized.startswith("/v1/messages"):
        return APIFormat.CLAUDE
    if normalized.startswith("/v1beta/") or normalized.startswith("/upload/v1beta/"):
        return APIFormat.GEMINI
    if normalized.startswith("/v1/chat/completions") or normalized.startswith("/v1/videos"):
        return APIFormat.OPENAI

    api_format, _api_key, _auth_method = detect_format_from_request(headers, query_params)
    return api_format


def _detect_auth_method(
    headers: dict[str, str], query_params: dict[str, str] | None
) -> tuple[AuthMethod, str | None]:
    # Query key (Gemini) has highest priority
    query_key = query_params.get("key") if query_params else None
    if query_key:
        return AuthMethod.QUERY_KEY, query_key

    x_goog_key = headers.get("x-goog-api-key")
    if x_goog_key:
        return AuthMethod.GOOG_API_KEY, x_goog_key

    x_api_key = headers.get("x-api-key")
    if x_api_key:
        return AuthMethod.API_KEY, x_api_key

    auth_header = headers.get("authorization", "")
    if auth_header.lower().startswith("bearer "):
        return AuthMethod.BEARER, auth_header[7:].strip()

    return AuthMethod.BEARER, None


def detect_format_from_request(
    headers: dict[str, str],
    query_params: dict[str, str] | None = None,
) -> tuple[APIFormat, str | None, str]:
    """
    从请求头检测 API 格式和 API Key

    检测优先级：
    1. x-api-key + anthropic-version -> Claude
    2. x-goog-api-key 或 ?key= -> Gemini
    3. Authorization: Bearer -> OpenAI (默认)

    Args:
        headers: 请求头字典（key 应为小写）
        query_params: 查询参数字典（可选）

    Returns:
        (APIFormat, api_key, auth_method) 元组
        - auth_method: 认证方式 ("header" 或 "query")
    """
    # Claude: x-api-key + anthropic-version (必须同时存在)
    claude_def = API_FORMAT_DEFINITIONS[APIFormat.CLAUDE]
    claude_key, claude_auth_method = _extract_api_key_by_definition(
        headers, query_params, claude_def
    )
    if claude_key and headers.get("anthropic-version"):
        return APIFormat.CLAUDE, claude_key, claude_auth_method

    # Gemini: x-goog-api-key (header 类型) 或 ?key=
    gemini_def = API_FORMAT_DEFINITIONS[APIFormat.GEMINI]
    gemini_key, gemini_auth_method = _extract_api_key_by_definition(
        headers, query_params, gemini_def
    )
    if gemini_key:
        return APIFormat.GEMINI, gemini_key, gemini_auth_method

    # OpenAI: Authorization: Bearer (默认)
    # 注意: 如果只有 x-api-key 但没有 anthropic-version，也走 OpenAI 格式
    openai_def = API_FORMAT_DEFINITIONS[APIFormat.OPENAI]
    openai_key, openai_auth_method = _extract_api_key_by_definition(
        headers, query_params, openai_def
    )
    # 如果 OpenAI 格式没有 key，但有 x-api-key，也用它（兼容）
    if not openai_key and claude_key:
        openai_key = claude_key
        openai_auth_method = claude_auth_method
    return APIFormat.OPENAI, openai_key, openai_auth_method


def detect_format_and_key_from_starlette(
    request: Request,
) -> tuple[str, str | None, str]:
    """
    从 Starlette Request 对象检测 API 格式和 API Key

    这是一个便捷函数，用于直接处理 Starlette/FastAPI 请求对象。

    Args:
        request: Starlette Request 对象

    Returns:
        (format_name, api_key, auth_method) 元组
        - format_name: 为小写字符串
        - auth_method: 认证方式 ("header" 或 "query")
    """
    # 规范化 headers 为小写
    headers = {k.lower(): v for k, v in request.headers.items()}
    query_params = dict(request.query_params)

    api_format, api_key, auth_method = detect_format_from_request(headers, query_params)

    # 返回小写格式名
    format_name = api_format.value.lower()
    return format_name, api_key, auth_method


def detect_request_context(request: Request) -> RequestContext:
    """
    从 Request 中检测三维度信息

    Returns:
        RequestContext(data_format, endpoint_type, auth_method, credentials)
    """
    headers = {k.lower(): v for k, v in request.headers.items()}
    query_params = dict(request.query_params)

    endpoint_type = _detect_endpoint_type(request.url.path)
    data_format = _detect_data_format(request.url.path, headers, query_params)
    auth_method, credentials = _detect_auth_method(headers, query_params)

    return RequestContext(
        data_format=data_format,
        endpoint_type=endpoint_type,
        auth_method=auth_method,
        credentials=credentials,
    )


def detect_format_from_response(
    response_data: dict,
) -> APIFormat | None:
    """
    从响应内容检测 API 格式

    Args:
        response_data: 响应 JSON 字典

    Returns:
        检测到的格式，或 None
    """
    # Claude: 有 type="message" 或特定的 content 结构
    if response_data.get("type") == "message":
        return APIFormat.CLAUDE
    if "content" in response_data and isinstance(response_data["content"], list):
        first_content = response_data["content"][0] if response_data["content"] else {}
        if first_content.get("type") in ("text", "tool_use"):
            return APIFormat.CLAUDE

    # OpenAI: 有 choices 数组
    if "choices" in response_data:
        return APIFormat.OPENAI

    # Gemini: 有 candidates 数组
    if "candidates" in response_data:
        return APIFormat.GEMINI

    return None


def detect_cli_format_from_path(
    path: str,
    base_format: APIFormat,
) -> bool:
    """
    根据请求路径检测是否为 CLI 模式

    CLI 模式的特征：
    - OpenAI CLI: 请求 /responses 路径
    - Claude CLI: 有特定的路径模式
    - Gemini CLI: 有特定的路径模式

    Args:
        path: 请求路径
        base_format: 基础格式

    Returns:
        True 如果是 CLI 模式
    """
    # OpenAI CLI 特征: /v1/responses 路径
    if base_format == APIFormat.OPENAI and "/responses" in path:
        return True

    # 其他 CLI 模式通常由 Adapter 层根据具体业务逻辑判断
    return False


__all__ = [
    "detect_format_from_request",
    "detect_format_and_key_from_starlette",
    "detect_format_from_response",
    "detect_cli_format_from_path",
    "detect_request_context",
    "RequestContext",
]
