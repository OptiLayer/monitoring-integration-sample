use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "optireopt-bridge",
    about = "Virtual spectrometer that ingests pushed scans from OptiReOpt"
)]
pub struct Cli {
    /// Bind address for the HTTP/WS service.
    #[arg(long, env = "BRIDGE_HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// HTTP port. Must match OPTIREOPT_BROADCAST_URL on the OptiReOpt side
    /// (default 8473).
    #[arg(long, env = "BRIDGE_PORT", default_value_t = 8473)]
    pub port: u16,
}
