use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "optireopt-bridge",
    about = "Virtual spectrometer fed by OptiReOpt broadcasts"
)]
pub struct Cli {
    /// Bind address for the HTTP/WS service.
    #[arg(long, env = "BRIDGE_HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// HTTP port (must match what OptiMonitor expects from a spectrometer device).
    #[arg(long, env = "BRIDGE_PORT", default_value_t = 8100)]
    pub port: u16,

    /// WebSocket URL of the OptiReOpt broadcaster.
    #[arg(long, env = "BRIDGE_SOURCE", default_value = "ws://127.0.0.1:9100")]
    pub source: String,

    /// Reconnect backoff base (ms). Backoff caps at 10x this value.
    #[arg(long, env = "BRIDGE_RECONNECT_MS", default_value_t = 1000)]
    pub reconnect_ms: u64,
}
