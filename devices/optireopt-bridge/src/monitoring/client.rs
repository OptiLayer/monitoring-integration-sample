use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;

use crate::error::BridgeError;

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

    pub async fn post_spectral_data(
        &self,
        api_url: &str,
        spectrometer_id: &str,
        readings: &[f64],
        wavelengths: Option<&[f64]>,
        timestamp: DateTime<Utc>,
    ) -> Result<(), BridgeError> {
        let url = format!("{}/spectrometers/{}/data", api_url, spectrometer_id);
        let payload = SpectralDataPayload {
            calibrated_readings: readings.to_vec(),
            wavelengths: wavelengths.map(|w| w.to_vec()),
            timestamp: timestamp.to_rfc3339(),
        };

        let response = self.client.post(&url).json(&payload).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::Monitoring(format!("{} - {}", status, body)));
        }
        Ok(())
    }
}

impl Default for MonitoringClient {
    fn default() -> Self {
        Self::new()
    }
}
