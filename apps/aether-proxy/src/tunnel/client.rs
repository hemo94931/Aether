//! WebSocket tunnel client: connect, authenticate, and run the tunnel.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tracing::{debug, info, warn};

use crate::state::{AppState, ServerContext};

use super::{dispatcher, heartbeat, writer};

/// Outcome of a tunnel session.
pub enum TunnelOutcome {
    /// Graceful shutdown requested by the local process.
    Shutdown,
    /// Remote side disconnected or connection lost — should reconnect.
    Disconnected,
}

/// Connect to Aether's WebSocket tunnel endpoint and run until disconnected.
///
/// `conn_idx` identifies which connection in the pool this is (0-based).
/// Only connection 0 sends heartbeats to avoid resetting shared metrics.
pub async fn connect_and_run(
    state: &Arc<AppState>,
    server: &Arc<ServerContext>,
    conn_idx: usize,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<TunnelOutcome, anyhow::Error> {
    let ws_url = build_tunnel_url(server);
    info!(url = %ws_url, conn = conn_idx, "connecting tunnel");

    // Build WebSocket request with auth headers
    let mut request = ws_url.clone().into_client_request()?;
    let headers = request.headers_mut();
    headers.insert(
        "Authorization",
        http::HeaderValue::from_str(&format!("Bearer {}", server.management_token))?,
    );
    let node_id = server.node_id.read().unwrap().clone();
    headers.insert("X-Node-Id", http::HeaderValue::from_str(&node_id)?);
    // Use dynamic node_name (may be updated by remote config) instead of
    // the static server.node_name, so that remote name changes take effect
    // on the next reconnect.
    let dynamic_node_name = server.dynamic.load().node_name.clone();
    headers.insert(
        "X-Node-Name",
        http::HeaderValue::from_str(&dynamic_node_name)?,
    );
    // Advertise per-connection max concurrent streams so the backend can
    // respect the proxy's capacity limit (backward-compatible: old backends
    // ignore this header).
    let max_streams = state.config.tunnel_max_streams.unwrap_or(128);
    headers.insert("X-Tunnel-Max-Streams", http::HeaderValue::from(max_streams));

    // Parse host:port from URL
    let uri: http::Uri = ws_url.parse()?;
    let host = uri
        .host()
        .ok_or_else(|| anyhow::anyhow!("missing host in tunnel URL"))?;
    let is_tls = uri.scheme_str() == Some("wss");
    let port = uri.port_u16().unwrap_or(if is_tls { 443 } else { 80 });

    // TCP connect with timeout
    let connect_timeout = Duration::from_secs(state.config.tunnel_connect_timeout_secs);
    let tcp_stream = tokio::time::timeout(connect_timeout, TcpStream::connect((host, port)))
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "tunnel TCP connect timeout ({}s)",
                connect_timeout.as_secs()
            )
        })??;

    // Configure TCP parameters via socket2
    configure_tcp_socket(&tcp_stream, state);

    // WebSocket upgrade (with TLS if wss://)
    let connector = if is_tls {
        Some(tokio_tungstenite::Connector::Rustls(Arc::clone(
            &state.tunnel_tls_config,
        )))
    } else {
        None
    };
    // Match Python-side _MAX_FRAME_SIZE (64 MiB) to prevent tungstenite's
    // default 16 MiB limit from rejecting large AI API payloads (multi-image
    // base64 requests can exceed 16 MiB).
    let ws_config = WebSocketConfig {
        max_frame_size: Some(64 << 20),
        max_message_size: Some(64 << 20),
        ..Default::default()
    };
    let handshake_timeout = Duration::from_secs(state.config.tunnel_connect_timeout_secs);
    let (ws_stream, _response) = tokio::time::timeout(
        handshake_timeout,
        tokio_tungstenite::client_async_tls_with_config(
            request,
            tcp_stream,
            Some(ws_config),
            connector,
        ),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "tunnel WebSocket handshake timeout ({}s)",
            handshake_timeout.as_secs()
        )
    })??;
    info!(
        conn = conn_idx,
        tcp_keepalive_secs = state.config.tunnel_tcp_keepalive_secs,
        tcp_nodelay = state.config.tunnel_tcp_nodelay,
        connect_timeout_secs = state.config.tunnel_connect_timeout_secs,
        stale_timeout_secs = state.config.tunnel_stale_timeout_secs,
        "tunnel connected"
    );

    // NOTE: reconnect_attempts reset is handled by the caller (mod.rs)
    // based on how long the connection stayed alive.

    // Split into read/write halves
    let (ws_sink, ws_read) = futures_util::StreamExt::split(ws_stream);

    // Spawn writer task (with WebSocket ping keepalive)
    let ping_interval = Duration::from_secs(state.config.tunnel_ping_interval_secs);
    let (frame_tx, mut writer_handle) = writer::spawn_writer(ws_sink, ping_interval);

    // Spawn heartbeat task (only for primary connection to avoid
    // resetting shared atomic metrics via swap(0))
    let hb_handle = if conn_idx == 0 {
        heartbeat::spawn(
            Arc::clone(state),
            Arc::clone(server),
            frame_tx.clone(),
            shutdown.clone(),
        )
    } else {
        heartbeat::spawn_noop()
    };

    // Run dispatcher (blocks until disconnect or shutdown).
    // Also watch for writer exit — if the write half dies (e.g. the peer
    // closed the connection) but the read half stays open, dispatcher would
    // block forever on `ws_stream.next()`.  Monitoring `writer_handle`
    // ensures we detect this and trigger a reconnect promptly.
    let state_clone = Arc::clone(state);
    let server_clone = Arc::clone(server);
    let outcome = tokio::select! {
        result = dispatcher::run(state_clone, server_clone, ws_read, frame_tx.clone(), hb_handle) => {
            match result {
                Ok(()) => TunnelOutcome::Disconnected,
                Err(e) => return Err(e),
            }
        }
        writer_result = &mut writer_handle => {
            match writer_result {
                Ok(()) => warn!("writer task exited normally, triggering reconnect"),
                Err(e) => {
                    if e.is_panic() {
                        tracing::error!(error = %e, "writer task panicked, triggering reconnect");
                    } else {
                        warn!(error = %e, "writer task cancelled, triggering reconnect");
                    }
                }
            }
            TunnelOutcome::Disconnected
        }
        _ = shutdown.changed() => {
            debug!("shutdown during tunnel dispatch");
            TunnelOutcome::Shutdown
        }
    };

    // Drop our sender; the writer will exit once all stream handler clones
    // are also dropped (i.e. after they finish their in-flight work).
    drop(frame_tx);

    // Wait for the writer task to finish with a generous timeout — the
    // dispatcher already waits up to 30s for stream handlers, so 35s here
    // covers that plus a small margin.
    // Skip if the writer already exited (the select branch that fired).
    if !writer_handle.is_finished() {
        let _ = tokio::time::timeout(Duration::from_secs(35), writer_handle).await;
    }

    info!("tunnel disconnected");
    Ok(outcome)
}

/// Configure TCP keepalive and NODELAY on an established socket.
fn configure_tcp_socket(stream: &TcpStream, state: &Arc<AppState>) {
    let sock_ref = socket2::SockRef::from(stream);

    if state.config.tunnel_tcp_keepalive_secs > 0 {
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(Duration::from_secs(state.config.tunnel_tcp_keepalive_secs))
            .with_interval(Duration::from_secs(5));
        #[cfg(not(target_os = "windows"))]
        let keepalive = keepalive.with_retries(3);
        if let Err(e) = sock_ref.set_tcp_keepalive(&keepalive) {
            warn!(error = %e, "failed to set TCP keepalive on tunnel socket");
        }
    }

    if state.config.tunnel_tcp_nodelay {
        if let Err(e) = sock_ref.set_nodelay(true) {
            warn!(error = %e, "failed to set TCP_NODELAY on tunnel socket");
        }
    }
}

/// Build rustls ClientConfig with system root certificates.
pub fn build_tls_config() -> rustls::ClientConfig {
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

fn build_tunnel_url(server: &ServerContext) -> String {
    let base = server.aether_url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") {
        base.replacen("https://", "wss://", 1)
    } else if base.starts_with("http://") {
        base.replacen("http://", "ws://", 1)
    } else {
        format!("wss://{}", base)
    };
    format!("{}/api/internal/proxy-tunnel", ws_base)
}
