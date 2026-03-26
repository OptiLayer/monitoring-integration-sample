use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
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
    log_file: Option<PathBuf>,
    is_active: Arc<AtomicBool>,
    reader_task: Option<JoinHandle<()>>,
    cmd_tx: Option<mpsc::Sender<String>>,
}

impl SerialDataSource {
    pub fn new(
        port_name: String,
        baud_rate: u32,
        gain: u8,
        fadc: f32,
        count: u8,
        log_file: Option<PathBuf>,
    ) -> Self {
        Self {
            port_name,
            baud_rate,
            gain,
            fadc,
            count,
            log_file,
            is_active: Arc::new(AtomicBool::new(false)),
            reader_task: None,
            cmd_tx: None,
        }
    }

    /// List available serial ports (helper for CLI)
    pub fn list_available_ports() -> Result<Vec<serialport::SerialPortInfo>, SpectrometerError> {
        serialport::available_ports().map_err(SpectrometerError::SerialPort)
    }

    /// Send initial configuration commands on the port
    fn send_initial_config(
        port: &mut dyn serialport::SerialPort,
        gain: u8,
        fadc: f32,
        count: u8,
    ) -> Result<(), SpectrometerError> {
        tracing::info!("Configuring device: GAIN={gain}, FADC={fadc}, COUNT={count}");

        for cmd in [
            format!("GAIN={gain}\n"),
            format!("FADC={fadc}\n"),
            format!("COUNT={count}\n"),
        ] {
            port.write_all(cmd.as_bytes())?;
            port.flush()?;
            std::thread::sleep(Duration::from_millis(50));
        }

        tracing::info!("Device configuration sent");
        Ok(())
    }
}

#[async_trait]
impl DataSource for SerialDataSource {
    async fn start(&mut self) -> Result<mpsc::Receiver<MeasurementCycle>, SpectrometerError> {
        let mut port = serialport::new(&self.port_name, self.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()?;

        // Send initial configuration
        Self::send_initial_config(port.as_mut(), self.gain, self.fadc, self.count)?;

        // Clone port for writing commands while reader owns the original
        let mut write_port = port.try_clone()?;

        let (cycle_tx, cycle_rx) = mpsc::channel(32);
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(16);

        self.is_active.store(true, Ordering::SeqCst);
        self.cmd_tx = Some(cmd_tx);
        let is_active = self.is_active.clone();
        let port_name = self.port_name.clone();
        let log_file = self.log_file.clone();

        // Spawn blocking reader + command writer task
        let reader_handle = tokio::task::spawn_blocking(move || {
            let mut reader = BufReader::new(port);
            let mut accumulator = CycleAccumulator::new();
            let mut line_buf = String::new();

            let mut log_writer = log_file.and_then(|path| {
                match OpenOptions::new().create(true).append(true).open(&path) {
                    Ok(f) => {
                        tracing::info!("Logging serial output to {:?}", path);
                        Some(std::io::BufWriter::new(f))
                    }
                    Err(e) => {
                        tracing::error!("Failed to open log file {:?}: {e}", path);
                        None
                    }
                }
            });

            tracing::info!("Serial reader started on {}", port_name);

            while is_active.load(Ordering::SeqCst) {
                // Check for pending commands (non-blocking)
                while let Ok(cmd) = cmd_rx.try_recv() {
                    tracing::info!("Sending command: {}", cmd.trim());
                    if let Err(e) = write_port.write_all(cmd.as_bytes()) {
                        tracing::error!("Failed to send command: {e}");
                    }
                    let _ = write_port.flush();
                }

                line_buf.clear();
                match reader.read_line(&mut line_buf) {
                    Ok(0) => continue,
                    Ok(_) => {
                        if let Some(w) = &mut log_writer {
                            let _ = w.write_all(line_buf.as_bytes());
                            let _ = w.flush();
                        }
                        let parsed = parse_line(&line_buf);
                        if let Some(cycle) = accumulator.process_line(parsed)
                            && cycle_tx.blocking_send(cycle).is_err()
                        {
                            tracing::warn!("Cycle receiver dropped, stopping reader");
                            break;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        continue;
                    }
                    Err(e) => {
                        tracing::error!("Serial read error: {e}");
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
        self.cmd_tx = None;

        if let Some(handle) = self.reader_task.take() {
            let _ = handle.await;
        }

        tracing::info!("Serial data source stopped");
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    async fn send_command(&mut self, command: &str) -> Result<(), SpectrometerError> {
        let Some(tx) = &self.cmd_tx else {
            return Err(SpectrometerError::DataSource(
                "Data source not started".into(),
            ));
        };

        let cmd = if command.ends_with('\n') {
            command.to_string()
        } else {
            format!("{command}\n")
        };

        tx.send(cmd)
            .await
            .map_err(|_| SpectrometerError::DataSource("Command channel closed".into()))
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
        let source = SerialDataSource::new("COM3".to_string(), 38400, 2, 250.0, 4, None);
        assert_eq!(source.port_name, "COM3");
        assert_eq!(source.baud_rate, 38400);
        assert_eq!(source.gain, 2);
        assert_eq!(source.fadc, 250.0);
        assert_eq!(source.count, 4);
        assert!(!source.is_active());
        assert!(source.cmd_tx.is_none());
    }

    #[test]
    fn test_serial_data_source_creation_linux_style() {
        let source = SerialDataSource::new("/dev/ttyUSB0".to_string(), 38400, 8, 500.0, 7, None);
        assert_eq!(source.port_name, "/dev/ttyUSB0");
        assert_eq!(source.gain, 8);
        assert_eq!(source.fadc, 500.0);
        assert_eq!(source.count, 7);
        assert!(!source.is_active());
    }

    #[test]
    fn test_list_ports_doesnt_panic() {
        let _ = SerialDataSource::list_available_ports();
    }
}
