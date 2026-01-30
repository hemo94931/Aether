"""
Video Generation API 路由
"""

from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Depends, Request
from sqlalchemy.orm import Session

from src.api.base.pipeline import ApiRequestPipeline
from src.api.handlers.gemini.video_adapter import GeminiVeoAdapter
from src.api.handlers.openai.video_adapter import OpenAIVideoAdapter
from src.database import get_db

router = APIRouter(tags=["Video Generation"])
pipeline = ApiRequestPipeline()


# -------------------- OpenAI Sora compatible --------------------


@router.post("/v1/videos")
async def create_video_sora(http_request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = OpenAIVideoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
    )


@router.get("/v1/videos/{task_id}")
async def get_video_task_sora(
    task_id: str, http_request: Request, db: Session = Depends(get_db)
) -> Any:
    adapter = OpenAIVideoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
        path_params={"task_id": task_id},
    )


@router.get("/v1/videos")
async def list_video_tasks_sora(http_request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = OpenAIVideoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
    )


@router.delete("/v1/videos/{task_id}")
async def cancel_video_task_sora(
    task_id: str, http_request: Request, db: Session = Depends(get_db)
) -> Any:
    adapter = OpenAIVideoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
        path_params={"task_id": task_id, "action": "cancel"},
    )


@router.get("/v1/videos/{task_id}/content")
async def download_video_content_sora(
    task_id: str, http_request: Request, db: Session = Depends(get_db)
) -> Any:
    adapter = OpenAIVideoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
        path_params={"task_id": task_id},
    )


# -------------------- Gemini Veo compatible --------------------


@router.post("/v1beta/models/{model}:predictLongRunning")
async def create_video_veo(model: str, http_request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = GeminiVeoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
        path_params={"model": model},
    )


@router.get("/v1beta/operations/{operation_id}")
async def get_video_veo(
    operation_id: str, http_request: Request, db: Session = Depends(get_db)
) -> Any:
    adapter = GeminiVeoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
        path_params={"task_id": operation_id},
    )


@router.get("/v1beta/operations")
async def list_video_tasks_veo(http_request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = GeminiVeoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
    )


@router.post("/v1beta/operations/{operation_id}:cancel")
async def cancel_video_veo(
    operation_id: str, http_request: Request, db: Session = Depends(get_db)
) -> Any:
    adapter = GeminiVeoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
        path_params={"task_id": operation_id, "action": "cancel"},
    )


@router.get("/v1beta/operations/{operation_id}/content")
async def download_video_content_veo(
    operation_id: str, http_request: Request, db: Session = Depends(get_db)
) -> Any:
    adapter = GeminiVeoAdapter()
    return await pipeline.run(
        adapter=adapter,
        http_request=http_request,
        db=db,
        mode=adapter.mode,
        api_format_hint=adapter.allowed_api_formats[0],
        path_params={"task_id": operation_id},
    )
