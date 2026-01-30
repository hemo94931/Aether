"""
API 格式枚举定义

定义所有支持的 API 格式，决定请求/响应的处理方式。
"""

from enum import Enum


class APIFormat(Enum):
    """API 格式枚举 - 决定请求/响应的处理方式"""

    CLAUDE = "CLAUDE"  # Claude API 格式
    CLAUDE_CLI = "CLAUDE_CLI"  # Claude CLI API 格式（使用 authorization: Bearer）
    OPENAI = "OPENAI"  # OpenAI API 格式
    OPENAI_CLI = "OPENAI_CLI"  # OpenAI CLI/Responses API 格式（用于 Claude Code 等客户端）
    GEMINI = "GEMINI"  # Google Gemini API 格式
    GEMINI_CLI = "GEMINI_CLI"  # Gemini CLI API 格式


class AuthMethod(str, Enum):
    """认证方式 - 决定如何构造认证 Header"""

    BEARER = "bearer"  # Authorization: Bearer {token}
    API_KEY = "api_key"  # x-api-key: {key}
    GOOG_API_KEY = "goog_key"  # x-goog-api-key: {key}
    OAUTH2 = "oauth2"  # Google OAuth2 / Service Account
    QUERY_KEY = "query_key"  # ?key={key} (Gemini 备用)


class EndpointType(str, Enum):
    """端点类型 - 决定 API 功能类别"""

    CHAT = "chat"  # Chat/Completion API
    VIDEO = "video"  # Video Generation API
    FILES = "files"  # Files API
    IMAGE = "image"  # Image Generation API
    AUDIO = "audio"  # Audio API
    EMBEDDING = "embedding"  # Embedding API
    MODELS = "models"  # Models API


__all__ = ["APIFormat", "AuthMethod", "EndpointType"]
