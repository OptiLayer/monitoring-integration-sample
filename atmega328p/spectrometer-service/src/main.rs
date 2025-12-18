use std::net::SocketAddr;

use clap::Parser;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod api;
mod config;
mod data_source;
mod error;
mod monitoring;
mod processing;
mod protocol;
mod service;

use config::Cli;
use data_source::serial::SerialDataSource;
use service::data_loop::DataProcessingLoop;
use service::state::create_shared_state;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with colors and stderr output
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(true)
                .with_writer(std::io::stderr),
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "spectrometer_service=info".into()),
        )
        .init();

    let cli = Cli::parse();

    // Handle --list-ports
    if cli.list_ports {
        list_serial_ports();
        return Ok(());
    }

    // Require a mode if not listing ports
    let Some(data_source_config) = cli.to_data_source_config() else {
        eprintln!("Error: Please specify a mode (serial or playback)");
        eprintln!("Use --help for usage information");
        std::process::exit(1);
    };

    tracing::info!(
        "Starting spectrometer service on {}:{}",
        cli.host,
        cli.listen
    );

    // Create shared state
    let state = create_shared_state();

    // Create data source
    let mut data_source = data_source_config.create_source();

    // Create outlier excluder
    let outlier_method = cli.to_outlier_method();
    let outlier_excluder = outlier_method.create();

    tracing::info!("Using {} outlier exclusion", outlier_excluder.name());

    // Start data source and get cycle receiver
    let cycle_rx = data_source.start().await?;

    // Create and spawn data processing loop
    let processing_loop = DataProcessingLoop::new(state.clone(), outlier_excluder);

    let processing_handle = tokio::spawn(async move {
        if let Err(e) = processing_loop.run(cycle_rx).await {
            tracing::error!("Data processing loop error: {}", e);
        }
    });

    // Create and run HTTP server
    let router = api::create_router(state);
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.listen).parse()?;

    tracing::info!("HTTP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Run server with graceful shutdown
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Cleanup
    tracing::info!("Shutting down...");
    data_source.stop().await?;
    processing_handle.abort();

    Ok(())
}

/// List available serial ports
fn list_serial_ports() {
    match SerialDataSource::list_available_ports() {
        Ok(ports) => {
            if ports.is_empty() {
                println!("No serial ports found");
            } else {
                println!("Available serial ports:");
                for port in ports {
                    let port_type = match port.port_type {
                        serialport::SerialPortType::UsbPort(info) => {
                            format!(
                                "USB - {}",
                                info.product.unwrap_or_else(|| "Unknown".to_string())
                            )
                        }
                        serialport::SerialPortType::BluetoothPort => "Bluetooth".to_string(),
                        serialport::SerialPortType::PciPort => "PCI".to_string(),
                        serialport::SerialPortType::Unknown => "Unknown".to_string(),
                    };
                    println!("  {} - {}", port.port_name, port_type);
                }
            }
        }
        Err(e) => {
            eprintln!("Error listing serial ports: {}", e);
        }
    }
}

/// Wait for shutdown signal (Ctrl+C)
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    tracing::info!("Received shutdown signal");
}
