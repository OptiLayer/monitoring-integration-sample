use std::sync::Arc;

use tokio::sync::RwLock;

use crate::protocol::ProcessedMeasurement;

/// Application state for the spectrometer service
#[derive(Debug, Clone)]
pub struct DeviceState {
    // Registration from OptiMonitor
    pub monitoring_api_url: Option<String>,
    pub spectrometer_id: Option<String>,
    pub vacuum_chamber_id: Option<String>,

    // Spectrometer state
    pub control_wavelength: f64,
    pub is_running: bool,

    // Vacuum chamber state
    pub current_material: String,
    pub is_depositing: bool,

    // Latest processed data
    pub latest_reading: Option<ProcessedMeasurement>,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            monitoring_api_url: None,
            spectrometer_id: None,
            vacuum_chamber_id: None,
            control_wavelength: 550.0, // Default wavelength in nm
            is_running: false,
            current_material: "H".to_string(),
            is_depositing: false,
            latest_reading: None,
        }
    }
}

impl DeviceState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the device is registered with monitoring API
    pub fn is_registered(&self) -> bool {
        self.monitoring_api_url.is_some() && self.spectrometer_id.is_some()
    }

    /// Check if we should be processing data
    pub fn should_process_data(&self) -> bool {
        self.is_running || self.is_depositing
    }
}

/// Thread-safe shared state
pub type SharedState = Arc<RwLock<DeviceState>>;

/// Create a new shared state instance
pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(DeviceState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_state_default() {
        let state = DeviceState::default();

        assert!(state.monitoring_api_url.is_none());
        assert!(state.spectrometer_id.is_none());
        assert_eq!(state.control_wavelength, 550.0);
        assert!(!state.is_running);
        assert_eq!(state.current_material, "H");
        assert!(!state.is_depositing);
    }

    #[test]
    fn test_is_registered() {
        let mut state = DeviceState::default();

        assert!(!state.is_registered());

        state.monitoring_api_url = Some("http://localhost:8200".to_string());
        assert!(!state.is_registered()); // Still need spectrometer_id

        state.spectrometer_id = Some("test-id".to_string());
        assert!(state.is_registered());
    }

    #[test]
    fn test_should_process_data() {
        let mut state = DeviceState::default();

        assert!(!state.should_process_data());

        state.is_running = true;
        assert!(state.should_process_data());

        state.is_running = false;
        state.is_depositing = true;
        assert!(state.should_process_data());
    }

    #[tokio::test]
    async fn test_shared_state() {
        let state = create_shared_state();

        // Write
        {
            let mut s = state.write().await;
            s.control_wavelength = 600.0;
        }

        // Read
        {
            let s = state.read().await;
            assert_eq!(s.control_wavelength, 600.0);
        }
    }
}
