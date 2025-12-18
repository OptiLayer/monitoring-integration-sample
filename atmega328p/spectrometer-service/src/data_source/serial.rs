use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::DataSource;
use crate::error::SpectrometerError;
use crate::protocol::{CycleAccumulator, MeasurementCycle, parse_line};

/// Data source for real serial port connection to ATmega328P
pub struct SerialDataSource {
    port_name: String,
    baud_rate: u32,
    gain: u8,
    fadc: f32,
    count: u8,
    is_active: Arc<AtomicBool>,
    reader_task: Option<JoinHandle<()>>,
}

impl SerialDataSource {
    pub fn new(port_name: String, baud_rate: u32, gain: u8, fadc: f32, count: u8) -> Self {
        Self {
            port_name,
            baud_rate,
            gain,
            fadc,
            count,
            is_active: Arc::new(AtomicBool::new(false)),
            reader_task: None,
        }
    }

    /// List available serial ports (helper for CLI)
    pub fn list_available_ports() -> Result<Vec<serialport::SerialPortInfo>, SpectrometerError> {
        serialport::available_ports().map_err(SpectrometerError::SerialPort)
    }
}

#[async_trait]
impl DataSource for SerialDataSource {
    async fn start(&mut self) -> Result<mpsc::Receiver<MeasurementCycle>, SpectrometerError> {
        let mut port = serialport::new(&self.port_name, self.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()?;

        // Send configuration commands before starting reader
        let gain = self.gain;
        let fadc = self.fadc;
        let count = self.count;

        tracing::info!(
            "Configuring device: GAIN={}, FADC={}, COUNT={}",
            gain,
            fadc,
            count
        );

        // Send GAIN command
        let cmd = format!("GAIN={}\n", gain);
        port.write_all(cmd.as_bytes())?;
        port.flush()?;
        std::thread::sleep(Duration::from_millis(50));

        // Send FADC command
        let cmd = format!("FADC={}\n", fadc);
        port.write_all(cmd.as_bytes())?;
        port.flush()?;
        std::thread::sleep(Duration::from_millis(50));

        // Send COUNT command
        let cmd = format!("COUNT={}\n", count);
        port.write_all(cmd.as_bytes())?;
        port.flush()?;
        std::thread::sleep(Duration::from_millis(50));

        tracing::info!("Device configuration sent");

        let (cycle_tx, cycle_rx) = mpsc::channel(32);

        self.is_active.store(true, Ordering::SeqCst);
        let is_active = self.is_active.clone();
        let port_name = self.port_name.clone();

        // Spawn blocking reader task
        let reader_handle = tokio::task::spawn_blocking(move || {
            let mut reader = BufReader::new(port);
            let mut accumulator = CycleAccumulator::new();
            let mut line_buf = String::new();

            tracing::info!("Serial reader started on {}", port_name);

            while is_active.load(Ordering::SeqCst) {
                line_buf.clear();
                match reader.read_line(&mut line_buf) {
                    Ok(0) => continue,
                    Ok(_) => {
                        let parsed = parse_line(&line_buf);
                        if let Some(cycle) = accumulator.process_line(parsed) {
                            if cycle_tx.blocking_send(cycle).is_err() {
                                tracing::warn!("Cycle receiver dropped, stopping reader");
                                break;
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        continue;
                    }
                    Err(e) => {
                        tracing::error!("Serial read error: {}", e);
                        break;
                    }
                }
            }

            tracing::info!("Serial reader stopped");
        });

        self.reader_task = Some(reader_handle);

        Ok(cycle_rx)
    }

    async fn stop(&mut self) -> Result<(), SpectrometerError> {
        self.is_active.store(false, Ordering::SeqCst);

        if let Some(handle) = self.reader_task.take() {
            let _ = handle.await;
        }

        tracing::info!("Serial data source stopped");

        Ok(())
    }

    fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    async fn send_command(&mut self, _command: &str) -> Result<(), SpectrometerError> {
        // Configuration commands (GAIN, FADC, COUNT) are sent during start() before
        // the reader task is spawned. After start(), the port is owned by the reader
        // task and commands cannot be sent.
        Err(SpectrometerError::DataSource(
            "Commands can only be sent during initialization (before start)".into(),
        ))
    }

    fn name(&self) -> &str {
        &self.port_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_data_source_creation_windows_style() {
        // Windows-style port name with default config (GAIN=2, FADC=250, COUNT=4)
        let source = SerialDataSource::new("COM3".to_string(), 38400, 2, 250.0, 4);
        assert_eq!(source.port_name, "COM3");
        assert_eq!(source.baud_rate, 38400);
        assert_eq!(source.gain, 2);
        assert_eq!(source.fadc, 250.0);
        assert_eq!(source.count, 4);
        assert!(!source.is_active());
    }

    #[test]
    fn test_serial_data_source_creation_linux_style() {
        // Linux-style port name with custom config
        let source = SerialDataSource::new("/dev/ttyUSB0".to_string(), 38400, 8, 500.0, 7);
        assert_eq!(source.port_name, "/dev/ttyUSB0");
        assert_eq!(source.baud_rate, 38400);
        assert_eq!(source.gain, 8);
        assert_eq!(source.fadc, 500.0);
        assert_eq!(source.count, 7);
        assert!(!source.is_active());
    }

    #[test]
    fn test_list_ports_doesnt_panic() {
        // Just verify it doesn't panic - actual ports depend on system
        // Returns COM ports on Windows, /dev/tty* on Linux
        let _ = SerialDataSource::list_available_ports();
    }
}
