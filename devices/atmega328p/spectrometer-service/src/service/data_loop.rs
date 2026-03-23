use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};

use crate::error::SpectrometerError;
use crate::monitoring::MonitoringClient;
use crate::processing::calibration::{CalibrationProcessor, mean};
use crate::processing::outlier::OutlierExcluder;
use crate::protocol::{MeasurementCycle, ProcessedMeasurement};
use crate::service::calibration::MAX_ADC_VALUE;
use crate::service::state::SharedState;

/// Background data processing loop
pub struct DataProcessingLoop {
    state: SharedState,
    broadcast_tx: broadcast::Sender<serde_json::Value>,
    outlier_excluder: Arc<dyn OutlierExcluder>,
    monitoring_client: MonitoringClient,
    calibrator: CalibrationProcessor,
}

impl DataProcessingLoop {
    pub fn new(
        state: SharedState,
        broadcast_tx: broadcast::Sender<serde_json::Value>,
        outlier_excluder: Box<dyn OutlierExcluder>,
    ) -> Self {
        Self {
            state,
            broadcast_tx,
            outlier_excluder: Arc::from(outlier_excluder),
            monitoring_client: MonitoringClient::new(),
            calibrator: CalibrationProcessor::new(),
        }
    }

    /// Run the processing loop, receiving cycles from the channel
    pub async fn run(
        &self,
        mut cycle_rx: mpsc::Receiver<MeasurementCycle>,
    ) -> Result<(), SpectrometerError> {
        tracing::info!("Data processing loop started");

        while let Some(cycle) = cycle_rx.recv().await {
            let processed = self.process_cycle(&cycle);
            let is_clipped = self.check_clipping(&cycle);

            // Broadcast to WebSocket clients
            let _ = self.broadcast_tx.send(serde_json::json!({
                "type": "cycle",
                "timestamp": processed.timestamp.to_rfc3339(),
                "dark_mean": processed.dark_mean,
                "full_mean": processed.full_mean,
                "sample_mean": processed.sample_mean,
                "calibrated_reading": processed.calibrated_reading,
                "is_clipped": is_clipped,
            }));

            // Update device state
            {
                let mut state = self.state.write().await;
                state.latest_reading = Some(processed.clone());
            }

            // Push to monitoring API if registered
            let should_push = {
                let state = self.state.read().await;
                state.should_process_data()
            };

            if should_push {
                self.push_to_monitoring(&processed).await;
            }
        }

        tracing::info!("Data processing loop finished");
        Ok(())
    }

    /// Check if any raw value in the cycle is at max (clipped/saturated)
    fn check_clipping(&self, cycle: &MeasurementCycle) -> bool {
        cycle.dark.values.contains(&MAX_ADC_VALUE)
            || cycle.full.values.contains(&MAX_ADC_VALUE)
            || cycle.sample.values.contains(&MAX_ADC_VALUE)
    }

    /// Process a single measurement cycle — per-cycle calibration
    fn process_cycle(&self, cycle: &MeasurementCycle) -> ProcessedMeasurement {
        let dark_values = cycle.dark.to_f64();
        let full_values = cycle.full.to_f64();
        let sample_values = cycle.sample.to_f64();

        let dark_filtered = self.outlier_excluder.filter(&dark_values);
        let full_filtered = self.outlier_excluder.filter(&full_values);
        let sample_filtered = self.outlier_excluder.filter(&sample_values);

        let dark_mean = mean(&dark_filtered);
        let full_mean = mean(&full_filtered);
        let sample_mean = mean(&sample_filtered);

        let calibrated = self.calibrator.calculate(dark_mean, full_mean, sample_mean);

        let measurement = ProcessedMeasurement::new(
            cycle.timestamp,
            dark_mean,
            full_mean,
            sample_mean,
            calibrated,
        );

        tracing::debug!(
            "Processed: dark={:.0}, full={:.0}, sample={:.0}, T={:.2}%, clipped={}",
            dark_mean,
            full_mean,
            sample_mean,
            calibrated,
            self.check_clipping(cycle),
        );

        measurement
    }

    /// Push processed measurement to the monitoring API
    async fn push_to_monitoring(&self, measurement: &ProcessedMeasurement) {
        let (api_url, spectrometer_id, control_wavelength) = {
            let state = self.state.read().await;
            (
                state.monitoring_api_url.clone(),
                state.spectrometer_id.clone(),
                state.control_wavelength,
            )
        };

        let Some(api_url) = api_url else {
            return;
        };

        let Some(spec_id) = spectrometer_id else {
            return;
        };

        let result = self
            .monitoring_client
            .post_spectral_data(
                &api_url,
                &spec_id,
                &[measurement.calibrated_reading],
                Some(&[control_wavelength]),
                measurement.timestamp,
            )
            .await;

        if let Err(e) = result {
            tracing::error!("Failed to push data to monitoring: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tokio::sync::broadcast;

    use super::*;
    use crate::processing::outlier::grubbs::GrubbsExcluder;
    use crate::protocol::SeriesData;
    use crate::service::state::create_shared_state;

    fn test_loop() -> DataProcessingLoop {
        let state = create_shared_state();
        let (tx, _) = broadcast::channel(16);
        let excluder = Box::new(GrubbsExcluder::new(0.05));
        DataProcessingLoop::new(state, tx, excluder)
    }

    #[test]
    fn test_process_cycle_valid() {
        let lp = test_loop();
        let cycle = MeasurementCycle::with_timestamp(
            Utc::now(),
            SeriesData::new(vec![100, 101, 102]),
            SeriesData::new(vec![1000, 1001, 1002]),
            SeriesData::new(vec![500, 501, 502]),
        );
        let processed = lp.process_cycle(&cycle);
        assert!(processed.calibrated_reading > 40.0 && processed.calibrated_reading < 50.0);
    }

    #[test]
    fn test_process_cycle_inverted_adc() {
        let lp = test_loop();
        let cycle = MeasurementCycle::with_timestamp(
            Utc::now(),
            SeriesData::new(vec![14_000_000, 14_000_100, 14_000_050]),
            SeriesData::new(vec![300, 310, 305]),
            SeriesData::new(vec![13_000_000, 13_000_100, 13_000_050]),
        );
        let processed = lp.process_cycle(&cycle);
        assert!(processed.calibrated_reading > 0.0);
    }

    #[test]
    fn test_check_clipping() {
        let lp = test_loop();

        let clipped = MeasurementCycle::with_timestamp(
            Utc::now(),
            SeriesData::new(vec![MAX_ADC_VALUE, MAX_ADC_VALUE]),
            SeriesData::new(vec![100, 200]),
            SeriesData::new(vec![MAX_ADC_VALUE, MAX_ADC_VALUE]),
        );
        assert!(lp.check_clipping(&clipped));

        let good = MeasurementCycle::with_timestamp(
            Utc::now(),
            SeriesData::new(vec![14_000_000]),
            SeriesData::new(vec![300]),
            SeriesData::new(vec![13_000_000]),
        );
        assert!(!lp.check_clipping(&good));
    }
}
