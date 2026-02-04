from __future__ import annotations

import asyncio
from typing import Any

import httpx
import jwt

from src.clients.http_client import HTTPClientPool, build_proxy_url
from src.core.logger import logger


_ANTHROPIC_TOKEN_URL = "https://console.anthropic.com/v1/oauth/token"
_GOOGLE_USERINFO_URL = "https://www.googleapis.com/oauth2/v1/userinfo?alt=json"


def _coerce_proxy_url(proxy_config: dict[str, Any] | None) -> str | None:
    if not proxy_config:
        return None
    try:
        if not proxy_config.get("enabled", True):
            return None
        return build_proxy_url(proxy_config)
    except Exception:
        return None


async def _httpx_post(
    url: str,
    *,
    headers: dict[str, str] | None,
    data: Any,
    json_body: Any,
    proxy_config: dict[str, Any] | None,
    timeout_seconds: float,
) -> httpx.Response:
    client = await HTTPClientPool.get_proxy_client(proxy_config)
    return await client.post(
        url,
        headers=headers,
        data=data,
        json=json_body,
        timeout=timeout_seconds,
    )


def _tls_client_post_sync(
    url: str,
    *,
    headers: dict[str, str] | None,
    data: Any,
    json_body: Any,
    proxy_url: str | None,
    timeout_seconds: float,
) -> tuple[int, dict[str, str], str]:
    # tls-client is optional at runtime; import only when needed.
    import tls_client  # type: ignore

    session = tls_client.Session(
        client_identifier="firefox_120",
        random_tls_extension_order=True,
    )

    if proxy_url:
        session.proxies = {"http": proxy_url, "https": proxy_url}

    # tls-client uses a requests-like API.
    resp = session.post(
        url,
        headers=headers or {},
        data=data,
        json=json_body,
        timeout_seconds=timeout_seconds,
    )

    # Normalize output
    status_code = int(getattr(resp, "status_code", 0))
    text = str(getattr(resp, "text", ""))
    resp_headers = dict(getattr(resp, "headers", {}) or {})
    return status_code, resp_headers, text


async def post_oauth_token(
    *,
    provider_type: str,
    token_url: str,
    headers: dict[str, str] | None,
    data: Any = None,
    json_body: Any = None,
    proxy_config: dict[str, Any] | None = None,
    timeout_seconds: float = 30.0,
) -> httpx.Response:
    """POST to token endpoint.

    Claude Code + Anthropic token URL will try tls-client (Firefox TLS fingerprint) first.
    If tls-client is unavailable or fails, fall back to httpx.

    IMPORTANT: Never log secrets (tokens, secrets). This function only logs generic errors.
    """

    if provider_type == "claude_code" and token_url == _ANTHROPIC_TOKEN_URL:
        proxy_url = _coerce_proxy_url(proxy_config)
        try:
            status_code, resp_headers, text = await asyncio.to_thread(
                _tls_client_post_sync,
                token_url,
                headers=headers,
                data=data,
                json_body=json_body,
                proxy_url=proxy_url,
                timeout_seconds=timeout_seconds,
            )
            return httpx.Response(
                status_code=status_code,
                headers=resp_headers,
                content=text.encode("utf-8", errors="replace"),
                request=httpx.Request("POST", token_url),
            )
        except Exception as e:
            logger.warning(
                "Claude OAuth token request via tls-client failed; fallback to httpx. err={!r}",
                e,
            )

    return await _httpx_post(
        token_url,
        headers=headers,
        data=data,
        json_body=json_body,
        proxy_config=proxy_config,
        timeout_seconds=timeout_seconds,
    )


def parse_codex_id_token(id_token: str | None) -> tuple[str | None, str | None]:
    """Parse Codex id_token WITHOUT signature verification.

    Extract:
    - email: claim `email`
    - account_id: claim `https://api.openai.com/auth`.`chatgpt_account_id`

    Return (email, account_id). On any failure returns (None, None).
    """

    if not id_token:
        return (None, None)
    try:
        claims = jwt.decode(
            id_token,
            options={
                "verify_signature": False,
                "verify_aud": False,
            },
        )
        email = claims.get("email")
        auth_info = claims.get("https://api.openai.com/auth") or {}
        account_id = None
        if isinstance(auth_info, dict):
            account_id = auth_info.get("chatgpt_account_id")
        return (
            str(email) if isinstance(email, str) and email else None,
            str(account_id) if isinstance(account_id, str) and account_id else None,
        )
    except Exception:
        return (None, None)


async def fetch_google_email(
    access_token: str,
    *,
    proxy_config: dict[str, Any] | None = None,
    timeout_seconds: float = 10.0,
) -> str | None:
    if not access_token:
        return None

    client = await HTTPClientPool.get_proxy_client(proxy_config)
    try:
        resp = await client.get(
            _GOOGLE_USERINFO_URL,
            headers={"Authorization": f"Bearer {access_token}", "Accept": "application/json"},
            timeout=timeout_seconds,
        )
        if resp.status_code < 200 or resp.status_code >= 300:
            return None
        data = resp.json()
        email = data.get("email")
        if isinstance(email, str) and email:
            return email
        return None
    except Exception:
        return None


def extract_claude_email_from_token_response(token: dict[str, Any]) -> str | None:
    # CLIProxyAPI expects: { account: { email_address: ... } }
    try:
        account = token.get("account")
        if isinstance(account, dict):
            email = account.get("email_address")
            if isinstance(email, str) and email:
                return email
    except Exception:
        pass
    return None


async def enrich_auth_config(
    *,
    provider_type: str,
    auth_config: dict[str, Any],
    token_response: dict[str, Any],
    access_token: str,
    proxy_config: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Enrich auth_config with non-secret metadata (email/account_id).

    - Claude Code: email from token response (if present)
    - Codex: parse id_token -> email/account_id
    - Gemini/Antigravity: call Google userinfo -> email

    id_token is not persisted.
    """

    # Claude
    if provider_type == "claude_code":
        email = extract_claude_email_from_token_response(token_response)
        if email:
            auth_config["email"] = email
        return auth_config

    # Codex
    if provider_type == "codex":
        id_token = token_response.get("id_token")
        email, account_id = parse_codex_id_token(str(id_token) if id_token else None)
        if email:
            auth_config["email"] = email
        if account_id:
            auth_config["account_id"] = account_id
        return auth_config

    # Gemini family (gemini_cli / antigravity)
    if provider_type in {"gemini_cli", "antigravity"}:
        # Only fetch if missing to reduce overhead
        if not auth_config.get("email"):
            email = await fetch_google_email(
                access_token,
                proxy_config=proxy_config,
                timeout_seconds=10.0,
            )
            if email:
                auth_config["email"] = email
        return auth_config

    return auth_config
