//! Shared application state passed to all subsystems.
//!
//! Consolidates the multiple `Arc<...>` parameters that were previously
//! threaded individually through proxy server, heartbeat, and handlers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio_rustls::TlsAcceptor;

use crate::config::Config;
use crate::hardware::HardwareInfo;
use crate::proxy::delegate_client::DelegateClient;
use crate::proxy::target_filter::DnsCache;
use crate::registration::client::AetherClient;
use crate::runtime::SharedDynamicConfig;

/// Central application state shared across all tasks.
pub struct AppState {
    pub config: Arc<Config>,
    pub node_id: Arc<RwLock<String>>,
    pub dynamic: SharedDynamicConfig,
    pub aether_client: Arc<AetherClient>,
    pub hardware_info: Arc<HardwareInfo>,
    pub public_ip: String,
    pub tls_fingerprint: Option<String>,
    pub tls_acceptor: Option<TlsAcceptor>,
    /// Shared delegate client for proxy-initiated upstream requests.
    pub delegate_client: DelegateClient,
    /// Active connection count for metrics reporting.
    pub active_connections: Arc<AtomicU64>,
    /// Connection concurrency limiter.
    pub connection_semaphore: Arc<Semaphore>,
    /// DNS cache for upstream target resolution.
    pub dns_cache: Arc<DnsCache>,
    /// Request/latency metrics for heartbeat.
    pub metrics: Arc<ProxyMetrics>,
}

/// Aggregate metrics for reporting to Aether.
pub struct ProxyMetrics {
    pub total_requests: AtomicU64,
    pub total_latency_ns: AtomicU64,
}

impl ProxyMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
        }
    }

    pub fn record_request(&self, elapsed: Duration) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ns.fetch_add(nanos, Ordering::Relaxed);
    }
}
