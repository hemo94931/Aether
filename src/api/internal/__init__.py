from fastapi import APIRouter

from .gateway import router as gateway_router
from .hub import router as hub_router

router = APIRouter()
router.include_router(hub_router)
router.include_router(gateway_router)

__all__ = ["router"]
