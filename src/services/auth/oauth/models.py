from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Optional


@dataclass(frozen=True)
class OAuthToken:
    access_token: str
    token_type: str = "bearer"
    refresh_token: Optional[str] = None
    expires_in: Optional[int] = None
    id_token: Optional[str] = None
    scope: Optional[str] = None
    raw: Optional[dict[str, Any]] = None


@dataclass(frozen=True)
class OAuthUserInfo:
    id: str
    username: Optional[str] = None
    email: Optional[str] = None
    email_verified: Optional[bool] = None
    raw: Optional[dict[str, Any]] = None


class OAuthFlowError(Exception):
    """用于 OAuth 流程的可控错误（会映射到 error_code）。"""

    def __init__(self, error_code: str, detail: str = ""):
        super().__init__(error_code)
        self.error_code = error_code
        self.detail = detail

