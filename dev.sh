#!/bin/bash
# 本地开发启动脚本
set -euo pipefail
clear

# 加载 .env 文件
set -a
source .env
set +a

# 构建 DATABASE_URL
export DATABASE_URL="postgresql://${DB_USER:-postgres}:${DB_PASSWORD}@${DB_HOST:-localhost}:${DB_PORT:-5432}/${DB_NAME:-aether}"
export REDIS_URL=redis://:${REDIS_PASSWORD}@${REDIS_HOST:-localhost}:${REDIS_PORT:-6379}/0

# 开发环境连接池低配（节省内存）
export DB_POOL_SIZE=${DB_POOL_SIZE:-5}
export DB_MAX_OVERFLOW=${DB_MAX_OVERFLOW:-5}
export HTTP_MAX_CONNECTIONS=${HTTP_MAX_CONNECTIONS:-20}
export HTTP_KEEPALIVE_CONNECTIONS=${HTTP_KEEPALIVE_CONNECTIONS:-5}

EXECUTOR_PID=""
GATEWAY_PID=""

cleanup() {
    if [ -n "${GATEWAY_PID}" ]; then
        echo ""
        echo "=> 停止 aether-gateway..."
        kill "${GATEWAY_PID}" >/dev/null 2>&1 || true
        wait "${GATEWAY_PID}" >/dev/null 2>&1 || true
    fi

    if [ -n "${EXECUTOR_PID}" ]; then
        echo ""
        echo "=> 停止 aether-executor..."
        kill "${EXECUTOR_PID}" >/dev/null 2>&1 || true
        wait "${EXECUTOR_PID}" >/dev/null 2>&1 || true
    fi

    if [ "${EXECUTOR_TRANSPORT:-}" = "unix_socket" ] && [ -n "${EXECUTOR_SOCKET_PATH:-}" ]; then
        rm -f "${EXECUTOR_SOCKET_PATH}" >/dev/null 2>&1 || true
    fi
}

trap cleanup EXIT

APP_PORT=${APP_PORT:-8084}
PYTHON_INTERNAL_HOST=${PYTHON_INTERNAL_HOST:-127.0.0.1}
PYTHON_INTERNAL_PORT=${PYTHON_INTERNAL_PORT:-18084}

should_start_executor=false
if [ "${DEV_START_EXECUTOR:-false}" = "true" ] || [ "${EXECUTOR_BACKEND:-rust}" = "rust" ]; then
    should_start_executor=true
fi

should_start_gateway=true
if [ "${DEV_START_GATEWAY:-true}" != "true" ]; then
    should_start_gateway=false
fi

if [ "${should_start_executor}" = "true" ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "=> 未找到 cargo，无法启动 aether-executor。请先安装 Rust toolchain。"
        exit 1
    fi

    if [ -z "${EXECUTOR_TRANSPORT:-}" ]; then
        if [ "${should_start_gateway}" = "true" ]; then
            export EXECUTOR_TRANSPORT=tcp
        else
            export EXECUTOR_TRANSPORT=unix_socket
        fi
    fi
    export EXECUTOR_SOCKET_PATH=${EXECUTOR_SOCKET_PATH:-/tmp/aether-executor.sock}
    export EXECUTOR_BASE_URL=${EXECUTOR_BASE_URL:-http://127.0.0.1:5219}
    export EXECUTOR_BIND=${EXECUTOR_BIND:-127.0.0.1:5219}

    if [ "${EXECUTOR_TRANSPORT}" = "unix_socket" ]; then
        rm -f "${EXECUTOR_SOCKET_PATH}"
        echo "=> 启动 aether-executor (unix socket: ${EXECUTOR_SOCKET_PATH})..."
        cargo run -q -p aether-executor -- --transport unix_socket --unix-socket "${EXECUTOR_SOCKET_PATH}" &
        EXECUTOR_PID=$!

        for _ in {1..100}; do
            if [ -S "${EXECUTOR_SOCKET_PATH}" ]; then
                break
            fi
            sleep 0.1
        done

        if [ ! -S "${EXECUTOR_SOCKET_PATH}" ]; then
            echo "=> aether-executor 未能在预期时间内启动。"
            exit 1
        fi
    else
        echo "=> 启动 aether-executor (tcp: ${EXECUTOR_BIND})..."
        cargo run -q -p aether-executor -- --transport tcp --bind "${EXECUTOR_BIND}" &
        EXECUTOR_PID=$!

        for _ in {1..100}; do
            if curl -sf "${EXECUTOR_BASE_URL}/health" >/dev/null 2>&1; then
                break
            fi
            sleep 0.1
        done

        if ! curl -sf "${EXECUTOR_BASE_URL}/health" >/dev/null 2>&1; then
            echo "=> aether-executor 未能在预期时间内启动。"
            exit 1
        fi
    fi
fi

if [ "${should_start_gateway}" = "true" ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "=> 未找到 cargo，无法启动 aether-gateway。请先安装 Rust toolchain。"
        exit 1
    fi

    export AETHER_GATEWAY_BIND=${AETHER_GATEWAY_BIND:-0.0.0.0:${APP_PORT}}
    export AETHER_GATEWAY_UPSTREAM=${AETHER_GATEWAY_UPSTREAM:-http://${PYTHON_INTERNAL_HOST}:${PYTHON_INTERNAL_PORT}}
    export AETHER_GATEWAY_CONTROL_URL=${AETHER_GATEWAY_CONTROL_URL:-http://${PYTHON_INTERNAL_HOST}:${PYTHON_INTERNAL_PORT}}
    if [ "${should_start_executor}" = "true" ] && [ "${EXECUTOR_TRANSPORT:-}" = "tcp" ]; then
        export AETHER_GATEWAY_EXECUTOR_URL=${AETHER_GATEWAY_EXECUTOR_URL:-${EXECUTOR_BASE_URL}}
    fi

    if [ -n "${AETHER_GATEWAY_EXECUTOR_URL:-}" ]; then
        echo "=> 启动 aether-gateway (${AETHER_GATEWAY_BIND} -> ${AETHER_GATEWAY_UPSTREAM}, control=${AETHER_GATEWAY_CONTROL_URL}, executor=${AETHER_GATEWAY_EXECUTOR_URL})..."
        cargo run -q -p aether-gateway -- --bind "${AETHER_GATEWAY_BIND}" --upstream "${AETHER_GATEWAY_UPSTREAM}" --control-url "${AETHER_GATEWAY_CONTROL_URL}" --executor-url "${AETHER_GATEWAY_EXECUTOR_URL}" &
    else
        echo "=> 启动 aether-gateway (${AETHER_GATEWAY_BIND} -> ${AETHER_GATEWAY_UPSTREAM}, control=${AETHER_GATEWAY_CONTROL_URL})..."
        cargo run -q -p aether-gateway -- --bind "${AETHER_GATEWAY_BIND}" --upstream "${AETHER_GATEWAY_UPSTREAM}" --control-url "${AETHER_GATEWAY_CONTROL_URL}" &
    fi
    GATEWAY_PID=$!

    for _ in {1..100}; do
        if curl -sf "http://127.0.0.1:${APP_PORT}/_gateway/health" >/dev/null 2>&1; then
            break
        fi
        sleep 0.1
    done

    if ! curl -sf "http://127.0.0.1:${APP_PORT}/_gateway/health" >/dev/null 2>&1; then
        echo "=> aether-gateway 未能在预期时间内启动。"
        exit 1
    fi
fi

PYTHON_BIND_HOST=${HOST:-0.0.0.0}
PYTHON_BIND_PORT=${PORT:-${APP_PORT}}
if [ "${should_start_gateway}" = "true" ]; then
    PYTHON_BIND_HOST=${PYTHON_INTERNAL_HOST}
    PYTHON_BIND_PORT=${PYTHON_INTERNAL_PORT}
fi

export HOST="${PYTHON_BIND_HOST}"
export PORT="${PYTHON_BIND_PORT}"

# 启动 uvicorn（热重载模式，只监视 src 目录）
echo "=> 启动本地开发服务器..."
if [ "${should_start_gateway}" = "true" ]; then
    echo "=> 主入口:      http://localhost:${APP_PORT} (aether-gateway)"
    echo "=> Python内部:  http://${PYTHON_BIND_HOST}:${PYTHON_BIND_PORT}"
    echo "=> Control API: ${AETHER_GATEWAY_CONTROL_URL}"
    if [ -n "${AETHER_GATEWAY_EXECUTOR_URL:-}" ]; then
        echo "=> Executor API: ${AETHER_GATEWAY_EXECUTOR_URL}"
    fi
else
    echo "=> 后端地址:    http://localhost:${APP_PORT}"
fi
echo "=> 数据库: ${DATABASE_URL}"
if [ "${should_start_executor}" = "true" ]; then
    echo "=> Executor 后端: ${EXECUTOR_BACKEND:-rust} (${EXECUTOR_TRANSPORT})"
fi
echo ""

uv run uvicorn src.main:app --reload --reload-dir src --host "${PYTHON_BIND_HOST}" --port "${PYTHON_BIND_PORT}"
