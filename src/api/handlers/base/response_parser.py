"""
响应解析器基类 - 定义统一的响应解析接口
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, Optional


@dataclass
class ParsedChunk:
    """解析后的流式数据块"""

    # 原始数据
    raw_line: str
    event_type: Optional[str] = None
    data: Optional[Dict[str, Any]] = None

    # 提取的内容
    text_delta: str = ""
    is_done: bool = False
    is_error: bool = False
    error_message: Optional[str] = None

    # 使用量信息（通常在最后一个 chunk 中）
    input_tokens: int = 0
    output_tokens: int = 0
    cache_creation_tokens: int = 0
    cache_read_tokens: int = 0

    # 响应 ID
    response_id: Optional[str] = None


@dataclass
class StreamStats:
    """流式响应统计信息"""

    # 计数
    chunk_count: int = 0
    data_count: int = 0

    # Token 使用量
    input_tokens: int = 0
    output_tokens: int = 0
    cache_creation_tokens: int = 0
    cache_read_tokens: int = 0

    # 内容
    collected_text: str = ""
    response_id: Optional[str] = None

    # 状态
    has_completion: bool = False
    status_code: int = 200
    error_message: Optional[str] = None

    # Provider 信息
    provider_name: Optional[str] = None
    endpoint_id: Optional[str] = None
    key_id: Optional[str] = None

    # 响应头和完整响应
    response_headers: Dict[str, str] = field(default_factory=dict)
    final_response: Optional[Dict[str, Any]] = None


@dataclass
class ParsedResponse:
    """解析后的非流式响应"""

    # 原始响应
    raw_response: Dict[str, Any]
    status_code: int

    # 提取的内容
    text_content: str = ""
    response_id: Optional[str] = None

    # 使用量
    input_tokens: int = 0
    output_tokens: int = 0
    cache_creation_tokens: int = 0
    cache_read_tokens: int = 0

    # 错误信息
    is_error: bool = False
    error_type: Optional[str] = None
    error_message: Optional[str] = None
    # 从响应体解析出的嵌套状态码（当 HTTP 200 但响应体含错误时使用）
    embedded_status_code: Optional[int] = None


class ResponseParser(ABC):
    """
    响应解析器基类

    定义统一的接口来解析不同 API 格式的响应。
    子类需要实现具体的解析逻辑。
    """

    # 解析器名称（用于日志）
    name: str = "base"

    # 支持的 API 格式
    api_format: str = "UNKNOWN"

    @abstractmethod
    def parse_sse_line(self, line: str, stats: StreamStats) -> Optional[ParsedChunk]:
        """
        解析单行 SSE 数据

        Args:
            line: SSE 行数据
            stats: 流统计对象（会被更新）

        Returns:
            解析后的数据块，如果行不包含有效数据则返回 None
        """
        pass

    @abstractmethod
    def parse_response(self, response: Dict[str, Any], status_code: int) -> ParsedResponse:
        """
        解析非流式响应

        Args:
            response: 响应 JSON
            status_code: HTTP 状态码

        Returns:
            解析后的响应对象
        """
        pass

    @abstractmethod
    def extract_usage_from_response(self, response: Dict[str, Any]) -> Dict[str, int]:
        """
        从响应中提取 token 使用量

        Args:
            response: 响应 JSON

        Returns:
            包含 input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens 的字典
        """
        pass

    @abstractmethod
    def extract_text_content(self, response: Dict[str, Any]) -> str:
        """
        从响应中提取文本内容

        Args:
            response: 响应 JSON

        Returns:
            提取的文本内容
        """
        pass

    def is_error_response(self, response: Dict[str, Any]) -> bool:
        """
        判断响应是否为错误响应

        Args:
            response: 响应 JSON

        Returns:
            是否为错误响应
        """
        return "error" in response

    def create_stats(self) -> StreamStats:
        """创建新的流统计对象"""
        return StreamStats()
