use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::api::models::{
    DeviceCapabilities, DeviceInfoResponse, RegisterRequest, RegisterResponse,
};
use crate::service::state::{AppState, ScanFrame};

pub async fn get_device_info() -> Json<DeviceInfoResponse> {
    Json(DeviceInfoResponse {
        device_type: "spectrometer".to_string(),
        name: "OptiReOpt Bridge".to_string(),
        capabilities: DeviceCapabilities {
            has_spectrometer: true,
            // The bridge has no real chamber — these endpoints are no-ops that
            // exist purely so OptiMonitor's AutomaticStrategy (driven by
            // OptiReOpt's dt_switch) can advance its own layer counter.
            has_vacuum_chamber: true,
            spectrometer_type: "broadband".to_string(),
            is_monochromatic: false,
        },
    })
}

pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Json<RegisterResponse> {
    let mut device = state.device.write().await;
    device.monitoring_api_url = Some(request.monitoring_api_url.clone());
    device.spectrometer_id = request.spectrometer_id.clone();

    tracing::info!(
        "Registered with OptiMonitor: {}, spectrometer_id: {:?}",
        request.monitoring_api_url,
        request.spectrometer_id,
    );

    Json(RegisterResponse {
        status: "registered".to_string(),
        spectrometer_id: device.spectrometer_id.clone(),
        monitoring_api_url: request.monitoring_api_url,
    })
}

pub async fn get_latest(State(state): State<AppState>) -> impl IntoResponse {
    let device = state.device.read().await;
    match &device.latest_frame {
        Some(frame) => (StatusCode::OK, Json(frame.clone())).into_response(),
        None => (StatusCode::NO_CONTENT, Json::<Option<ScanFrame>>(None)).into_response(),
    }
}
