use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use hyper::rt;
use hyper::Uri;
use hyper_util::client::legacy::connect::{Connected, Connection, HttpConnector};
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use tokio_rustls::TlsConnector;
use tower_service::Service;

use crate::config::Config;
use crate::proxy::BoxBody;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

type DelegateStream = MaybeHttpsStream<TokioIo<tokio::net::TcpStream>>;

type DelegateConn = TimedConn<DelegateStream>;

pub(crate) type DelegateClient = Client<InstrumentedConnector, BoxBody>;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ConnectTiming {
    pub connect_ms: u64,
    pub tls_ms: u64,
}

pub(crate) fn build_delegate_client(config: &Config) -> DelegateClient {
    let mut http = HttpConnector::new();
    http.enforce_http(false);
    http.set_connect_timeout(Some(Duration::from_secs(
        config.delegate_connect_timeout_secs,
    )));
    http.set_nodelay(config.delegate_tcp_nodelay);
    if config.delegate_tcp_keepalive_secs > 0 {
        http.set_keepalive(Some(Duration::from_secs(
            config.delegate_tcp_keepalive_secs,
        )));
    } else {
        http.set_keepalive(None);
    }

    let connector = InstrumentedConnector {
        http,
        tls_config: build_tls_config(),
    };

    let mut builder = Client::builder(TokioExecutor::new());
    builder.pool_max_idle_per_host(config.delegate_pool_max_idle_per_host);
    builder.pool_idle_timeout(Duration::from_secs(config.delegate_pool_idle_timeout_secs));
    builder.pool_timer(TokioTimer::new());

    builder.build::<_, BoxBody>(connector)
}

fn build_tls_config() -> Arc<ClientConfig> {
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Arc::new(config)
}

#[derive(Clone)]
pub(crate) struct InstrumentedConnector {
    http: HttpConnector,
    tls_config: Arc<ClientConfig>,
}

impl Service<Uri> for InstrumentedConnector {
    type Response = DelegateConn;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, BoxError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.http.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let scheme = dst.scheme_str().map(|s| s.to_ascii_lowercase());
        let tls_config = self.tls_config.clone();
        let connecting = self.http.call(dst.clone());
        let connect_start = Instant::now();

        Box::pin(async move {
            match scheme.as_deref() {
                Some("http") => {
                    let tcp = connecting.await.map_err(|e| Box::new(e) as BoxError)?;
                    let connect_ms = connect_start.elapsed().as_millis() as u64;
                    Ok(TimedConn::new(
                        MaybeHttpsStream::Http(tcp),
                        ConnectTiming {
                            connect_ms,
                            tls_ms: 0,
                        },
                    ))
                }
                Some("https") => {
                    let server_name = resolve_server_name(&dst)?;
                    let tcp = connecting.await.map_err(|e| Box::new(e) as BoxError)?;
                    let connect_ms = connect_start.elapsed().as_millis() as u64;

                    let tls_start = Instant::now();
                    let tls_stream = TlsConnector::from(tls_config)
                        .connect(server_name, TokioIo::new(tcp))
                        .await
                        .map_err(std::io::Error::other)?;
                    let tls_ms = tls_start.elapsed().as_millis() as u64;

                    Ok(TimedConn::new(
                        MaybeHttpsStream::Https(TokioIo::new(tls_stream)),
                        ConnectTiming { connect_ms, tls_ms },
                    ))
                }
                Some(other) => {
                    Err(std::io::Error::other(format!("unsupported scheme {other}")).into())
                }
                None => Err(std::io::Error::other("missing scheme").into()),
            }
        })
    }
}

fn resolve_server_name(uri: &Uri) -> Result<ServerName<'static>, BoxError> {
    let host = uri.host().ok_or("missing host")?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Ok(ServerName::try_from(host.to_string())?)
}

pub(crate) struct TimedConn<T> {
    inner: T,
    timing: ConnectTiming,
}

impl<T> TimedConn<T> {
    fn new(inner: T, timing: ConnectTiming) -> Self {
        Self { inner, timing }
    }
}

impl<T: Connection> Connection for TimedConn<T> {
    fn connected(&self) -> Connected {
        self.inner.connected().extra(self.timing)
    }
}

impl<T: rt::Read + Unpin> rt::Read for TimedConn<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<T: rt::Write + Unpin> rt::Write for TimedConn<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum MaybeHttpsStream<T> {
    Http(T),
    Https(TokioIo<tokio_rustls::client::TlsStream<TokioIo<T>>>),
}

impl<T: rt::Read + rt::Write + Connection + Unpin> Connection for MaybeHttpsStream<T> {
    fn connected(&self) -> Connected {
        match self {
            Self::Http(stream) => stream.connected(),
            Self::Https(stream) => {
                let (tcp, tls) = stream.inner().get_ref();
                if tls.alpn_protocol() == Some(b"h2") {
                    tcp.inner().connected().negotiated_h2()
                } else {
                    tcp.inner().connected()
                }
            }
        }
    }
}

impl<T: rt::Read + rt::Write + Unpin> rt::Read for MaybeHttpsStream<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Https(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl<T: rt::Write + rt::Read + Unpin> rt::Write for MaybeHttpsStream<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Https(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(stream) => Pin::new(stream).poll_flush(cx),
            Self::Https(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Https(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            Self::Http(stream) => stream.is_write_vectored(),
            Self::Https(stream) => stream.is_write_vectored(),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(stream) => Pin::new(stream).poll_write_vectored(cx, bufs),
            Self::Https(stream) => Pin::new(stream).poll_write_vectored(cx, bufs),
        }
    }
}
