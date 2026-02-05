"""Routes for authenticated user self-service APIs."""

from fastapi import APIRouter

from .routes import router as me_router

router = APIRouter()
router.include_router(me_router)

# 注意：management_tokens_router 已迁移到模块系统，由 ModuleRegistry 动态注册
# 当 MANAGEMENT_TOKENS_AVAILABLE=true 时注册

__all__ = ["router"]
