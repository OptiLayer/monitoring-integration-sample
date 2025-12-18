use std::sync::Arc;

use tokio::sync::mpsc;

use crate::error::SpectrometerError;
use crate::monitoring::MonitoringClient;
use crate::processing::calibration::{CalibrationProcessor, mean};
use crate::processing::outlier::OutlierExcluder;
use crate::processing::validation::MeasurementValidator;
use crate::protocol::{MeasurementCycle, ProcessedMeasurement};
use crate::service::state::SharedState;

/// Background data processing loop
pub struct DataProcessingLoop {
    state: SharedState,
    outlier_excluder: Arc<dyn OutlierExcluder>,
    monitoring_client: MonitoringClient,
    calibrator: CalibrationProcessor,
    validator: MeasurementValidator,
}

impl DataProcessingLoop {
    pub fn new(state: SharedState, outlier_excluder: Box<dyn OutlierExcluder>) -> Self {
        Self {
            state,
            outlier_excluder: Arc::from(outlier_excluder),
            monitoring_client: MonitoringClient::new(),
            calibrator: CalibrationProcessor::new(),
            validator: MeasurementValidator::new(),
        }
    }

    /// Run the processing loop, receiving cycles from the channel
    pub async fn run(
        &self,
        mut cycle_rx: mpsc::Receiver<MeasurementCycle>,
    ) -> Result<(), SpectrometerError> {
        tracing::info!("Data processing loop started");

        while let Some(cycle) = cycle_rx.recv().await {
            // Check if we should process
            let should_process = {
                let state = self.state.read().await;
                state.should_process_data()
            };

            if !should_process {
                tracing::trace!("Skipping cycle - not processing");
                continue;
            }

            // Process the measurement cycle
            let processed = self.process_cycle(&cycle);

            // Update state with latest reading
            {
                let mut state = self.state.write().await;
                state.latest_reading = Some(processed.clone());
            }

            // Push to monitoring API if registered and valid
            if processed.is_valid {
                self.push_to_monitoring(&processed).await;
            } else {
                tracing::warn!("Invalid measurement: {:?}", processed.validation_error);
            }
        }

        tracing::info!("Data processing loop finished");
        Ok(())
    }

    /// Process a single measurement cycle
    fn process_cycle(&self, cycle: &MeasurementCycle) -> ProcessedMeasurement {
        // Convert to f64 for processing
        let dark_values = cycle.dark.to_f64();
        let full_values = cycle.full.to_f64();
        let sample_values = cycle.sample.to_f64();

        // Apply outlier exclusion
        let dark_filtered = self.outlier_excluder.filter(&dark_values);
        let full_filtered = self.outlier_excluder.filter(&full_values);
        let sample_filtered = self.outlier_excluder.filter(&sample_values);

        // Calculate means
        let dark_mean = mean(&dark_filtered);
        let full_mean = mean(&full_filtered);
        let sample_mean = mean(&sample_filtered);

        // Validate: full > sample > dark
        let validation_result = self.validator.validate(dark_mean, full_mean, sample_mean);

        // Calculate calibrated reading
        let calibrated = self.calibrator.calculate(dark_mean, full_mean, sample_mean);

        let mut measurement = ProcessedMeasurement::new(
            cycle.timestamp,
            dark_mean,
            full_mean,
            sample_mean,
            calibrated,
        );

        if let Err(error) = validation_result {
            measurement = measurement.with_error(error);
        }

        tracing::debug!(
            "Processed: dark={:.0}, full={:.0}, sample={:.0}, calibrated={:.2}%, valid={}",
            dark_mean,
            full_mean,
            sample_mean,
            calibrated,
            measurement.is_valid
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
            tracing::trace!("Not registered - skipping push");
            return;
        };

        let Some(spec_id) = spectrometer_id else {
            tracing::trace!("No spectrometer ID - skipping push");
            return;
        };

        // For monochromatic spectrometer, send single-point array
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
            tracing::error!("Failed to push data to monitoring: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::processing::outlier::grubbs::GrubbsExcluder;
    use crate::protocol::SeriesData;
    use crate::service::state::create_shared_state;

    #[test]
    fn test_process_cycle_valid() {
        let state = create_shared_state();
        let excluder = Box::new(GrubbsExcluder::new(0.05));
        let loop_processor = DataProcessingLoop::new(state, excluder);

        let cycle = MeasurementCycle::with_timestamp(
            Utc::now(),
            SeriesData::new(vec![100, 101, 102]),    // dark
            SeriesData::new(vec![1000, 1001, 1002]), // full
            SeriesData::new(vec![500, 501, 502]),    // sample
        );

        let processed = loop_processor.process_cycle(&cycle);

        assert!(processed.is_valid);
        assert!(processed.calibrated_reading > 40.0 && processed.calibrated_reading < 50.0);
    }

    #[test]
    fn test_process_cycle_invalid() {
        let state = create_shared_state();
        let excluder = Box::new(GrubbsExcluder::new(0.05));
        let loop_processor = DataProcessingLoop::new(state, excluder);

        // sample > full - invalid
        let cycle = MeasurementCycle::with_timestamp(
            Utc::now(),
            SeriesData::new(vec![100, 101, 102]),
            SeriesData::new(vec![500, 501, 502]), // full
            SeriesData::new(vec![600, 601, 602]), // sample > full!
        );

        let processed = loop_processor.process_cycle(&cycle);

        assert!(!processed.is_valid);
        assert!(processed.validation_error.is_some());
    }

    #[test]
    fn test_process_cycle_with_outlier() {
        let state = create_shared_state();
        let excluder = Box::new(GrubbsExcluder::new(0.05));
        let loop_processor = DataProcessingLoop::new(state, excluder);

        // Include an obvious outlier in sample
        let cycle = MeasurementCycle::with_timestamp(
            Utc::now(),
            SeriesData::new(vec![100, 101, 102]),
            SeriesData::new(vec![1000, 1001, 1002]),
            SeriesData::new(vec![500, 501, 502, 9999]), // 9999 is outlier
        );

        let processed = loop_processor.process_cycle(&cycle);

        // Should still be valid after outlier removal
        assert!(processed.is_valid);
        // Calibrated reading should be close to 44.4% (not affected by outlier)
        assert!(processed.calibrated_reading > 40.0 && processed.calibrated_reading < 50.0);
    }
}
