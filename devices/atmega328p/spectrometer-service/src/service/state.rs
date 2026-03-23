use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};

use crate::protocol::ProcessedMeasurement;
use crate::service::calibration::SharedConfig;

/// Application state for the spectrometer service
#[derive(Debug, Clone)]
pub struct DeviceState {
    pub monitoring_api_url: Option<String>,
    pub spectrometer_id: Option<String>,
    pub vacuum_chamber_id: Option<String>,
    pub control_wavelength: f64,
    pub is_running: bool,
    pub current_material: String,
    pub is_depositing: bool,
    pub latest_reading: Option<ProcessedMeasurement>,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            monitoring_api_url: None,
            spectrometer_id: None,
            vacuum_chamber_id: None,
            control_wavelength: 550.0,
            is_running: false,
            current_material: "H".to_string(),
            is_depositing: false,
            latest_reading: None,
        }
    }
}

impl DeviceState {
    #[allow(dead_code)]
    pub fn is_registered(&self) -> bool {
        self.monitoring_api_url.is_some() && self.spectrometer_id.is_some()
    }

    pub fn should_process_data(&self) -> bool {
        self.is_running || self.is_depositing
    }
}

pub type SharedState = Arc<RwLock<DeviceState>>;

pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(DeviceState::default()))
}

/// Composite application state for axum handlers
#[derive(Clone)]
pub struct AppState {
    pub device: SharedState,
    pub config: SharedConfig,
    pub broadcast_tx: broadcast::Sender<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_state_default() {
        let state = DeviceState::default();
        assert!(state.monitoring_api_url.is_none());
        assert_eq!(state.control_wavelength, 550.0);
        assert!(!state.is_running);
        assert_eq!(state.current_material, "H");
    }

    #[test]
    fn test_is_registered() {
        let mut state = DeviceState::default();
        assert!(!state.is_registered());
        state.monitoring_api_url = Some("http://localhost:8200".to_string());
        state.spectrometer_id = Some("test-id".to_string());
        assert!(state.is_registered());
    }

    #[test]
    fn test_should_process_data() {
        let mut state = DeviceState::default();
        assert!(!state.should_process_data());
        state.is_running = true;
        assert!(state.should_process_data());
    }
}
