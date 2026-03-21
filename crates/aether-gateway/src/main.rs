use clap::Parser;
use tracing::info;

use aether_gateway::{serve_tcp, serve_tcp_with_endpoints};

#[derive(Parser, Debug)]
#[command(
    name = "aether-gateway",
    about = "Phase 3a Rust ingress gateway for Aether"
)]
struct Args {
    #[arg(long, env = "AETHER_GATEWAY_BIND", default_value = "0.0.0.0:8084")]
    bind: String,

    #[arg(
        long,
        env = "AETHER_GATEWAY_UPSTREAM",
        default_value = "http://127.0.0.1:18084"
    )]
    upstream: String,

    #[arg(long, env = "AETHER_GATEWAY_CONTROL_URL")]
    control_url: Option<String>,

    #[arg(long, env = "AETHER_GATEWAY_EXECUTOR_URL")]
    executor_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aether_gateway=info".into()),
        )
        .init();

    let args = Args::parse();
    let control_url = args
        .control_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let executor_url = args
        .executor_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    info!(
        bind = %args.bind,
        upstream = %args.upstream,
        control_url = control_url.unwrap_or("-"),
        executor_url = executor_url.unwrap_or("-"),
        "aether-gateway started"
    );
    if executor_url.is_some() {
        serve_tcp_with_endpoints(&args.bind, &args.upstream, control_url, executor_url).await?;
    } else {
        serve_tcp(&args.bind, &args.upstream, control_url).await?;
    }
    Ok(())
}
