use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use regex::Regex;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

use super::DataSource;
use crate::error::SpectrometerError;
use crate::protocol::{CycleAccumulator, MeasurementCycle, ParsedLine, parse_line};

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
    cycle_interval_ms: u64,
    is_active: Arc<AtomicBool>,
    reader_task: Option<JoinHandle<()>>,
    log_tx: Option<mpsc::Sender<String>>,
}

impl PlaybackDataSource {
    #[allow(dead_code)]
    pub fn new(log_file: PathBuf, speed_multiplier: f64, loop_playback: bool) -> Self {
        Self {
            log_file,
            speed_multiplier: speed_multiplier.max(0.1),
            loop_playback,
            cycle_interval_ms: 100, // default: 100ms between cycles
            is_active: Arc::new(AtomicBool::new(false)),
            reader_task: None,
            log_tx: None,
        }
    }

    /// Create a playback source for raw log files (no timestamps).
    /// `cycle_interval_ms` controls the delay between emitted cycles.
    pub fn new_raw(
        log_file: PathBuf,
        speed_multiplier: f64,
        loop_playback: bool,
        cycle_interval_ms: u64,
    ) -> Self {
        Self {
            log_file,
            speed_multiplier: speed_multiplier.max(0.1),
            loop_playback,
            cycle_interval_ms,
            is_active: Arc::new(AtomicBool::new(false)),
            reader_task: None,
            log_tx: None,
        }
    }

    /// Parse a timestamped line from the log file
    /// Format: "2025-01-15T10:30:00.123 SERIES1 = [1234567 1234568 1234569]"
    fn parse_timestamped_line(line: &str) -> Option<TimestampedLine> {
        let re = Regex::new(
            r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)\s+(.*)$",
        )
        .ok()?;

        let caps = re.captures(line.trim())?;
        let timestamp_str = caps.get(1)?.as_str();
        let content = caps.get(2)?.as_str();

        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%dT%H:%M:%S%.f")
                    .map(|ndt| ndt.and_utc())
            })
            .or_else(|_| {
                NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%dT%H:%M:%S")
                    .map(|ndt| ndt.and_utc())
            })
            .ok()?;

        Some(TimestampedLine {
            timestamp,
            content: content.to_string(),
        })
    }

    /// Detect whether the file has ISO8601 timestamps by checking first few data lines
    async fn detect_has_timestamps(file_path: &PathBuf) -> bool {
        let file = match File::open(file_path).await {
            Ok(f) => f,
            Err(_) => return false,
        };

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut checked = 0;

        while checked < 10 {
            let line = match lines.next_line().await {
                Ok(Some(line)) => line,
                _ => break,
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Skip PuTTY header and non-data lines
            let parsed = parse_line(trimmed);
            if !matches!(parsed, ParsedLine::Series { .. } | ParsedLine::EndCycle) {
                continue;
            }

            checked += 1;
            // If any data line has a timestamp, assume timestamped format
            if Self::parse_timestamped_line(trimmed).is_some() {
                return true;
            }
        }

        false
    }

    /// Run timestamped playback (original behavior)
    async fn run_timestamped(
        log_file: PathBuf,
        speed_multiplier: f64,
        loop_playback: bool,
        is_active: Arc<AtomicBool>,
        cycle_tx: mpsc::Sender<MeasurementCycle>,
        log_tx: Option<mpsc::Sender<String>>,
    ) {
        tracing::info!(
            "Timestamped playback from {:?} at {}x speed",
            log_file,
            speed_multiplier
        );

        loop {
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
                    Ok(None) => break,
                    Err(e) => {
                        tracing::error!("Error reading log file: {}", e);
                        break;
                    }
                };

                let Some(timestamped) = Self::parse_timestamped_line(&line) else {
                    continue;
                };

                if log_start.is_none() {
                    log_start = Some(timestamped.timestamp);
                }

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

                if let Some(tx) = &log_tx {
                    let _ = tx.send(timestamped.content.clone()).await;
                }
                let parsed = parse_line(&timestamped.content);
                if let Some(cycle) =
                    accumulator.process_line_with_timestamp(parsed, timestamped.timestamp)
                    && cycle_tx.send(cycle).await.is_err()
                {
                    tracing::warn!("Cycle receiver dropped, stopping playback");
                    return;
                }
            }

            if !loop_playback || !is_active.load(Ordering::SeqCst) {
                break;
            }

            tracing::info!("Looping playback from start");
        }

        tracing::info!("Timestamped playback finished");
    }

    /// Run raw playback for log files without timestamps.
    /// Generates synthetic timestamps and paces cycles at cycle_interval_ms.
    async fn run_raw(
        log_file: PathBuf,
        speed_multiplier: f64,
        cycle_interval_ms: u64,
        loop_playback: bool,
        is_active: Arc<AtomicBool>,
        cycle_tx: mpsc::Sender<MeasurementCycle>,
        log_tx: Option<mpsc::Sender<String>>,
    ) {
        let effective_interval_ms = (cycle_interval_ms as f64 / speed_multiplier) as u64;
        tracing::info!(
            "Raw playback from {:?} at {}x speed ({}ms between cycles)",
            log_file,
            speed_multiplier,
            effective_interval_ms
        );

        loop {
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
            let mut cycle_count: u64 = 0;
            let base_timestamp = Utc::now();

            while is_active.load(Ordering::SeqCst) {
                let line = match lines.next_line().await {
                    Ok(Some(line)) => line,
                    Ok(None) => break,
                    Err(e) => {
                        tracing::error!("Error reading log file: {}", e);
                        break;
                    }
                };

                let trimmed = line.trim().to_string();
                if let Some(tx) = &log_tx {
                    let _ = tx.send(trimmed.clone()).await;
                }
                let parsed = parse_line(&trimmed);

                // Generate a synthetic timestamp for this cycle
                let synthetic_ts = base_timestamp
                    + ChronoDuration::milliseconds((cycle_count * cycle_interval_ms) as i64);

                if let Some(cycle) = accumulator.process_line_with_timestamp(parsed, synthetic_ts) {
                    cycle_count += 1;

                    // Pace the output
                    if effective_interval_ms > 0 {
                        sleep(Duration::from_millis(effective_interval_ms)).await;
                    }

                    if cycle_tx.send(cycle).await.is_err() {
                        tracing::warn!("Cycle receiver dropped, stopping playback");
                        return;
                    }
                }
            }

            tracing::info!("Raw playback: emitted {} cycles", cycle_count);

            if !loop_playback || !is_active.load(Ordering::SeqCst) {
                break;
            }

            tracing::info!("Looping playback from start");
        }

        tracing::info!("Raw playback finished");
    }
}

#[async_trait]
impl DataSource for PlaybackDataSource {
    async fn start(&mut self) -> Result<mpsc::Receiver<MeasurementCycle>, SpectrometerError> {
        if !self.log_file.exists() {
            return Err(SpectrometerError::DataSource(format!(
                "Log file not found: {:?}",
                self.log_file
            )));
        }

        let (cycle_tx, cycle_rx) = mpsc::channel(32);

        self.is_active.store(true, Ordering::SeqCst);
        let is_active = self.is_active.clone();
        let speed_multiplier = self.speed_multiplier;
        let loop_playback = self.loop_playback;
        let log_file = self.log_file.clone();
        let cycle_interval_ms = self.cycle_interval_ms;
        let log_tx = self.log_tx.clone();

        // Auto-detect whether file has timestamps
        let has_timestamps = Self::detect_has_timestamps(&log_file).await;

        let reader_handle = if has_timestamps {
            tracing::info!("Detected timestamped log format");
            tokio::spawn(async move {
                Self::run_timestamped(
                    log_file,
                    speed_multiplier,
                    loop_playback,
                    is_active,
                    cycle_tx,
                    log_tx,
                )
                .await;
            })
        } else {
            tracing::info!("Detected raw log format (no timestamps)");
            let log_tx2 = log_tx;
            tokio::spawn(async move {
                Self::run_raw(
                    log_file,
                    speed_multiplier,
                    cycle_interval_ms,
                    loop_playback,
                    is_active,
                    cycle_tx,
                    log_tx2,
                )
                .await;
            })
        };

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
        Err(SpectrometerError::DataSource(
            "Cannot send commands in playback mode".into(),
        ))
    }

    fn name(&self) -> &str {
        self.log_file.to_str().unwrap_or("playback")
    }

    fn set_log_channel(&mut self, tx: mpsc::Sender<String>) {
        self.log_tx = Some(tx);
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
        let result = PlaybackDataSource::parse_timestamped_line("SERIES1 = [100 200 300]");
        assert!(result.is_none());

        let result =
            PlaybackDataSource::parse_timestamped_line("2025/01/15 10:30:00 SERIES1 = [100]");
        assert!(result.is_none());

        let result = PlaybackDataSource::parse_timestamped_line("");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_raw_line_no_timestamp() {
        // Raw putty.log lines should NOT parse as timestamped
        let result =
            PlaybackDataSource::parse_timestamped_line("SERIES1 = 16777215 16777215 16777215");
        assert!(result.is_none());

        let result = PlaybackDataSource::parse_timestamped_line("END_CYCLE");
        assert!(result.is_none());

        let result = PlaybackDataSource::parse_timestamped_line("GAIN=4");
        assert!(result.is_none());
    }

    #[test]
    fn test_playback_source_creation() {
        let source = PlaybackDataSource::new(PathBuf::from("test.log"), 2.0, true);

        assert_eq!(source.speed_multiplier, 2.0);
        assert!(source.loop_playback);
        assert!(!source.is_active());
        assert_eq!(source.cycle_interval_ms, 100);
    }

    #[test]
    fn test_playback_source_raw_creation() {
        let source = PlaybackDataSource::new_raw(PathBuf::from("test.log"), 1.0, true, 200);

        assert_eq!(source.cycle_interval_ms, 200);
        assert!(source.loop_playback);
    }

    #[test]
    fn test_playback_speed_minimum() {
        let source = PlaybackDataSource::new(PathBuf::from("test.log"), 0.01, false);
        assert_eq!(source.speed_multiplier, 0.1);

        let source = PlaybackDataSource::new_raw(PathBuf::from("test.log"), 0.01, false, 100);
        assert_eq!(source.speed_multiplier, 0.1);
    }

    #[test]
    fn test_parse_different_timestamp_formats() {
        let line = "2025-01-15T10:30:00.123456 SERIES1 = [100]";
        assert!(PlaybackDataSource::parse_timestamped_line(line).is_some());

        let line = "2025-01-15T10:30:00.123Z SERIES1 = [100]";
        assert!(PlaybackDataSource::parse_timestamped_line(line).is_some());

        let line = "2025-01-15T10:30:00.123+00:00 SERIES1 = [100]";
        assert!(PlaybackDataSource::parse_timestamped_line(line).is_some());
    }
}
