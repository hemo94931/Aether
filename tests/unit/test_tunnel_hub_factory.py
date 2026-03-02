from src.services.proxy_node.hub_transport import HubTunnelTransport
from src.services.proxy_node.tunnel_protocol import Frame, MsgType
from src.services.proxy_node.tunnel_transport import create_tunnel_transport


def test_create_tunnel_transport_always_uses_hub_transport() -> None:
    transport = create_tunnel_transport("node-1", timeout=12.0)
    assert isinstance(transport, HubTunnelTransport)


def test_tunnel_protocol_supports_node_status_msg_type() -> None:
    raw = Frame(0, MsgType.NODE_STATUS, 0, b'{"node_id":"n1","connected":true}').encode()
    decoded = Frame.decode(raw)
    assert decoded.msg_type == MsgType.NODE_STATUS
