use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};

#[derive(Debug, Clone, Default)]
pub struct DeviceState {
    pub monitoring_api_url: Option<String>,
    pub spectrometer_id: Option<String>,
    pub source_connected: bool,
    pub latest_frame: Option<ScanFrame>,
    pub scans_received: u64,
    // Mirrored from OptiMonitor's auto-switch calls so /vacuum_chamber/status
    // returns something coherent. We don't drive any real hardware with this.
    pub current_material: String,
    pub current_fraction: Option<f64>,
    pub is_depositing: bool,
    pub material_switches: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanFrame {
    pub wavelength: Vec<f64>,
    pub values: Vec<f64>,
    pub rt_data: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub source_url: String,
    pub reconnect_ms: u64,
}

pub type SharedDevice = Arc<RwLock<DeviceState>>;
pub type SharedConfig = Arc<RwLock<BridgeConfig>>;

#[derive(Clone)]
pub struct AppState {
    pub device: SharedDevice,
    pub config: SharedConfig,
    pub broadcast_tx: broadcast::Sender<serde_json::Value>,
}
