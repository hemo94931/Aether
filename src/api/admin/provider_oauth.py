"""管理员 Provider OAuth 管理 API。

用于固定类型 Provider 的 OAuth2 授权：
- start: 生成授权 URL（PKCE/state）
- complete: 粘贴 callback_url 完成换 token
- refresh: 手动强制刷新 token

注意：
- 该模块是“上游 Provider OAuth（用于反代调用）”，不是用户登录/绑定 OAuth。
- 不得在日志或响应中返回 access_token/refresh_token/client_secret。
"""

from __future__ import annotations

import json
import secrets
import time
from dataclasses import dataclass
from typing import Any
import base64
import hashlib

from urllib.parse import parse_qsl, urlencode, urlparse

import httpx
from fastapi import APIRouter, Depends, Request
from pydantic import BaseModel, Field
from redis.asyncio import Redis
from sqlalchemy.orm import Session

from src.clients.redis_client import get_redis_client
from src.core.crypto import crypto_service
from src.core.exceptions import InvalidRequestException, NotFoundException
from src.core.logger import logger
from src.database.database import get_db
from src.models.database import Provider, ProviderAPIKey

from src.core.provider_oauth_utils import enrich_auth_config, post_oauth_token
from src.core.provider_templates.fixed_providers import FIXED_PROVIDERS
from src.core.provider_templates.types import ProviderType

router = APIRouter(prefix="/api/admin/provider-oauth", tags=["Provider OAuth"])


# ==============================================================================
# Redis state storage
# ==============================================================================

_PROVIDER_OAUTH_STATE_TTL_SECONDS = 600
_PROVIDER_OAUTH_STATE_PREFIX = "provider_oauth_state:"


_CONSUME_STATE_SCRIPT = r"""
local value = redis.call("GET", KEYS[1])
if value then
    redis.call("DEL", KEYS[1])
end
return value
"""


def _state_key(nonce: str) -> str:
    return f"{_PROVIDER_OAUTH_STATE_PREFIX}{nonce}"


@dataclass(frozen=True)
class ProviderOAuthStateData:
    nonce: str
    key_id: str
    provider_type: str
    pkce_verifier: str | None
    created_at: int


async def _create_state(
    redis: Redis,
    *,
    key_id: str,
    provider_type: str,
    pkce_verifier: str | None,
) -> str:
    nonce = secrets.token_urlsafe(24)
    data = {
        "nonce": nonce,
        "key_id": key_id,
        "provider_type": provider_type,
        "pkce_verifier": pkce_verifier,
        "created_at": int(time.time()),
    }
    await redis.setex(_state_key(nonce), _PROVIDER_OAUTH_STATE_TTL_SECONDS, json.dumps(data))
    return nonce


async def _consume_state(redis: Redis, nonce: str) -> ProviderOAuthStateData | None:
    if not nonce:
        return None
    key = _state_key(nonce)
    raw = await redis.eval(_CONSUME_STATE_SCRIPT, 1, key)
    if not raw:
        return None
    try:
        parsed = json.loads(raw)
    except Exception:
        return None
    return ProviderOAuthStateData(
        nonce=str(parsed.get("nonce") or ""),
        key_id=str(parsed.get("key_id") or ""),
        provider_type=str(parsed.get("provider_type") or ""),
        pkce_verifier=parsed.get("pkce_verifier"),
        created_at=int(parsed.get("created_at") or 0),
    )


# ==============================================================================
# Requests / responses
# ==============================================================================


class StartOAuthResponse(BaseModel):
    authorization_url: str
    redirect_uri: str
    provider_type: str
    instructions: str


class CompleteOAuthRequest(BaseModel):
    callback_url: str = Field(..., min_length=5, description="浏览器地址栏中的完整回调 URL")


class CompleteOAuthResponse(BaseModel):
    provider_type: str
    expires_at: int | None = None
    has_refresh_token: bool = False


# ==============================================================================
# Helpers
# ==============================================================================


def _require_fixed_provider(provider: Provider) -> str:
    provider_type = (getattr(provider, "provider_type", "custom") or "custom").strip()
    if provider_type == "custom":
        raise InvalidRequestException("该 Provider 不是固定类型，无法使用 provider-oauth")
    return provider_type


def _pkce_s256(verifier: str) -> str:
    digest = hashlib.sha256(verifier.encode("utf-8")).digest()
    return base64.urlsafe_b64encode(digest).decode("utf-8").rstrip("=")


def _parse_callback_params(callback_url: str) -> dict[str, str]:
    parsed = urlparse(callback_url.strip())
    query = dict(parse_qsl(parsed.query, keep_blank_values=True))
    fragment = dict(parse_qsl((parsed.fragment or "").lstrip("#"), keep_blank_values=True))
    merged = {**query, **fragment}

    # Claude 参考实现里：code 参数可能包含 "<code>#<state>" 的拼接形式
    code = merged.get("code")
    if code and "#" in code:
        code_part, state_part = code.split("#", 1)
        merged["code"] = code_part
        if "state" not in merged and state_part:
            merged["state"] = state_part

    return {str(k): str(v) for k, v in merged.items()}


# ==============================================================================
# Routes
# ==============================================================================


@router.get("/supported-types")
async def supported_types() -> list[dict[str, Any]]:
    # 不返回 client_secret
    result: list[dict[str, Any]] = []
    for provider_type, template in FIXED_PROVIDERS.items():
        result.append(
            {
                "provider_type": str(provider_type.value) if hasattr(provider_type, "value") else str(provider_type),
                "display_name": template.display_name,
                "scopes": list(template.oauth.scopes),
                "redirect_uri": template.oauth.redirect_uri,
                "authorize_url": template.oauth.authorize_url,
                "token_url": template.oauth.token_url,
                "use_pkce": bool(template.oauth.use_pkce),
            }
        )
    return result


@router.post("/keys/{key_id}/start", response_model=StartOAuthResponse)
async def start_oauth(
    key_id: str,
    request: Request,
    db: Session = Depends(get_db),
) -> StartOAuthResponse:
    key = db.query(ProviderAPIKey).filter(ProviderAPIKey.id == key_id).first()
    if not key:
        raise NotFoundException("Key 不存在", "key")
    if (getattr(key, "auth_type", "api_key") or "api_key") != "oauth":
        raise InvalidRequestException("该 Key 不是 oauth 认证类型")

    provider = db.query(Provider).filter(Provider.id == key.provider_id).first()
    if not provider:
        raise NotFoundException("Provider 不存在", "provider")
    provider_type = _require_fixed_provider(provider)

    try:
        template = FIXED_PROVIDERS.get(ProviderType(provider_type))
    except Exception:
        template = None
    if not template:
        raise InvalidRequestException("不支持的 provider_type")

    redis = await get_redis_client(require_redis=True)
    assert redis is not None

    pkce_verifier: str | None = None
    code_challenge: str | None = None

    if template.oauth.use_pkce:
        pkce_verifier = secrets.token_urlsafe(32)
        code_challenge = _pkce_s256(pkce_verifier)

    state = await _create_state(
        redis,
        key_id=key_id,
        provider_type=provider_type,
        pkce_verifier=pkce_verifier,
    )

    params: dict[str, Any] = {
        "client_id": template.oauth.client_id,
        "response_type": "code",
        "redirect_uri": template.oauth.redirect_uri,
        "scope": " ".join(template.oauth.scopes),
        "state": state,
    }

    # Codex 参考实现额外参数
    if provider_type == ProviderType.CODEX.value:
        params.update(
            {
                "prompt": "login",
                "id_token_add_organizations": "true",
                "codex_cli_simplified_flow": "true",
            }
        )

    if template.oauth.use_pkce and code_challenge:
        params["code_challenge"] = code_challenge
        params["code_challenge_method"] = "S256"

    authorization_url = f"{template.oauth.authorize_url}?{urlencode(params)}"

    return StartOAuthResponse(
        authorization_url=authorization_url,
        redirect_uri=template.oauth.redirect_uri,
        provider_type=provider_type,
        instructions=(
            "1) 打开 authorization_url 完成授权\n"
            "2) 授权后会跳转到 redirect_uri（localhost）\n"
            "3) 复制浏览器地址栏完整 URL，调用 complete 接口粘贴 callback_url"
        ),
    )


@router.post("/keys/{key_id}/complete", response_model=CompleteOAuthResponse)
async def complete_oauth(
    key_id: str,
    payload: CompleteOAuthRequest,
    request: Request,
    db: Session = Depends(get_db),
) -> CompleteOAuthResponse:
    redis = await get_redis_client(require_redis=True)
    assert redis is not None

    params = _parse_callback_params(payload.callback_url)
    code = params.get("code")
    state = params.get("state")
    if not code or not state:
        raise InvalidRequestException("callback_url 缺少 code/state")

    state_data = await _consume_state(redis, state)
    if not state_data or state_data.key_id != key_id:
        raise InvalidRequestException("state 无效或已过期")

    key = db.query(ProviderAPIKey).filter(ProviderAPIKey.id == key_id).first()
    if not key:
        raise NotFoundException("Key 不存在", "key")
    if (getattr(key, "auth_type", "api_key") or "api_key") != "oauth":
        raise InvalidRequestException("该 Key 不是 oauth 认证类型")

    provider = db.query(Provider).filter(Provider.id == key.provider_id).first()
    if not provider:
        raise NotFoundException("Provider 不存在", "provider")
    provider_type = _require_fixed_provider(provider)

    try:
        template = FIXED_PROVIDERS.get(ProviderType(provider_type))
    except Exception:
        template = None
    if not template:
        raise InvalidRequestException("不支持的 provider_type")

    # exchange token
    token_url = template.oauth.token_url

    # Claude token endpoint 是 JSON；Codex/Google 是 form。这里先做最小实现：按 URL 判断。
    is_json = "anthropic.com" in token_url

    if is_json:
        body: dict[str, Any] = {
            "grant_type": "authorization_code",
            "client_id": template.oauth.client_id,
            "redirect_uri": template.oauth.redirect_uri,
            "code": code,
            "state": state,
        }
        if state_data.pkce_verifier:
            body["code_verifier"] = state_data.pkce_verifier
        headers = {"Content-Type": "application/json", "Accept": "application/json"}
        data = None
        json_body = body
    else:
        form: dict[str, str] = {
            "grant_type": "authorization_code",
            "client_id": template.oauth.client_id,
            "redirect_uri": template.oauth.redirect_uri,
            "code": code,
        }
        if template.oauth.client_secret:
            form["client_secret"] = template.oauth.client_secret
        if state_data.pkce_verifier:
            form["code_verifier"] = state_data.pkce_verifier
        headers = {"Content-Type": "application/x-www-form-urlencoded", "Accept": "application/json"}
        data = form
        json_body = None

    proxy_config = getattr(provider, "proxy", None)

    resp = await post_oauth_token(
        provider_type=provider_type,
        token_url=token_url,
        headers=headers,
        data=data,
        json_body=json_body,
        proxy_config=proxy_config,
        timeout_seconds=30.0,
    )

    if resp.status_code < 200 or resp.status_code >= 300:
        raise InvalidRequestException("token exchange 失败")

    token = resp.json()
    access_token = str(token.get("access_token") or "")
    refresh_token = str(token.get("refresh_token") or "")
    expires_in = token.get("expires_in")
    expires_at: int | None = None
    try:
        if expires_in is not None:
            expires_at = int(time.time()) + int(expires_in)
    except Exception:
        expires_at = None

    if not access_token:
        raise InvalidRequestException("token exchange 返回缺少 access_token")

    # store
    key.api_key = crypto_service.encrypt(access_token)
    auth_config: dict[str, Any] = {
        "provider_type": provider_type,
        "token_type": token.get("token_type"),
        "refresh_token": refresh_token or None,
        "expires_at": expires_at,
        "scope": token.get("scope"),
        "updated_at": int(time.time()),
    }

    auth_config = await enrich_auth_config(
        provider_type=provider_type,
        auth_config=auth_config,
        token_response=token,
        access_token=access_token,
        proxy_config=proxy_config,
    )

    key.auth_config = crypto_service.encrypt(json.dumps(auth_config))
    db.commit()

    return CompleteOAuthResponse(
        provider_type=provider_type,
        expires_at=expires_at,
        has_refresh_token=bool(refresh_token),
    )


@router.post("/keys/{key_id}/refresh", response_model=CompleteOAuthResponse)
async def refresh_oauth(
    key_id: str,
    request: Request,
    db: Session = Depends(get_db),
) -> CompleteOAuthResponse:
    key = db.query(ProviderAPIKey).filter(ProviderAPIKey.id == key_id).first()
    if not key:
        raise NotFoundException("Key 不存在", "key")
    if (getattr(key, "auth_type", "api_key") or "api_key") != "oauth":
        raise InvalidRequestException("该 Key 不是 oauth 认证类型")

    provider = db.query(Provider).filter(Provider.id == key.provider_id).first()
    if not provider:
        raise NotFoundException("Provider 不存在", "provider")
    provider_type = _require_fixed_provider(provider)

    try:
        template = FIXED_PROVIDERS.get(ProviderType(provider_type))
    except Exception:
        template = None
    if not template:
        raise InvalidRequestException("不支持的 provider_type")

    encrypted_auth_config = getattr(key, "auth_config", None)
    if not encrypted_auth_config:
        raise InvalidRequestException("缺少 auth_config，无法 refresh")

    decrypted = crypto_service.decrypt(encrypted_auth_config)
    parsed = json.loads(decrypted)
    refresh_token = str(parsed.get("refresh_token") or "")
    if not refresh_token:
        raise InvalidRequestException("缺少 refresh_token，需要重新授权")

    token_url = template.oauth.token_url
    is_json = "anthropic.com" in token_url

    if is_json:
        body: dict[str, Any] = {
            "grant_type": "refresh_token",
            "client_id": template.oauth.client_id,
            "refresh_token": refresh_token,
        }
        headers = {"Content-Type": "application/json", "Accept": "application/json"}
        data = None
        json_body = body
    else:
        form: dict[str, str] = {
            "grant_type": "refresh_token",
            "client_id": template.oauth.client_id,
            "refresh_token": refresh_token,
        }
        if template.oauth.client_secret:
            form["client_secret"] = template.oauth.client_secret
        headers = {"Content-Type": "application/x-www-form-urlencoded", "Accept": "application/json"}
        data = form
        json_body = None

    proxy_config = getattr(provider, "proxy", None)

    resp = await post_oauth_token(
        provider_type=provider_type,
        token_url=token_url,
        headers=headers,
        data=data,
        json_body=json_body,
        proxy_config=proxy_config,
        timeout_seconds=30.0,
    )

    if resp.status_code < 200 or resp.status_code >= 300:
        raise InvalidRequestException("token refresh 失败")

    token = resp.json()
    access_token = str(token.get("access_token") or "")
    new_refresh_token = str(token.get("refresh_token") or "")
    expires_in = token.get("expires_in")
    expires_at: int | None = None
    try:
        if expires_in is not None:
            expires_at = int(time.time()) + int(expires_in)
    except Exception:
        expires_at = None

    if not access_token:
        raise InvalidRequestException("token refresh 返回缺少 access_token")

    # store
    key.api_key = crypto_service.encrypt(access_token)
    parsed["token_type"] = token.get("token_type")
    if new_refresh_token:
        parsed["refresh_token"] = new_refresh_token
    parsed["expires_at"] = expires_at
    parsed["scope"] = token.get("scope")
    parsed["updated_at"] = int(time.time())

    parsed = await enrich_auth_config(
        provider_type=provider_type,
        auth_config=parsed,
        token_response=token,
        access_token=access_token,
        proxy_config=proxy_config,
    )

    key.auth_config = crypto_service.encrypt(json.dumps(parsed))
    db.commit()

    return CompleteOAuthResponse(
        provider_type=provider_type,
        expires_at=expires_at,
        has_refresh_token=bool(parsed.get("refresh_token")),
    )
