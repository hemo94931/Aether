from __future__ import annotations

import json
import secrets
import time
from dataclasses import dataclass
from typing import Any, Awaitable, Optional, cast

from redis.asyncio import Redis


OAUTH_STATE_TTL_SECONDS = 600
OAUTH_STATE_KEY_PREFIX = "oauth_state:"


CONSUME_STATE_SCRIPT = r"""
local value = redis.call("GET", KEYS[1])
if value then
    redis.call("DEL", KEYS[1])
end
return value
"""


@dataclass(frozen=True)
class OAuthStateData:
    nonce: str
    provider_type: str
    action: str  # "login" | "bind"
    user_id: Optional[str]
    created_at: int

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "OAuthStateData":
        return cls(
            nonce=str(data.get("nonce") or ""),
            provider_type=str(data.get("provider_type") or ""),
            action=str(data.get("action") or ""),
            user_id=data.get("user_id"),
            created_at=int(data.get("created_at") or 0),
        )


def _state_key(nonce: str) -> str:
    return f"{OAUTH_STATE_KEY_PREFIX}{nonce}"


async def create_oauth_state(
    redis: Redis, *, provider_type: str, action: str, user_id: Optional[str] = None
) -> str:
    nonce = secrets.token_urlsafe(24)
    data = {
        "nonce": nonce,
        "provider_type": provider_type,
        "action": action,
        "user_id": user_id,
        "created_at": int(time.time()),
    }
    await redis.setex(_state_key(nonce), OAUTH_STATE_TTL_SECONDS, json.dumps(data))
    return nonce


async def consume_oauth_state(redis: Redis, nonce: str) -> Optional[OAuthStateData]:
    if not nonce:
        return None

    key = _state_key(nonce)
    # redis-py 的类型标注在 sync/async 之间会出现 Union；这里明确按 async 处理。
    raw = await cast(Awaitable[Optional[str]], redis.eval(CONSUME_STATE_SCRIPT, 1, key))
    if not raw:
        return None

    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError:
        return None

    return OAuthStateData.from_dict(parsed)
