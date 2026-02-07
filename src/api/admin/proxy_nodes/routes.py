"""管理员代理节点（ProxyNode）管理端点

用于 aether-proxy 在 VPS 上注册、心跳、注销节点，以及管理员查看/删除节点记录。
"""

from __future__ import annotations

import ipaddress
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any

from fastapi import APIRouter, Depends, Query, Request
from pydantic import BaseModel, Field, ValidationError, field_validator
from sqlalchemy.orm import Session

from src.api.base.admin_adapter import AdminApiAdapter
from src.api.base.context import ApiRequestContext
from src.api.base.pipeline import ApiRequestPipeline
from src.core.exceptions import InvalidRequestException, NotFoundException
from src.database import get_db
from src.models.database import ProxyNode, ProxyNodeStatus, SystemConfig

router = APIRouter(prefix="/api/admin/proxy-nodes", tags=["Admin - Proxy Nodes"])
pipeline = ApiRequestPipeline()


def _mask_password(password: str | None) -> str | None:
    """脱敏密码，仅显示前2位和后2位"""
    if not password:
        return None
    if len(password) <= 4:
        return "****"
    return password[:2] + "****" + password[-2:]


def _node_to_dict(node: ProxyNode) -> dict[str, Any]:
    d = {
        "id": node.id,
        "name": node.name,
        "ip": node.ip,
        "port": node.port,
        "region": node.region,
        "status": node.status.value if node.status else None,
        "is_manual": bool(node.is_manual),
        "registered_by": node.registered_by,
        "last_heartbeat_at": node.last_heartbeat_at,
        "heartbeat_interval": node.heartbeat_interval,
        "active_connections": node.active_connections,
        "total_requests": node.total_requests,
        "avg_latency_ms": node.avg_latency_ms,
        "created_at": node.created_at,
        "updated_at": node.updated_at,
    }
    # 手动节点附带代理配置（密码脱敏）
    if node.is_manual:
        d["proxy_url"] = node.proxy_url
        d["proxy_username"] = node.proxy_username
        d["proxy_password"] = _mask_password(node.proxy_password)
    return d


class ProxyNodeRegisterRequest(BaseModel):
    name: str = Field(..., min_length=1, max_length=100, description="节点名")
    ip: str = Field(..., description="公网 IP（IPv4/IPv6）")
    port: int = Field(..., ge=1, le=65535, description="代理端口")
    region: str | None = Field(None, max_length=100, description="区域标签")
    heartbeat_interval: int = Field(30, ge=5, le=600, description="心跳间隔（秒）")

    # 指标（可选）
    active_connections: int | None = Field(None, ge=0, description="当前活跃连接数")
    total_requests: int | None = Field(None, ge=0, description="累计请求数")
    avg_latency_ms: float | None = Field(None, ge=0, description="平均延迟（毫秒）")

    @field_validator("ip")
    @classmethod
    def validate_ip(cls, v: str) -> str:
        v = v.strip()
        try:
            ipaddress.ip_address(v)
        except ValueError as exc:
            raise ValueError("ip 必须是合法的 IPv4/IPv6 地址") from exc
        return v


class ProxyNodeHeartbeatRequest(BaseModel):
    node_id: str = Field(..., min_length=1, max_length=36, description="节点 ID")
    heartbeat_interval: int | None = Field(None, ge=5, le=600, description="心跳间隔（秒）")

    active_connections: int | None = Field(None, ge=0, description="当前活跃连接数")
    total_requests: int | None = Field(None, ge=0, description="累计请求数")
    avg_latency_ms: float | None = Field(None, ge=0, description="平均延迟（毫秒）")


class ProxyNodeUnregisterRequest(BaseModel):
    node_id: str = Field(..., min_length=1, max_length=36, description="节点 ID")


class ManualProxyNodeCreateRequest(BaseModel):
    """手动创建代理节点"""

    name: str = Field(..., min_length=1, max_length=100, description="节点名")
    proxy_url: str = Field(
        ..., min_length=1, max_length=500, description="代理 URL (http/https/socks5)"
    )
    username: str | None = Field(None, max_length=255, description="代理用户名")
    password: str | None = Field(None, max_length=500, description="代理密码")
    region: str | None = Field(None, max_length=100, description="区域标签")

    @field_validator("proxy_url")
    @classmethod
    def validate_proxy_url(cls, v: str) -> str:
        import re
        from urllib.parse import urlparse

        v = v.strip()
        if not re.match(r"^(http|https|socks5)://", v, re.IGNORECASE):
            raise ValueError("代理 URL 必须以 http://, https:// 或 socks5:// 开头")
        parsed = urlparse(v)
        if not parsed.hostname:
            raise ValueError("代理 URL 必须包含有效的 host")
        return v


class ManualProxyNodeUpdateRequest(BaseModel):
    """更新手动代理节点"""

    name: str | None = Field(None, min_length=1, max_length=100, description="节点名")
    proxy_url: str | None = Field(None, min_length=1, max_length=500, description="代理 URL")
    username: str | None = Field(None, max_length=255, description="代理用户名")
    password: str | None = Field(None, max_length=500, description="代理密码")
    region: str | None = Field(None, max_length=100, description="区域标签")

    @field_validator("proxy_url")
    @classmethod
    def validate_proxy_url(cls, v: str | None) -> str | None:
        if v is None:
            return None
        import re
        from urllib.parse import urlparse

        v = v.strip()
        if not re.match(r"^(http|https|socks5)://", v, re.IGNORECASE):
            raise ValueError("代理 URL 必须以 http://, https:// 或 socks5:// 开头")
        parsed = urlparse(v)
        if not parsed.hostname:
            raise ValueError("代理 URL 必须包含有效的 host")
        return v


@router.post("/register")
async def register_proxy_node(request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = AdminRegisterProxyNodeAdapter()
    return await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)


@router.post("/heartbeat")
async def heartbeat_proxy_node(request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = AdminHeartbeatProxyNodeAdapter()
    return await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)


@router.post("/unregister")
async def unregister_proxy_node(request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = AdminUnregisterProxyNodeAdapter()
    return await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)


@router.get("")
async def list_proxy_nodes(
    request: Request,
    status: str | None = Query(None, description="按状态筛选：online/unhealthy/offline"),
    skip: int = Query(0, ge=0),
    limit: int = Query(100, ge=1, le=1000),
    db: Session = Depends(get_db),
) -> Any:
    adapter = AdminListProxyNodesAdapter(status=status, skip=skip, limit=limit)
    return await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)


@router.post("/manual")
async def create_manual_proxy_node(request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = AdminCreateManualProxyNodeAdapter()
    return await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)


@router.patch("/{node_id}")
async def update_manual_proxy_node(
    node_id: str, request: Request, db: Session = Depends(get_db)
) -> Any:
    adapter = AdminUpdateManualProxyNodeAdapter(node_id=node_id)
    return await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)


@router.delete("/{node_id}")
async def delete_proxy_node(node_id: str, request: Request, db: Session = Depends(get_db)) -> Any:
    adapter = AdminDeleteProxyNodeAdapter(node_id=node_id)
    return await pipeline.run(adapter=adapter, http_request=request, db=db, mode=adapter.mode)


def _format_validation_error(exc: ValidationError) -> str:
    parts: list[str] = []
    for err in exc.errors():
        field = " -> ".join(str(x) for x in err.get("loc", []))
        msg = str(err.get("msg", "invalid"))
        parts.append(f"{field}: {msg}")
    return "; ".join(parts) or "输入验证失败"


@dataclass
class AdminRegisterProxyNodeAdapter(AdminApiAdapter):
    name: str = "admin_register_proxy_node"

    async def handle(self, context: ApiRequestContext) -> Any:
        payload = context.ensure_json_body()
        try:
            req = ProxyNodeRegisterRequest.model_validate(payload)
        except ValidationError as exc:
            raise InvalidRequestException("输入验证失败: " + _format_validation_error(exc))

        now = datetime.now(timezone.utc)

        node = (
            context.db.query(ProxyNode)
            .filter(ProxyNode.ip == req.ip, ProxyNode.port == req.port)
            .first()
        )
        if node:
            node.name = req.name
            node.region = req.region
            node.status = ProxyNodeStatus.ONLINE
            node.last_heartbeat_at = now
            node.heartbeat_interval = req.heartbeat_interval
            if req.active_connections is not None:
                node.active_connections = req.active_connections
            if req.total_requests is not None:
                node.total_requests = req.total_requests
            if req.avg_latency_ms is not None:
                node.avg_latency_ms = req.avg_latency_ms
        else:
            node = ProxyNode(
                id=str(uuid.uuid4()),
                name=req.name,
                ip=req.ip,
                port=req.port,
                region=req.region,
                status=ProxyNodeStatus.ONLINE,
                registered_by=context.user.id if context.user else None,
                last_heartbeat_at=now,
                heartbeat_interval=req.heartbeat_interval,
                active_connections=req.active_connections or 0,
                total_requests=req.total_requests or 0,
                avg_latency_ms=req.avg_latency_ms,
                created_at=now,
                updated_at=now,
            )
            context.db.add(node)

        context.db.commit()
        context.db.refresh(node)

        context.add_audit_metadata(
            action="proxy_node_register",
            proxy_node_id=node.id,
            proxy_node_ip=node.ip,
            proxy_node_port=node.port,
        )

        return {"node_id": node.id, "node": _node_to_dict(node)}


@dataclass
class AdminHeartbeatProxyNodeAdapter(AdminApiAdapter):
    name: str = "admin_heartbeat_proxy_node"

    async def handle(self, context: ApiRequestContext) -> Any:
        payload = context.ensure_json_body()
        try:
            req = ProxyNodeHeartbeatRequest.model_validate(payload)
        except ValidationError as exc:
            raise InvalidRequestException("输入验证失败: " + _format_validation_error(exc))

        node = context.db.query(ProxyNode).filter(ProxyNode.id == req.node_id).first()
        if not node:
            raise NotFoundException(f"ProxyNode {req.node_id} 不存在", "proxy_node")

        now = datetime.now(timezone.utc)
        node.status = ProxyNodeStatus.ONLINE
        node.last_heartbeat_at = now
        if req.heartbeat_interval is not None:
            node.heartbeat_interval = req.heartbeat_interval
        if req.active_connections is not None:
            node.active_connections = req.active_connections
        if req.total_requests is not None:
            node.total_requests = req.total_requests
        if req.avg_latency_ms is not None:
            node.avg_latency_ms = req.avg_latency_ms

        context.db.commit()
        context.db.refresh(node)

        context.add_audit_metadata(
            action="proxy_node_heartbeat",
            proxy_node_id=node.id,
        )

        return {"message": "heartbeat ok", "node": _node_to_dict(node)}


@dataclass
class AdminUnregisterProxyNodeAdapter(AdminApiAdapter):
    name: str = "admin_unregister_proxy_node"

    async def handle(self, context: ApiRequestContext) -> Any:
        payload = context.ensure_json_body()
        try:
            req = ProxyNodeUnregisterRequest.model_validate(payload)
        except ValidationError as exc:
            raise InvalidRequestException("输入验证失败: " + _format_validation_error(exc))

        node = context.db.query(ProxyNode).filter(ProxyNode.id == req.node_id).first()
        if not node:
            raise NotFoundException(f"ProxyNode {req.node_id} 不存在", "proxy_node")

        node.status = ProxyNodeStatus.OFFLINE
        node.updated_at = datetime.now(timezone.utc)
        context.db.commit()

        context.add_audit_metadata(
            action="proxy_node_unregister",
            proxy_node_id=node.id,
        )

        return {"message": "unregistered", "node_id": node.id}


@dataclass
class AdminListProxyNodesAdapter(AdminApiAdapter):
    name: str = "admin_list_proxy_nodes"
    status: str | None = None
    skip: int = 0
    limit: int = 100

    async def handle(self, context: ApiRequestContext) -> Any:
        query = context.db.query(ProxyNode)
        if self.status:
            normalized = self.status.strip().lower()
            allowed = {"online", "unhealthy", "offline"}
            if normalized not in allowed:
                raise InvalidRequestException(f"status 必须是以下之一: {sorted(allowed)}", "status")
            query = query.filter(ProxyNode.status == ProxyNodeStatus(normalized))

        total = query.count()
        nodes = (
            query.order_by(ProxyNode.updated_at.desc()).offset(self.skip).limit(self.limit).all()
        )
        return {
            "items": [_node_to_dict(n) for n in nodes],
            "total": total,
            "skip": self.skip,
            "limit": self.limit,
        }


@dataclass
class AdminDeleteProxyNodeAdapter(AdminApiAdapter):
    name: str = "admin_delete_proxy_node"
    node_id: str = ""

    async def handle(self, context: ApiRequestContext) -> Any:
        node = context.db.query(ProxyNode).filter(ProxyNode.id == self.node_id).first()
        if not node:
            raise NotFoundException(f"ProxyNode {self.node_id} 不存在", "proxy_node")

        context.add_audit_metadata(
            action="proxy_node_delete",
            proxy_node_id=node.id,
            proxy_node_ip=node.ip,
            proxy_node_port=node.port,
        )

        # 若该节点是系统默认代理，自动清除引用
        was_system_proxy = False
        sys_cfg = (
            context.db.query(SystemConfig)
            .filter(SystemConfig.key == "system_proxy_node_id")
            .first()
        )
        if sys_cfg and sys_cfg.value == self.node_id:
            sys_cfg.value = None
            was_system_proxy = True

        context.db.delete(node)
        context.db.commit()

        if was_system_proxy:
            from src.clients.http_client import invalidate_system_proxy_cache

            invalidate_system_proxy_cache()

        msg = "deleted"
        if was_system_proxy:
            msg = "deleted, system default proxy cleared"
        return {"message": msg, "node_id": self.node_id, "cleared_system_proxy": was_system_proxy}


def _parse_host_port(proxy_url: str) -> tuple[str, int]:
    """从代理 URL 中解析 host 和 port（含协议前缀，避免唯一约束冲突）"""
    from urllib.parse import urlparse

    parsed = urlparse(proxy_url)
    host = parsed.hostname or "manual"
    default_ports = {"https": 443, "socks5": 1080}
    port = parsed.port or default_ports.get((parsed.scheme or "").lower(), 80)
    # 添加协议前缀区分同 host:port 不同协议的场景
    scheme = (parsed.scheme or "http").lower()
    if scheme != "http":
        host = f"{scheme}://{host}"
    return host, port


@dataclass
class AdminCreateManualProxyNodeAdapter(AdminApiAdapter):
    name: str = "admin_create_manual_proxy_node"

    async def handle(self, context: ApiRequestContext) -> Any:
        payload = context.ensure_json_body()
        try:
            req = ManualProxyNodeCreateRequest.model_validate(payload)
        except ValidationError as exc:
            raise InvalidRequestException("输入验证失败: " + _format_validation_error(exc))

        host, port = _parse_host_port(req.proxy_url)
        now = datetime.now(timezone.utc)

        node = ProxyNode(
            id=str(uuid.uuid4()),
            name=req.name,
            ip=host,
            port=port,
            region=req.region,
            is_manual=True,
            proxy_url=req.proxy_url,
            proxy_username=req.username,
            proxy_password=req.password,
            status=ProxyNodeStatus.ONLINE,
            registered_by=context.user.id if context.user else None,
            last_heartbeat_at=None,
            heartbeat_interval=0,
            active_connections=0,
            total_requests=0,
            avg_latency_ms=None,
            created_at=now,
            updated_at=now,
        )
        # 检查是否已存在同地址的节点
        existing = (
            context.db.query(ProxyNode).filter(ProxyNode.ip == host, ProxyNode.port == port).first()
        )
        if existing:
            raise InvalidRequestException(
                f"已存在相同地址的代理节点: {existing.name} ({existing.ip}:{existing.port})"
            )

        context.db.add(node)
        context.db.commit()
        context.db.refresh(node)

        context.add_audit_metadata(
            action="proxy_node_manual_create",
            proxy_node_id=node.id,
        )

        return {"node_id": node.id, "node": _node_to_dict(node)}


@dataclass
class AdminUpdateManualProxyNodeAdapter(AdminApiAdapter):
    name: str = "admin_update_manual_proxy_node"
    node_id: str = ""

    async def handle(self, context: ApiRequestContext) -> Any:
        node = context.db.query(ProxyNode).filter(ProxyNode.id == self.node_id).first()
        if not node:
            raise NotFoundException(f"ProxyNode {self.node_id} 不存在", "proxy_node")
        if not node.is_manual:
            raise InvalidRequestException("只能编辑手动添加的代理节点")

        payload = context.ensure_json_body()
        try:
            req = ManualProxyNodeUpdateRequest.model_validate(payload)
        except ValidationError as exc:
            raise InvalidRequestException("输入验证失败: " + _format_validation_error(exc))

        if req.name is not None:
            node.name = req.name
        if req.proxy_url is not None:
            node.proxy_url = req.proxy_url
            host, port = _parse_host_port(req.proxy_url)
            # 检查新地址是否与其他节点冲突
            existing = (
                context.db.query(ProxyNode)
                .filter(ProxyNode.ip == host, ProxyNode.port == port, ProxyNode.id != node.id)
                .first()
            )
            if existing:
                raise InvalidRequestException(
                    f"已存在相同地址的代理节点: {existing.name} ({existing.ip}:{existing.port})"
                )
            node.ip = host
            node.port = port
        if req.username is not None:
            node.proxy_username = req.username
        # password: None=不发送(保留原值), ""=清空, 非空=更新
        if req.password is not None:
            node.proxy_password = req.password or None
        if req.region is not None:
            node.region = req.region

        node.updated_at = datetime.now(timezone.utc)
        context.db.commit()
        context.db.refresh(node)

        context.add_audit_metadata(
            action="proxy_node_manual_update",
            proxy_node_id=node.id,
        )

        return {"node_id": node.id, "node": _node_to_dict(node)}
