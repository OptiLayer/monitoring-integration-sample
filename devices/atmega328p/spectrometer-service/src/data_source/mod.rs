pub mod playback;
pub mod serial;

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::SpectrometerError;
use crate::protocol::MeasurementCycle;

/// Trait for abstracting data sources (real hardware vs playback)
#[allow(dead_code)]
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Start the data source and return a channel receiver for measurement cycles
    async fn start(&mut self) -> Result<mpsc::Receiver<MeasurementCycle>, SpectrometerError>;

    /// Stop the data source
    async fn stop(&mut self) -> Result<(), SpectrometerError>;

    /// Check if data source is active
    fn is_active(&self) -> bool;

    /// Send a command to the device (only applicable for real hardware)
    async fn send_command(&mut self, command: &str) -> Result<(), SpectrometerError>;

    /// Get the name of this data source for logging
    fn name(&self) -> &str;
}

/// Configuration for creating data sources
#[derive(Debug, Clone)]
pub enum DataSourceConfig {
    /// Real serial port connection
    Serial {
        port: String,
        baud_rate: u32,
        gain: u8,
        fadc: f32,
        count: u8,
    },
    /// Log file playback (supports both timestamped and raw log formats)
    Playback {
        log_file: PathBuf,
        speed_multiplier: f64,
        loop_playback: bool,
        /// Cycle interval in ms for raw logs without timestamps (default: 100)
        cycle_interval_ms: u64,
    },
}

impl DataSourceConfig {
    /// Create a data source from this configuration
    pub fn create_source(&self) -> Box<dyn DataSource> {
        match self {
            DataSourceConfig::Serial {
                port,
                baud_rate,
                gain,
                fadc,
                count,
            } => Box::new(serial::SerialDataSource::new(
                port.clone(),
                *baud_rate,
                *gain,
                *fadc,
                *count,
            )),
            DataSourceConfig::Playback {
                log_file,
                speed_multiplier,
                loop_playback,
                cycle_interval_ms,
            } => Box::new(playback::PlaybackDataSource::new_raw(
                log_file.clone(),
                *speed_multiplier,
                *loop_playback,
                *cycle_interval_ms,
            )),
        }
    }
}
