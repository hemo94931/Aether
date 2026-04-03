use serde::Serialize;
use sysinfo::System;
use tracing::info;

/// Hardware information collected at startup.
///
/// The struct is `Serialize`-able so it can be sent directly as the
/// `hardware_info` JSON bag in the registration request.  New fields
/// can be added without database schema migrations.
#[derive(Debug, Clone, Serialize)]
pub struct HardwareInfo {
    pub cpu_cores: u32,
    pub total_memory_mb: u64,
    pub os_info: String,
    pub fd_limit: u64,
    #[serde(skip)]
    pub estimated_max_concurrency: u64,
}

/// Collect hardware information and estimate max concurrency.
///
/// Should be called once at startup -- hardware does not change at runtime.
pub fn collect() -> HardwareInfo {
    let sys = System::new_all();

    let cpu_cores = sys.cpus().len() as u32;
    let total_memory_mb = sys.total_memory() / (1024 * 1024);
    let os_info = format!(
        "{} {}",
        System::name().unwrap_or_else(|| "Unknown".into()),
        System::os_version().unwrap_or_default(),
    )
    .trim()
    .to_string();

    // Estimate max concurrent connections:
    //   - Each tokio async task uses ~8-16 KB stack + heap buffers
    //   - OS file descriptor limit is often the real bottleneck
    //   - Conservative formula: min(fd_limit - 100, ram_mb * 40, cpu_cores * 2000)
    let fd_limit = get_fd_limit();
    let by_fd = fd_limit.saturating_sub(100);
    let by_ram = total_memory_mb.saturating_mul(40);
    let by_cpu = (cpu_cores as u64).saturating_mul(2000);
    let estimated_max_concurrency = by_fd.min(by_ram).min(by_cpu);

    info!(
        cpu_cores,
        total_memory_mb,
        os_info = %os_info,
        fd_limit,
        estimated_max_concurrency,
        "hardware info collected"
    );

    HardwareInfo {
        cpu_cores,
        total_memory_mb,
        os_info,
        fd_limit,
        estimated_max_concurrency,
    }
}

/// Read the soft file-descriptor limit (RLIMIT_NOFILE).
fn get_fd_limit() -> u64 {
    #[cfg(unix)]
    {
        let mut rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let ret = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) };
        if ret == 0 {
            return rlim.rlim_cur;
        }
    }
    // Fallback for non-unix or error
    1024
}
