//! Application lifecycle: initialization, task orchestration, and shutdown.
//!
//! Extracted from `main.rs` to keep the entry point minimal and consolidate
//! the startup sequence, tracing init, and graceful shutdown logic.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::signal;
use tokio::sync::{watch, Semaphore};
use tracing::{error, info};

use crate::config::Config;
use crate::net;
use crate::registration::client::AetherClient;
use crate::runtime::{self, DynamicConfig};
use crate::state::{AppState, ProxyMetrics};
use crate::{hardware, proxy};

/// Run the full application lifecycle after config has been parsed.
pub async fn run(mut config: Config) -> anyhow::Result<()> {
    init_tracing(&config);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        port = config.listen_port,
        node_name = %config.node_name,
        "aether-proxy starting"
    );

    // Resolve public IP
    let public_ip = match &config.public_ip {
        Some(ip) => ip.clone(),
        None => net::detect_public_ip().await?,
    };
    info!(public_ip = %public_ip, "using public IP");

    // Auto-detect region if not configured
    if config.node_region.is_none() {
        if let Some(region) = net::detect_region(&public_ip).await {
            config.node_region = Some(region);
        }
    }

    // Initialize TLS if enabled
    let (tls_acceptor, tls_fingerprint) = if config.enable_tls {
        let cert_path = std::path::PathBuf::from(&config.tls_cert);
        let key_path = std::path::PathBuf::from(&config.tls_key);

        proxy::tls::ensure_self_signed_cert(&cert_path, &key_path)?;
        let acceptor = proxy::tls::build_tls_acceptor(&cert_path, &key_path)?;
        let fingerprint = proxy::tls::cert_sha256_fingerprint(&cert_path)?;

        info!(fingerprint = %fingerprint, "TLS enabled");
        (Some(acceptor), Some(fingerprint))
    } else {
        info!("TLS disabled");
        (None, None)
    };

    // Collect hardware info (once at startup)
    let hw_info = hardware::collect();

    let max_connections_raw = config
        .max_concurrent_connections
        .unwrap_or(hw_info.estimated_max_concurrency)
        .max(1);
    let max_connections = usize::try_from(max_connections_raw).unwrap_or(usize::MAX);
    info!(
        max_connections = max_connections_raw,
        "connection limit configured"
    );

    let connection_semaphore = Arc::new(Semaphore::new(max_connections));
    let metrics = Arc::new(ProxyMetrics::new());
    let dns_cache = Arc::new(proxy::target_filter::DnsCache::new(
        Duration::from_secs(config.dns_cache_ttl_secs),
        config.dns_cache_capacity,
    ));

    // Register with Aether
    let aether_client = Arc::new(AetherClient::new(&config));
    let node_id = aether_client
        .register(
            &config,
            &public_ip,
            config.enable_tls,
            tls_fingerprint.as_deref(),
            Some(&hw_info),
        )
        .await?;

    info!(node_id = %node_id, "node registered");

    // Build DynamicConfig before moving config into Arc
    let dynamic = Arc::new(RwLock::new(DynamicConfig::from_config(&config)));

    // Build delegate HTTP client (for proxy-initiated upstream requests).
    let delegate_client = proxy::delegate_client::build_delegate_client(&config);

    // Build shared application state
    let state = Arc::new(AppState {
        config: Arc::new(config),
        node_id: Arc::new(RwLock::new(node_id)),
        dynamic,
        aether_client,
        hardware_info: Arc::new(hw_info),
        public_ip,
        tls_fingerprint,
        tls_acceptor,
        delegate_client,
        active_connections: Arc::new(AtomicU64::new(0)),
        connection_semaphore,
        dns_cache,
        metrics,
    });

    // Shutdown signal channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Start heartbeat task
    let heartbeat_handle = {
        let state = Arc::clone(&state);
        let rx = shutdown_rx.clone();
        tokio::spawn(async move {
            crate::registration::heartbeat::run(&state, rx).await;
        })
    };

    // Start proxy server
    let server_handle = {
        let state = Arc::clone(&state);
        let rx = shutdown_rx.clone();
        tokio::spawn(async move {
            if let Err(e) = proxy::server::run(&state, rx).await {
                error!(error = %e, "proxy server error");
            }
        })
    };

    // Wait for shutdown signal (SIGTERM or SIGINT)
    wait_for_shutdown().await;

    info!("shutdown signal received, cleaning up...");

    // Signal all tasks to stop
    let _ = shutdown_tx.send(true);

    // Graceful unregister (best-effort)
    let current_node_id = state.node_id.read().unwrap().clone();
    if let Err(e) = state.aether_client.unregister(&current_node_id).await {
        error!(error = %e, "unregister failed during shutdown");
    }

    // Wait for tasks to finish
    let _ = tokio::join!(heartbeat_handle, server_handle);

    info!("aether-proxy stopped");
    Ok(())
}

fn init_tracing(config: &Config) {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{reload, EnvFilter};

    let filter = EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| EnvFilter::new("info"));

    let (filter_layer, reload_handle) = reload::Layer::new(filter);

    // Register log-level hot-reloader
    runtime::set_log_reloader(Box::new(move |level: &str| {
        if let Ok(new_filter) = EnvFilter::try_new(level) {
            let _ = reload_handle.modify(|f| *f = new_filter);
        }
    }));

    if config.log_json {
        tracing_subscriber::registry()
            .with(filter_layer)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter_layer)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

async fn wait_for_shutdown() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
