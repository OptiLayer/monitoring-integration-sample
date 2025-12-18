use serde::{Deserialize, Serialize};

// ============= Device Endpoints =============

#[derive(Debug, Serialize)]
pub struct DeviceInfoResponse {
    #[serde(rename = "type")]
    pub device_type: String,
    pub name: String,
    pub capabilities: DeviceCapabilities,
}

#[derive(Debug, Serialize)]
pub struct DeviceCapabilities {
    pub has_spectrometer: bool,
    pub has_vacuum_chamber: bool,
    pub spectrometer_type: String,
    pub is_monochromatic: bool,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub monitoring_api_url: String,
    pub spectrometer_id: Option<String>,
    pub vacuum_chamber_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub status: String,
    pub spectrometer_id: Option<String>,
    pub vacuum_chamber_id: Option<String>,
    pub monitoring_api_url: String,
}

// ============= Spectrometer Endpoints =============

#[derive(Debug, Deserialize)]
pub struct ControlWavelengthRequest {
    pub wavelength: f64,
}

#[derive(Debug, Serialize)]
pub struct ControlWavelengthResponse {
    pub control_wavelength: f64,
}

// ============= Vacuum Chamber Endpoints =============

#[derive(Debug, Serialize)]
pub struct MaterialResponse {
    pub material: String,
}

#[derive(Debug, Serialize)]
pub struct VacuumChamberStatusResponse {
    pub status: String,
    pub is_depositing: bool,
}

#[derive(Debug, Serialize)]
pub struct DepositionResponse {
    pub status: String,
}

// ============= Error Response =============

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
