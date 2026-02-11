# aether-proxy

Aether 正向代理节点，部署在海外 VPS 上，为墙内的 Aether 实例中转 API 流量。

## 安装

### 下载预编译二进制

在 [GitHub Releases](https://github.com/fawney19/Aether/releases) 页面下载对应平台的预编译文件，无需安装 Rust 环境。

## 快速开始

```bash
# 1. 首次安装配置（TUI 向导，勾选 Install Service 随系统启动服务）
sudo ./aether-proxy setup

# 2. 日常管理 (勾选 Install Service 作为系统服务的情况下)
aether-proxy status          # 看状态
aether-proxy logs            # 看日志

sudo aether-proxy start      # 启动服务
sudo aether-proxy stop       # 停止服务
sudo aether-proxy restart    # 重启服务

# 3. 重新配置（改完自动重启服务）
sudo aether-proxy setup

# 4. 彻底卸载
sudo aether-proxy uninstall
```

完成向导后, 配置自动保存到 `aether-proxy.toml`，如果启用了 Install Service，将自动注册并启动 systemd 服务。

### 直接运行

如果不需要安装为系统服务，可以直接运行。缺少必填参数时会自动进入 setup 向导：

```bash
./aether-proxy
```

## 配置

配置按以下优先级加载（高优先级覆盖低优先级）：

1. CLI 参数
2. 环境变量（`AETHER_PROXY_*`）
3. 配置文件（`aether-proxy.toml`，或通过 `AETHER_PROXY_CONFIG` 指定路径）

### 参数一览

| 参数 | 环境变量 | 默认值 | 说明 |
|------|----------|--------|------|
| `--aether-url` | `AETHER_PROXY_AETHER_URL` | **必填** | Aether 服务器地址 |
| `--management-token` | `AETHER_PROXY_MANAGEMENT_TOKEN` | **必填** | 管理员 Token（`ae_xxx` 格式） |
| `--hmac-key` | `AETHER_PROXY_HMAC_KEY` | **必填** | HMAC 密钥，需与 Aether 端一致 |
| `--listen-port` | `AETHER_PROXY_LISTEN_PORT` | `18080` | 监听端口 |
| `--public-ip` | `AETHER_PROXY_PUBLIC_IP` | 自动检测 | 公网 IP |
| `--node-name` | `AETHER_PROXY_NODE_NAME` | `proxy-01` | 节点名称标识 |
| `--node-region` | `AETHER_PROXY_NODE_REGION` | 自动检测 | 地区标识 |
| `--heartbeat-interval` | `AETHER_PROXY_HEARTBEAT_INTERVAL` | `30` | 心跳间隔（秒） |
| `--allowed-ports` | `AETHER_PROXY_ALLOWED_PORTS` | `80,443,8080,8443` | 允许代理的目标端口 |
| `--timestamp-tolerance` | `AETHER_PROXY_TIMESTAMP_TOLERANCE` | `300` | HMAC 时间戳容差（秒） |
| `--aether-request-timeout` | `AETHER_PROXY_AETHER_REQUEST_TIMEOUT` | `10` | Aether API 请求总超时（秒） |
| `--aether-connect-timeout` | `AETHER_PROXY_AETHER_CONNECT_TIMEOUT` | `10` | Aether API 建连超时（秒） |
| `--aether-pool-max-idle-per-host` | `AETHER_PROXY_AETHER_POOL_MAX_IDLE_PER_HOST` | `8` | Aether API 每 Host 最大空闲连接数 |
| `--aether-pool-idle-timeout` | `AETHER_PROXY_AETHER_POOL_IDLE_TIMEOUT` | `90` | Aether API 连接池空闲超时（秒） |
| `--aether-tcp-keepalive` | `AETHER_PROXY_AETHER_TCP_KEEPALIVE` | `60` | Aether API TCP keepalive（秒，0 关闭） |
| `--aether-tcp-nodelay` | `AETHER_PROXY_AETHER_TCP_NODELAY` | `true` | Aether API 启用 TCP_NODELAY |
| `--aether-http2` | `AETHER_PROXY_AETHER_HTTP2` | `true` | Aether API 启用 HTTP/2 |
| `--aether-retry-max-attempts` | `AETHER_PROXY_AETHER_RETRY_MAX_ATTEMPTS` | `3` | Aether API 最大重试次数（含首次） |
| `--aether-retry-base-delay-ms` | `AETHER_PROXY_AETHER_RETRY_BASE_DELAY_MS` | `200` | Aether API 重试基础延迟（毫秒） |
| `--aether-retry-max-delay-ms` | `AETHER_PROXY_AETHER_RETRY_MAX_DELAY_MS` | `2000` | Aether API 重试最大延迟（毫秒） |
| `--max-concurrent-connections` | `AETHER_PROXY_MAX_CONCURRENT_CONNECTIONS` | 自动估算 | 最大并发连接数（默认硬件估算） |
| `--connect-timeout` | `AETHER_PROXY_CONNECT_TIMEOUT` | `30` | CONNECT 上游建连超时（秒） |
| `--tls-handshake-timeout` | `AETHER_PROXY_TLS_HANDSHAKE_TIMEOUT` | `10` | TLS 握手超时（秒） |
| `--dns-cache-ttl` | `AETHER_PROXY_DNS_CACHE_TTL` | `60` | DNS 缓存 TTL（秒） |
| `--dns-cache-capacity` | `AETHER_PROXY_DNS_CACHE_CAPACITY` | `1024` | DNS 缓存容量（条目数） |
| `--delegate-connect-timeout` | `AETHER_PROXY_DELEGATE_CONNECT_TIMEOUT` | `30` | delegate 上游建连超时（秒） |
| `--delegate-pool-max-idle-per-host` | `AETHER_PROXY_DELEGATE_POOL_MAX_IDLE_PER_HOST` | `64` | delegate 每 Host 最大空闲连接数 |
| `--delegate-pool-idle-timeout` | `AETHER_PROXY_DELEGATE_POOL_IDLE_TIMEOUT` | `300` | delegate 连接池空闲超时（秒） |
| `--delegate-tcp-keepalive` | `AETHER_PROXY_DELEGATE_TCP_KEEPALIVE` | `60` | delegate TCP keepalive（秒，0 关闭） |
| `--delegate-tcp-nodelay` | `AETHER_PROXY_DELEGATE_TCP_NODELAY` | `true` | delegate 启用 TCP_NODELAY |
| `--log-level` | `AETHER_PROXY_LOG_LEVEL` | `info` | 日志级别 |
| `--log-json` | `AETHER_PROXY_LOG_JSON` | `false` | JSON 格式日志 |
| `--enable-tls` | `AETHER_PROXY_ENABLE_TLS` | `true` | 启用 TLS |
| `--tls-cert` | `AETHER_PROXY_TLS_CERT` | `aether-proxy-cert.pem` | TLS 证书路径 |
| `--tls-key` | `AETHER_PROXY_TLS_KEY` | `aether-proxy-key.pem` | TLS 私钥路径 |

## 发布新版本

推送 `proxy-v*` 格式的 tag，GitHub Actions 会自动编译所有平台并发布到 Releases：

```bash
git tag proxy-v0.1.0
git push origin proxy-v0.1.0
```
