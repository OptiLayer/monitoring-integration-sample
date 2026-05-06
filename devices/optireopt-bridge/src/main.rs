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
use service::state::{AppState, DeviceState};

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

    let device_state = Arc::new(RwLock::new(DeviceState::default()));
    let (broadcast_tx, _) = broadcast::channel::<serde_json::Value>(256);
    let monitoring = Arc::new(MonitoringClient::new());

    let app_state = AppState {
        device: device_state,
        broadcast_tx,
        monitoring,
    };

    let router = api::create_router(app_state);
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    tracing::info!("OptiReOpt Bridge listening on http://{}", addr);
    tracing::info!(
        "Open http://localhost:{}/ for the live spectrum dashboard",
        cli.port
    );
    tracing::info!(
        "Set OPTIREOPT_BROADCAST_URL=http://127.0.0.1:{}/ingest on the OptiReOpt side",
        cli.port
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    tracing::info!("Received shutdown signal");
}
