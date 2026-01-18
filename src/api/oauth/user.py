"""OAuth 用户端点（需登录）。"""

from __future__ import annotations

from typing import Any, cast

from fastapi import APIRouter, Depends, Request, status
from sqlalchemy.orm import Session
from starlette.responses import RedirectResponse

from src.api.base.authenticated_adapter import AuthenticatedApiAdapter
from src.api.base.context import ApiRequestContext
from src.api.base.pipeline import ApiRequestPipeline
from src.database import get_db
from src.services.auth.oauth.service import OAuthService

router = APIRouter(prefix="/api/user/oauth", tags=["User - OAuth"])
pipeline = ApiRequestPipeline()


@router.get("/bindable-providers")
async def list_bindable_providers(request: Request, db: Session = Depends(get_db)) -> dict[str, Any]:
    adapter = ListBindableProvidersAdapter()
    result = await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)
    return cast(dict[str, Any], result)


@router.get("/links")
async def list_my_oauth_links(request: Request, db: Session = Depends(get_db)) -> dict[str, Any]:
    adapter = ListMyOAuthLinksAdapter()
    result = await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)
    return cast(dict[str, Any], result)


@router.get("/{provider_type}/bind")
async def bind_oauth_provider(
    provider_type: str, request: Request, db: Session = Depends(get_db)
) -> RedirectResponse:
    adapter = BindOAuthProviderAdapter(provider_type=provider_type)
    result = await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)
    return cast(RedirectResponse, result)


@router.delete("/{provider_type}")
async def unbind_oauth_provider(
    provider_type: str, request: Request, db: Session = Depends(get_db)
) -> dict[str, Any]:
    adapter = UnbindOAuthProviderAdapter(provider_type=provider_type)
    result = await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)
    return cast(dict[str, Any], result)


class ListBindableProvidersAdapter(AuthenticatedApiAdapter):
    async def handle(self, context: ApiRequestContext) -> dict[str, Any]:  # type: ignore[override]
        assert context.user is not None
        providers = await OAuthService.list_bindable_providers(context.db, context.user)
        return {"providers": providers}


class ListMyOAuthLinksAdapter(AuthenticatedApiAdapter):
    async def handle(self, context: ApiRequestContext) -> dict[str, Any]:  # type: ignore[override]
        assert context.user is not None
        links = await OAuthService.list_user_links(context.db, context.user)
        return {"links": links}


class BindOAuthProviderAdapter(AuthenticatedApiAdapter):
    def __init__(self, provider_type: str):
        self.provider_type = provider_type

    async def handle(self, context: ApiRequestContext) -> RedirectResponse:  # type: ignore[override]
        assert context.user is not None
        url = await OAuthService.build_bind_authorize_url(context.db, context.user, self.provider_type)
        return RedirectResponse(url=url, status_code=status.HTTP_302_FOUND)


class UnbindOAuthProviderAdapter(AuthenticatedApiAdapter):
    def __init__(self, provider_type: str):
        self.provider_type = provider_type

    async def handle(self, context: ApiRequestContext) -> dict[str, Any]:  # type: ignore[override]
        assert context.user is not None
        await OAuthService.unbind_provider(context.db, context.user, self.provider_type)
        return {"message": "解绑成功"}
