"""
全局HTTP客户端池管理
避免每次请求都创建新的AsyncClient,提高性能

性能优化说明：
1. 默认客户端：无代理场景，全局复用单一客户端
2. 代理客户端缓存：相同代理配置复用同一客户端，避免重复创建
3. 连接池复用：Keep-alive 连接减少 TCP 握手开销
"""

from __future__ import annotations

import asyncio
import hashlib
import hmac
import time
from contextlib import asynccontextmanager
from typing import Any
from urllib.parse import quote, urlparse

import httpx

from src.config import config
from src.core.exceptions import ProxyNodeUnavailableError
from src.core.logger import logger
from src.utils.ssl_utils import get_ssl_context

# 模块级锁，避免类属性延迟初始化的竞态条件
_proxy_clients_lock = asyncio.Lock()
_default_client_lock = asyncio.Lock()

# ProxyNode 信息缓存（降低高频 DB 查询开销）
_proxy_node_cache: dict[str, tuple[dict[str, Any] | None, float]] = {}
_PROXY_NODE_CACHE_TTL_SECONDS = 60.0
_PROXY_NODE_CACHE_MAX_SIZE = 256


def _get_proxy_node_info(node_id: str) -> dict[str, Any] | None:
    """
    读取 ProxyNode 信息（带内存 TTL 缓存）

    Returns:
        aether-proxy 节点: {"ip": str, "port": int}
        手动节点: {"is_manual": True, "proxy_url": str, "username": str|None, "password": str|None}
        不存在/非在线: None
    """
    now = time.time()
    cached = _proxy_node_cache.get(node_id)
    if cached:
        value, expires_at = cached
        if now < expires_at:
            return value

    # 防止无效 node_id 导致缓存无限膨胀
    if len(_proxy_node_cache) >= _PROXY_NODE_CACHE_MAX_SIZE:
        _proxy_node_cache.clear()

    from src.database import create_session
    from src.models.database import ProxyNode, ProxyNodeStatus

    db = create_session()
    try:
        node = db.query(ProxyNode).filter(ProxyNode.id == node_id).first()
        if not node or node.status != ProxyNodeStatus.ONLINE:
            _proxy_node_cache[node_id] = (None, now + _PROXY_NODE_CACHE_TTL_SECONDS)
            return None

        if node.is_manual:
            value: dict[str, Any] = {
                "is_manual": True,
                "proxy_url": node.proxy_url,
                "username": node.proxy_username,
                "password": node.proxy_password,
            }
        else:
            value = {"ip": node.ip, "port": node.port}

        _proxy_node_cache[node_id] = (value, now + _PROXY_NODE_CACHE_TTL_SECONDS)
        return value
    finally:
        db.close()


def _build_hmac_proxy_url(ip: str, port: int, node_id: str) -> str:
    """
    构建带 HMAC BasicAuth 的 httpx proxy URL

    格式: http://hmac:{timestamp}.{signature}@{ip}:{port}
    signature = HMAC-SHA256(PROXY_HMAC_KEY, "{timestamp}\\n{node_id}") 的 hex
    """
    if not config.proxy_hmac_key:
        raise ProxyNodeUnavailableError(
            "PROXY_HMAC_KEY 未配置，无法使用 ProxyNode 代理", node_id=node_id
        )

    timestamp = str(int(time.time()))
    payload = f"{timestamp}\n{node_id}".encode("utf-8")
    signature = hmac.new(
        config.proxy_hmac_key.encode("utf-8"),
        payload,
        hashlib.sha256,
    ).hexdigest()

    host = f"[{ip}]" if ":" in ip else ip
    return f"http://hmac:{timestamp}.{signature}@{host}:{int(port)}"


# 系统默认代理缓存
_system_proxy_cache: tuple[dict[str, Any] | None, float] | None = None
_SYSTEM_PROXY_CACHE_TTL = 60.0


def invalidate_system_proxy_cache() -> None:
    """手动失效系统代理缓存（在删除节点等操作后调用）"""
    global _system_proxy_cache
    _system_proxy_cache = None


def get_system_proxy_config() -> dict[str, Any] | None:
    """
    获取系统默认代理配置（带 TTL 缓存）

    从 system_configs 表中读取 system_proxy_node_id。
    返回 {"node_id": "...", "enabled": True} 或 None。
    """
    global _system_proxy_cache
    now = time.time()
    if _system_proxy_cache:
        value, expires_at = _system_proxy_cache
        if now < expires_at:
            return value

    from src.database import create_session
    from src.services.system.config import SystemConfigService

    db = create_session()
    try:
        node_id = SystemConfigService.get_config(db, "system_proxy_node_id")
        if node_id and isinstance(node_id, str) and node_id.strip():
            result: dict[str, Any] | None = {"node_id": node_id.strip(), "enabled": True}
        else:
            result = None
        _system_proxy_cache = (result, now + _SYSTEM_PROXY_CACHE_TTL)
        return result
    except Exception:
        _system_proxy_cache = (None, now + _SYSTEM_PROXY_CACHE_TTL)
        return None
    finally:
        db.close()


def resolve_ops_proxy(connector_config: dict[str, Any] | None) -> str | None:
    """
    从 ops connector.config 中解析代理 URL（含系统默认回退）

    优先级：
    1. connector_config.proxy_node_id（新格式）
    2. connector_config.proxy（旧格式 URL 字符串）
    3. 系统默认代理节点

    Args:
        connector_config: connector 的 config 字典

    Returns:
        代理 URL 字符串，或 None
    """
    if connector_config:
        # 新格式：proxy_node_id → 通过 build_proxy_url 解析
        node_id = connector_config.get("proxy_node_id")
        if isinstance(node_id, str) and node_id.strip():
            try:
                return build_proxy_url({"node_id": node_id.strip(), "enabled": True})
            except Exception:
                return None

        # 旧格式：直接返回 proxy URL 字符串
        proxy = connector_config.get("proxy")
        if isinstance(proxy, str) and proxy.strip():
            return proxy

    # 回退：系统默认代理
    system_proxy = get_system_proxy_config()
    if system_proxy:
        try:
            return build_proxy_url(system_proxy)
        except Exception:
            return None

    return None


def _compute_proxy_cache_key(proxy_config: dict[str, Any] | None) -> str:
    """
    计算代理配置的缓存键

    Args:
        proxy_config: 代理配置字典

    Returns:
        缓存键字符串，无代理时返回 "__no_proxy__"
    """
    if not proxy_config:
        return "__no_proxy__"

    # enabled=False 时视为无代理（兼容旧数据）
    if not proxy_config.get("enabled", True):
        return "__no_proxy__"

    # ProxyNode 模式：基于 node_id + 时间桶缓存，避免签名随时间变化导致 cache key 爆炸
    node_id = proxy_config.get("node_id")
    if isinstance(node_id, str) and node_id.strip():
        time_bucket = int(time.time() / 120)  # 120 秒一个桶
        return f"proxy_node:{node_id.strip()}:{time_bucket}"

    # 构建代理 URL 作为缓存键的基础
    proxy_url = build_proxy_url(proxy_config)
    if not proxy_url:
        return "__no_proxy__"

    # 使用 MD5 哈希来避免过长的键名
    return f"proxy:{hashlib.md5(proxy_url.encode()).hexdigest()[:16]}"


def build_proxy_url(proxy_config: dict[str, Any]) -> str | None:
    """
    根据代理配置构建完整的代理 URL

    Args:
        proxy_config: 代理配置字典，支持两种模式：
            - 手动 URL 模式: {url, username, password, enabled}
            - ProxyNode 模式: {node_id, enabled}

    Returns:
        完整的代理 URL，如 socks5://user:pass@host:port
        如果 enabled=False 或无配置，返回 None
    """
    if not proxy_config:
        return None

    # 检查 enabled 字段，默认为 True（兼容旧数据）
    if not proxy_config.get("enabled", True):
        return None

    # ProxyNode 模式（aether-proxy 或手动节点）
    node_id = proxy_config.get("node_id")
    if isinstance(node_id, str) and node_id.strip():
        node_id = node_id.strip()
        node_info = _get_proxy_node_info(node_id)
        if not node_info:
            raise ProxyNodeUnavailableError(f"代理节点 {node_id} 不可用", node_id=node_id)

        # 手动节点：直接使用存储的代理 URL（含认证信息）
        if node_info.get("is_manual"):
            manual_url = node_info.get("proxy_url")
            if not manual_url:
                raise ProxyNodeUnavailableError(
                    f"手动代理节点 {node_id} 缺少 proxy_url", node_id=node_id
                )
            username = node_info.get("username")
            password = node_info.get("password")
            if username:
                parsed = urlparse(manual_url)
                encoded_username = quote(username, safe="")
                encoded_password = quote(password, safe="") if password else ""
                # 使用 hostname+port 而非 netloc，避免 URL 内嵌凭据导致双重认证
                host_part = parsed.hostname or "localhost"
                if parsed.port:
                    host_part = f"{host_part}:{parsed.port}"
                if encoded_password:
                    auth_url = (
                        f"{parsed.scheme}://{encoded_username}:{encoded_password}@{host_part}"
                    )
                else:
                    auth_url = f"{parsed.scheme}://{encoded_username}@{host_part}"
                if parsed.path:
                    auth_url += parsed.path
                return auth_url
            return manual_url

        # aether-proxy 节点：使用 HMAC 认证
        return _build_hmac_proxy_url(node_info["ip"], node_info["port"], node_id)

    proxy_url: str | None = proxy_config.get("url")
    if not proxy_url:
        return None

    username = proxy_config.get("username")
    password = proxy_config.get("password")

    # 只要有用户名就添加认证信息（密码可以为空）
    if username:
        parsed = urlparse(proxy_url)
        # URL 编码用户名和密码，处理特殊字符（如 @, :, /）
        encoded_username = quote(username, safe="")
        encoded_password = quote(password, safe="") if password else ""
        # 重新构建带认证的代理 URL
        if encoded_password:
            auth_proxy = f"{parsed.scheme}://{encoded_username}:{encoded_password}@{parsed.netloc}"
        else:
            auth_proxy = f"{parsed.scheme}://{encoded_username}@{parsed.netloc}"
        if parsed.path:
            auth_proxy += parsed.path
        return auth_proxy

    return proxy_url


class HTTPClientPool:
    """
    全局HTTP客户端池单例

    管理可重用的httpx.AsyncClient实例,避免频繁创建/销毁连接

    性能优化：
    1. 默认客户端：无代理场景复用
    2. 代理客户端缓存：相同代理配置复用同一客户端
    3. LRU 淘汰：代理客户端超过上限时淘汰最久未使用的
    """

    _instance: HTTPClientPool | None = None
    _default_client: httpx.AsyncClient | None = None
    _clients: dict[str, httpx.AsyncClient] = {}
    # 代理客户端缓存：{cache_key: (client, last_used_time)}
    _proxy_clients: dict[str, tuple[httpx.AsyncClient, float]] = {}
    # 代理客户端缓存上限（避免内存泄漏）
    _max_proxy_clients: int = 50

    def __new__(cls) -> "HTTPClientPool":
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    @classmethod
    async def get_default_client_async(cls) -> httpx.AsyncClient:
        """
        获取默认的HTTP客户端（异步线程安全版本）

        用于大多数HTTP请求,具有合理的默认配置
        """
        if cls._default_client is not None:
            return cls._default_client

        async with _default_client_lock:
            # 双重检查，避免重复创建
            if cls._default_client is None:
                cls._default_client = httpx.AsyncClient(
                    http2=False,  # 暂时禁用HTTP/2以提高兼容性
                    verify=get_ssl_context(),  # 使用 certifi 证书
                    timeout=httpx.Timeout(
                        connect=config.http_connect_timeout,
                        read=config.http_read_timeout,
                        write=config.http_write_timeout,
                        pool=config.http_pool_timeout,
                    ),
                    limits=httpx.Limits(
                        max_connections=config.http_max_connections,
                        max_keepalive_connections=config.http_keepalive_connections,
                        keepalive_expiry=config.http_keepalive_expiry,
                    ),
                    follow_redirects=True,  # 跟随重定向
                )
                logger.info(
                    f"全局HTTP客户端池已初始化: "
                    f"max_connections={config.http_max_connections}, "
                    f"keepalive={config.http_keepalive_connections}, "
                    f"keepalive_expiry={config.http_keepalive_expiry}s"
                )
        return cls._default_client

    @classmethod
    def get_default_client(cls) -> httpx.AsyncClient:
        """
        获取默认的HTTP客户端（同步版本，向后兼容）

        ⚠️ 注意：此方法在高并发首次调用时可能存在竞态条件，
        推荐使用 get_default_client_async() 异步版本。
        """
        if cls._default_client is None:
            cls._default_client = httpx.AsyncClient(
                http2=False,  # 暂时禁用HTTP/2以提高兼容性
                verify=get_ssl_context(),  # 使用 certifi 证书
                timeout=httpx.Timeout(
                    connect=config.http_connect_timeout,
                    read=config.http_read_timeout,
                    write=config.http_write_timeout,
                    pool=config.http_pool_timeout,
                ),
                limits=httpx.Limits(
                    max_connections=config.http_max_connections,
                    max_keepalive_connections=config.http_keepalive_connections,
                    keepalive_expiry=config.http_keepalive_expiry,
                ),
                follow_redirects=True,  # 跟随重定向
            )
            logger.info(
                f"全局HTTP客户端池已初始化: "
                f"max_connections={config.http_max_connections}, "
                f"keepalive={config.http_keepalive_connections}, "
                f"keepalive_expiry={config.http_keepalive_expiry}s"
            )
        return cls._default_client

    @classmethod
    def get_client(cls, name: str, **kwargs: Any) -> httpx.AsyncClient:
        """
        获取或创建命名的HTTP客户端

        用于需要特定配置的场景(如不同的超时设置、代理等)

        Args:
            name: 客户端标识符
            **kwargs: httpx.AsyncClient的配置参数
        """
        if name not in cls._clients:
            # 合并默认配置和自定义配置
            default_config = {
                "http2": False,
                "verify": get_ssl_context(),
                "timeout": httpx.Timeout(
                    connect=config.http_connect_timeout,
                    read=config.http_read_timeout,
                    write=config.http_write_timeout,
                    pool=config.http_pool_timeout,
                ),
                "follow_redirects": True,
            }
            default_config.update(kwargs)

            cls._clients[name] = httpx.AsyncClient(**default_config)  # type: ignore[arg-type]
            logger.debug(f"创建命名HTTP客户端: {name}")

        return cls._clients[name]

    @classmethod
    def _get_proxy_clients_lock(cls) -> asyncio.Lock:
        """获取代理客户端缓存锁（模块级单例，避免竞态条件）"""
        return _proxy_clients_lock

    @classmethod
    async def _evict_lru_proxy_client(cls) -> None:
        """淘汰最久未使用的代理客户端"""
        if len(cls._proxy_clients) < cls._max_proxy_clients:
            return

        # 找到最久未使用的客户端
        oldest_key = min(cls._proxy_clients.keys(), key=lambda k: cls._proxy_clients[k][1])
        old_client, _ = cls._proxy_clients.pop(oldest_key)

        # 异步关闭旧客户端
        try:
            await old_client.aclose()
            logger.debug(f"淘汰代理客户端: {oldest_key}")
        except Exception as e:
            logger.warning(f"关闭代理客户端失败: {e}")

    @classmethod
    async def get_proxy_client(
        cls,
        proxy_config: dict[str, Any] | None = None,
    ) -> httpx.AsyncClient:
        """
        获取代理客户端（带缓存复用）

        相同代理配置会复用同一个客户端，大幅减少连接建立开销。
        当 proxy_config 为 None 时，自动回退到系统默认代理节点。
        注意：返回的客户端使用默认超时配置，如需自定义超时请在请求时传递 timeout 参数。

        Args:
            proxy_config: 代理配置字典，为 None 时使用系统默认代理

        Returns:
            可复用的 httpx.AsyncClient 实例
        """
        # 无特定代理时，回退到系统默认代理
        if not proxy_config:
            proxy_config = get_system_proxy_config()

        cache_key = _compute_proxy_cache_key(proxy_config)

        # 无代理时返回默认客户端
        if cache_key == "__no_proxy__":
            return await cls.get_default_client_async()

        lock = cls._get_proxy_clients_lock()
        async with lock:
            # 检查缓存
            if cache_key in cls._proxy_clients:
                client, _ = cls._proxy_clients[cache_key]
                # 健康检查：如果客户端已关闭，移除并重新创建
                if client.is_closed:
                    del cls._proxy_clients[cache_key]
                    logger.debug(f"代理客户端已关闭，将重新创建: {cache_key}")
                else:
                    # 更新最后使用时间
                    cls._proxy_clients[cache_key] = (client, time.time())
                    return client

            # 淘汰旧客户端（如果超过上限）
            await cls._evict_lru_proxy_client()

            # 创建新客户端（使用默认超时，请求时可覆盖）
            client_config: dict[str, Any] = {
                "http2": False,
                "verify": get_ssl_context(),
                "follow_redirects": True,
                "limits": httpx.Limits(
                    max_connections=config.http_max_connections,
                    max_keepalive_connections=config.http_keepalive_connections,
                    keepalive_expiry=config.http_keepalive_expiry,
                ),
                "timeout": httpx.Timeout(
                    connect=config.http_connect_timeout,
                    read=config.http_read_timeout,
                    write=config.http_write_timeout,
                    pool=config.http_pool_timeout,
                ),
            }

            # 添加代理配置
            proxy_url = build_proxy_url(proxy_config) if proxy_config else None
            if proxy_url:
                client_config["proxy"] = proxy_url

            client = httpx.AsyncClient(**client_config)  # type: ignore[arg-type]
            cls._proxy_clients[cache_key] = (client, time.time())

            proxy_label = "none"
            if proxy_config:
                proxy_label = str(
                    proxy_config.get("node_id") or proxy_config.get("url") or "unknown"
                )
            logger.debug(
                f"创建代理客户端(缓存): {proxy_label}, " f"缓存数量: {len(cls._proxy_clients)}"
            )

            return client

    @classmethod
    async def close_all(cls) -> None:
        """关闭所有HTTP客户端"""
        if cls._default_client is not None:
            await cls._default_client.aclose()
            cls._default_client = None
            logger.info("默认HTTP客户端已关闭")

        for name, client in cls._clients.items():
            await client.aclose()
            logger.debug(f"命名HTTP客户端已关闭: {name}")

        cls._clients.clear()

        # 关闭代理客户端缓存
        for cache_key, (client, _) in cls._proxy_clients.items():
            try:
                await client.aclose()
                logger.debug(f"代理客户端已关闭: {cache_key}")
            except Exception as e:
                logger.warning(f"关闭代理客户端失败: {e}")

        cls._proxy_clients.clear()
        logger.info("所有HTTP客户端已关闭")

    @classmethod
    @asynccontextmanager
    async def get_temp_client(cls, **kwargs: Any) -> Any:
        """
        获取临时HTTP客户端(上下文管理器)

        用于一次性请求,使用后自动关闭

        用法:
            async with HTTPClientPool.get_temp_client() as client:
                response = await client.get('https://example.com')
        """
        default_config = {
            "http2": False,
            "verify": get_ssl_context(),
            "timeout": httpx.Timeout(
                connect=config.http_connect_timeout,
                read=config.http_read_timeout,
                write=config.http_write_timeout,
                pool=config.http_pool_timeout,
            ),
        }
        default_config.update(kwargs)

        client = httpx.AsyncClient(**default_config)  # type: ignore[arg-type]
        try:
            yield client
        finally:
            await client.aclose()

    @classmethod
    def create_client_with_proxy(
        cls,
        proxy_config: dict[str, Any] | None = None,
        timeout: httpx.Timeout | None = None,
        **kwargs: Any,
    ) -> httpx.AsyncClient:
        """
        创建带代理配置的HTTP客户端

        ⚠️ 性能警告：此方法每次都创建新客户端，推荐使用 get_proxy_client() 复用连接。

        Args:
            proxy_config: 代理配置字典，包含 url, username, password
            timeout: 超时配置
            **kwargs: 其他 httpx.AsyncClient 配置参数

        Returns:
            配置好的 httpx.AsyncClient 实例（调用者需要负责关闭）
        """
        client_config: dict[str, Any] = {
            "http2": False,
            "verify": get_ssl_context(),
            "follow_redirects": True,
        }

        if timeout:
            client_config["timeout"] = timeout
        else:
            client_config["timeout"] = httpx.Timeout(
                connect=config.http_connect_timeout,
                read=config.http_read_timeout,
                write=config.http_write_timeout,
                pool=config.http_pool_timeout,
            )

        # 添加代理配置
        proxy_url = build_proxy_url(proxy_config) if proxy_config else None
        if proxy_url:
            client_config["proxy"] = proxy_url
            logger.debug(f"创建带代理的HTTP客户端(一次性): {proxy_config.get('url', 'unknown')}")

        client_config.update(kwargs)
        return httpx.AsyncClient(**client_config)  # type: ignore[arg-type]

    @classmethod
    def get_pool_stats(cls) -> dict[str, Any]:
        """获取连接池统计信息"""
        return {
            "default_client_active": cls._default_client is not None,
            "named_clients_count": len(cls._clients),
            "proxy_clients_count": len(cls._proxy_clients),
            "max_proxy_clients": cls._max_proxy_clients,
        }


# 便捷访问函数
def get_http_client() -> httpx.AsyncClient:
    """获取默认HTTP客户端的便捷函数"""
    return HTTPClientPool.get_default_client()


async def close_http_clients() -> None:
    """关闭所有HTTP客户端的便捷函数"""
    await HTTPClientPool.close_all()
