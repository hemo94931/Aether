#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportMode {
    UnixSocketHttp,
    TcpHttp,
}
