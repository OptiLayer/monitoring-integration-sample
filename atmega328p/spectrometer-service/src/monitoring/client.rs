use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;

use crate::error::SpectrometerError;

/// HTTP client for communicating with OptiMonitor
pub struct MonitoringClient {
    client: Client,
}

#[derive(Debug, Serialize)]
struct SpectralDataPayload {
    calibrated_readings: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wavelengths: Option<Vec<f64>>,
    timestamp: String,
}

impl MonitoringClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Post spectral data to the monitoring API
    ///
    /// For monochromatic spectrometer, calibrated_readings is a single-element array
    pub async fn post_spectral_data(
        &self,
        api_url: &str,
        spectrometer_id: &str,
        calibrated_readings: &[f64],
        wavelengths: Option<&[f64]>,
        timestamp: DateTime<Utc>,
    ) -> Result<(), SpectrometerError> {
        let url = format!("{}/spectrometers/{}/data", api_url, spectrometer_id);

        let payload = SpectralDataPayload {
            calibrated_readings: calibrated_readings.to_vec(),
            wavelengths: wavelengths.map(|w| w.to_vec()),
            timestamp: timestamp.to_rfc3339(),
        };

        let response = self.client.post(&url).json(&payload).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("Failed to post spectral data: {} - {}", status, body);
            return Err(SpectrometerError::DataSource(format!(
                "Monitoring API returned {}",
                status
            )));
        }

        tracing::debug!("Posted spectral data to {}", url);
        Ok(())
    }
}

impl Default for MonitoringClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = MonitoringClient::new();
        // Just verify it doesn't panic
        assert!(true);
    }

    #[test]
    fn test_payload_serialization() {
        let payload = SpectralDataPayload {
            calibrated_readings: vec![45.5],
            wavelengths: Some(vec![550.0]),
            timestamp: "2025-01-15T10:30:00Z".to_string(),
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("45.5"));
        assert!(json.contains("550.0"));
    }

    #[test]
    fn test_payload_without_wavelengths() {
        let payload = SpectralDataPayload {
            calibrated_readings: vec![45.5],
            wavelengths: None,
            timestamp: "2025-01-15T10:30:00Z".to_string(),
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("45.5"));
        assert!(!json.contains("wavelengths")); // Should be skipped
    }
}
