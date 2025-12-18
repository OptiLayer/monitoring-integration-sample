use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use regex::Regex;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

use super::DataSource;
use crate::error::SpectrometerError;
use crate::protocol::{CycleAccumulator, MeasurementCycle, parse_line};

/// A line from the log file with its timestamp
#[derive(Debug, Clone)]
struct TimestampedLine {
    timestamp: DateTime<Utc>,
    content: String,
}

/// Data source for log file playback with timestamp-based timing
pub struct PlaybackDataSource {
    log_file: PathBuf,
    speed_multiplier: f64,
    loop_playback: bool,
    is_active: Arc<AtomicBool>,
    reader_task: Option<JoinHandle<()>>,
}

impl PlaybackDataSource {
    pub fn new(log_file: PathBuf, speed_multiplier: f64, loop_playback: bool) -> Self {
        Self {
            log_file,
            speed_multiplier: speed_multiplier.max(0.1), // Minimum 0.1x speed
            loop_playback,
            is_active: Arc::new(AtomicBool::new(false)),
            reader_task: None,
        }
    }

    /// Parse a timestamped line from the log file
    /// Format: "2025-01-15T10:30:00.123 SERIES1 = [1234567 1234568 1234569]"
    fn parse_timestamped_line(line: &str) -> Option<TimestampedLine> {
        // Match ISO8601 timestamp at start of line, with optional timezone (Z or +HH:MM)
        let re = Regex::new(
            r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)\s+(.*)$",
        )
        .ok()?;

        let caps = re.captures(line.trim())?;
        let timestamp_str = caps.get(1)?.as_str();
        let content = caps.get(2)?.as_str();

        // Try parsing with timezone
        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                // Try without timezone, assume UTC
                NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%dT%H:%M:%S%.f")
                    .map(|ndt| ndt.and_utc())
            })
            .or_else(|_| {
                // Try without milliseconds
                NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%dT%H:%M:%S")
                    .map(|ndt| ndt.and_utc())
            })
            .ok()?;

        Some(TimestampedLine {
            timestamp,
            content: content.to_string(),
        })
    }
}

#[async_trait]
impl DataSource for PlaybackDataSource {
    async fn start(&mut self) -> Result<mpsc::Receiver<MeasurementCycle>, SpectrometerError> {
        let file = File::open(&self.log_file).await?;
        let reader = BufReader::new(file);

        let (cycle_tx, cycle_rx) = mpsc::channel(32);

        self.is_active.store(true, Ordering::SeqCst);
        let is_active = self.is_active.clone();
        let speed_multiplier = self.speed_multiplier;
        let loop_playback = self.loop_playback;
        let log_file = self.log_file.clone();

        let reader_handle = tokio::spawn(async move {
            tracing::info!(
                "Playback started from {:?} at {}x speed",
                log_file,
                speed_multiplier
            );

            loop {
                // Re-open file for each loop iteration
                let file = match File::open(&log_file).await {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::error!("Failed to open log file: {}", e);
                        break;
                    }
                };

                let reader = BufReader::new(file);
                let mut lines = reader.lines();
                let mut accumulator = CycleAccumulator::new();
                let mut last_timestamp: Option<DateTime<Utc>> = None;
                let playback_start = std::time::Instant::now();
                let mut log_start: Option<DateTime<Utc>> = None;

                while is_active.load(Ordering::SeqCst) {
                    let line = match lines.next_line().await {
                        Ok(Some(line)) => line,
                        Ok(None) => break, // End of file
                        Err(e) => {
                            tracing::error!("Error reading log file: {}", e);
                            break;
                        }
                    };

                    let Some(timestamped) = Self::parse_timestamped_line(&line) else {
                        continue;
                    };

                    // Initialize log start time
                    if log_start.is_none() {
                        log_start = Some(timestamped.timestamp);
                    }

                    // Calculate wait time based on timestamps
                    if let (Some(log_start_time), Some(_last_ts)) = (log_start, last_timestamp) {
                        let log_elapsed =
                            (timestamped.timestamp - log_start_time).num_milliseconds() as f64;
                        let target_elapsed_ms = log_elapsed / speed_multiplier;
                        let actual_elapsed_ms = playback_start.elapsed().as_millis() as f64;

                        let wait_ms = target_elapsed_ms - actual_elapsed_ms;
                        if wait_ms > 0.0 {
                            sleep(Duration::from_millis(wait_ms as u64)).await;
                        }
                    }

                    last_timestamp = Some(timestamped.timestamp);

                    // Parse and process the line content
                    let parsed = parse_line(&timestamped.content);
                    if let Some(cycle) =
                        accumulator.process_line_with_timestamp(parsed, timestamped.timestamp)
                    {
                        if cycle_tx.send(cycle).await.is_err() {
                            tracing::warn!("Cycle receiver dropped, stopping playback");
                            return;
                        }
                    }
                }

                if !loop_playback || !is_active.load(Ordering::SeqCst) {
                    break;
                }

                tracing::info!("Looping playback from start");
            }

            tracing::info!("Playback finished");
        });

        self.reader_task = Some(reader_handle);

        Ok(cycle_rx)
    }

    async fn stop(&mut self) -> Result<(), SpectrometerError> {
        self.is_active.store(false, Ordering::SeqCst);

        if let Some(handle) = self.reader_task.take() {
            handle.abort();
            let _ = handle.await;
        }

        tracing::info!("Playback data source stopped");

        Ok(())
    }

    fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    async fn send_command(&mut self, _command: &str) -> Result<(), SpectrometerError> {
        // Playback mode doesn't support sending commands
        Err(SpectrometerError::DataSource(
            "Cannot send commands in playback mode".into(),
        ))
    }

    fn name(&self) -> &str {
        self.log_file.to_str().unwrap_or("playback")
    }
}

#[cfg(test)]
mod tests {
    use chrono::Timelike;

    use super::*;

    #[test]
    fn test_parse_timestamped_line_with_millis() {
        let line = "2025-01-15T10:30:00.123 SERIES1 = [1234567 1234568 1234569]";
        let result = PlaybackDataSource::parse_timestamped_line(line);

        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.content, "SERIES1 = [1234567 1234568 1234569]");
        assert_eq!(parsed.timestamp.hour(), 10);
        assert_eq!(parsed.timestamp.minute(), 30);
    }

    #[test]
    fn test_parse_timestamped_line_without_millis() {
        let line = "2025-01-15T10:30:00 END_CYCLE";
        let result = PlaybackDataSource::parse_timestamped_line(line);

        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.content, "END_CYCLE");
    }

    #[test]
    fn test_parse_timestamped_line_invalid() {
        // No timestamp
        let result = PlaybackDataSource::parse_timestamped_line("SERIES1 = [100 200 300]");
        assert!(result.is_none());

        // Invalid timestamp format
        let result =
            PlaybackDataSource::parse_timestamped_line("2025/01/15 10:30:00 SERIES1 = [100]");
        assert!(result.is_none());

        // Empty line
        let result = PlaybackDataSource::parse_timestamped_line("");
        assert!(result.is_none());
    }

    #[test]
    fn test_playback_source_creation() {
        let source = PlaybackDataSource::new(PathBuf::from("test.log"), 2.0, true);

        assert_eq!(source.speed_multiplier, 2.0);
        assert!(source.loop_playback);
        assert!(!source.is_active());
    }

    #[test]
    fn test_playback_speed_minimum() {
        // Speed should be clamped to minimum 0.1
        let source = PlaybackDataSource::new(PathBuf::from("test.log"), 0.01, false);

        assert_eq!(source.speed_multiplier, 0.1);
    }

    #[test]
    fn test_parse_different_timestamp_formats() {
        // With microseconds
        let line = "2025-01-15T10:30:00.123456 SERIES1 = [100]";
        assert!(PlaybackDataSource::parse_timestamped_line(line).is_some());

        // ISO8601 with timezone (Z)
        let line = "2025-01-15T10:30:00.123Z SERIES1 = [100]";
        assert!(PlaybackDataSource::parse_timestamped_line(line).is_some());

        // ISO8601 with timezone offset
        let line = "2025-01-15T10:30:00.123+00:00 SERIES1 = [100]";
        assert!(PlaybackDataSource::parse_timestamped_line(line).is_some());
    }
}
