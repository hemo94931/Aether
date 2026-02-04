from __future__ import annotations

from enum import Enum


class ProviderType(str, Enum):
    CUSTOM = "custom"
    CLAUDE_CODE = "claude_code"
    CODEX = "codex"
    GEMINI_CLI = "gemini_cli"
    ANTIGRAVITY = "antigravity"


__all__ = ["ProviderType"]
