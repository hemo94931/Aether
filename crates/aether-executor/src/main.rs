use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use aether_executor::server;

#[derive(Parser, Debug)]
#[command(name = "aether-executor", about = "Internal Rust executor for Aether")]
struct Args {
    #[arg(long, env = "AETHER_EXECUTOR_TRANSPORT", default_value = "unix_socket")]
    transport: String,

    #[arg(long, env = "AETHER_EXECUTOR_BIND", default_value = "127.0.0.1:5219")]
    bind: String,

    #[arg(
        long,
        env = "AETHER_EXECUTOR_UNIX_SOCKET",
        default_value = "/tmp/aether-executor.sock"
    )]
    unix_socket: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aether_executor=info".into()),
        )
        .init();

    let args = Args::parse();
    match args.transport.trim().to_ascii_lowercase().as_str() {
        "unix_socket" | "unix" | "uds" => {
            info!(socket = %args.unix_socket.display(), "aether-executor started");
            server::serve_unix(&args.unix_socket).await?;
        }
        "tcp" => {
            info!(bind = %args.bind, "aether-executor started");
            server::serve_tcp(&args.bind).await?;
        }
        other => {
            return Err(format!("unsupported executor transport: {other}").into());
        }
    }

    Ok(())
}
