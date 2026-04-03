from src.services.proxy_node.tunnel_config import TunnelRelayConfig


def test_local_relay_url_uses_http_path() -> None:
    config = TunnelRelayConfig(
        enabled=True,
        url="http://127.0.0.1:8084",
        connect_timeout_seconds=1.0,
    )

    assert (
        config.local_relay_url("node a/1")
        == "http://127.0.0.1:8084/api/internal/tunnel/relay/node%20a%2F1"
    )
