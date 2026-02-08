mod app;
mod auth;
mod config;
mod hardware;
mod net;
mod proxy;
mod registration;
mod runtime;
mod setup;
mod state;

use std::path::PathBuf;

use clap::Parser;

use config::Config;

/// Default config file name.
const DEFAULT_CONFIG: &str = "aether-proxy.toml";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle subcommands before clap parsing (these don't need Config)
    if args.len() > 1 {
        match args[1].as_str() {
            "setup" => {
                let path = args
                    .get(2)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG));
                return setup::run(path);
            }
            "start" => return setup::service::cmd_start(),
            "status" => return setup::service::cmd_status(),
            "logs" => return setup::service::cmd_logs(),
            "restart" => return setup::service::cmd_restart(),
            "stop" => return setup::service::cmd_stop(),
            "uninstall" => return setup::service::cmd_uninstall(),
            "upgrade" => {
                let version = args.get(2).cloned();
                return setup::upgrade::cmd_upgrade(version).await;
            }
            _ => {} // fall through to clap (--help, --version, config args)
        }
    }

    // Load config file as env-var defaults (before clap)
    let config_file_path =
        std::env::var("AETHER_PROXY_CONFIG").unwrap_or_else(|_| DEFAULT_CONFIG.to_string());
    if std::path::Path::new(&config_file_path).exists() {
        if let Ok(file_cfg) = config::ConfigFile::load(std::path::Path::new(&config_file_path)) {
            file_cfg.inject_env();
        }
    }

    // Parse config; fall back to setup TUI if required args are missing
    let config = match Config::try_parse() {
        Ok(c) => c,
        Err(e) => {
            if e.kind() == clap::error::ErrorKind::MissingRequiredArgument {
                eprintln!("Missing required config, launching setup wizard...\n");
                return setup::run(PathBuf::from(&config_file_path));
            }
            e.exit();
        }
    };

    // Warn if systemd service is already running (would cause port conflict).
    // Skip this check when we ARE the systemd service (INVOCATION_ID is set by systemd).
    if std::env::var_os("INVOCATION_ID").is_none() && setup::service::is_service_active() {
        eprintln!("Warning: systemd service is already running.");
        eprintln!("Use `aether-proxy stop` to stop it first, or manage via subcommands:");
        eprintln!("  aether-proxy status / logs / restart / stop");
        std::process::exit(1);
    }

    app::run(config).await
}
