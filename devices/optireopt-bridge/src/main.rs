use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use tokio::sync::{RwLock, broadcast};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod api;
mod config;
mod error;
mod monitoring;
mod service;

use config::Cli;
use monitoring::client::MonitoringClient;
use service::source::run_source_loop;
use service::state::{AppState, BridgeConfig, DeviceState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(false))
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "optireopt_bridge=info".into()),
        )
        .init();

    let cli = Cli::parse();

    let bridge_config = BridgeConfig {
        source_url: cli.source.clone(),
        reconnect_ms: cli.reconnect_ms,
    };

    let device_state = Arc::new(RwLock::new(DeviceState::default()));
    let config = Arc::new(RwLock::new(bridge_config));
    let (broadcast_tx, _) = broadcast::channel::<serde_json::Value>(256);
    let monitoring = Arc::new(MonitoringClient::new());

    let app_state = AppState {
        device: device_state.clone(),
        config: config.clone(),
        broadcast_tx: broadcast_tx.clone(),
    };

    // Spawn the source task that subscribes to OptiReOpt's broadcaster.
    let source_handle = tokio::spawn(run_source_loop(
        device_state.clone(),
        config.clone(),
        broadcast_tx.clone(),
        monitoring.clone(),
    ));

    let router = api::create_router(app_state);
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    tracing::info!("OptiReOpt Bridge listening on http://{}", addr);
    tracing::info!(
        "Open http://localhost:{}/ for the live spectrum dashboard",
        cli.port
    );
    tracing::info!("Subscribing to OptiReOpt at {}", cli.source);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    source_handle.abort();
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    tracing::info!("Received shutdown signal");
}
