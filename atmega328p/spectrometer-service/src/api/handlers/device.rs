use axum::Json;
use axum::extract::State;

use crate::api::models::*;
use crate::service::SharedState;

/// GET /device/info - Return device capabilities
pub async fn get_device_info() -> Json<DeviceInfoResponse> {
    Json(DeviceInfoResponse {
        device_type: "spectrometer".to_string(),
        name: "ATmega328P Monochromatic Spectrometer".to_string(),
        capabilities: DeviceCapabilities {
            has_spectrometer: true,
            has_vacuum_chamber: true,
            spectrometer_type: "two-component".to_string(),
            is_monochromatic: true,
        },
    })
}

/// POST /register - Receive assigned IDs from monitoring system
pub async fn register(
    State(state): State<SharedState>,
    Json(request): Json<RegisterRequest>,
) -> Json<RegisterResponse> {
    let mut state = state.write().await;

    state.monitoring_api_url = Some(request.monitoring_api_url.clone());
    state.spectrometer_id = request.spectrometer_id.clone();
    state.vacuum_chamber_id = request.vacuum_chamber_id.clone();

    tracing::info!(
        "Registered with monitoring API: {}, spectrometer_id: {:?}, vacuum_chamber_id: {:?}",
        request.monitoring_api_url,
        request.spectrometer_id,
        request.vacuum_chamber_id
    );

    Json(RegisterResponse {
        status: "registered".to_string(),
        spectrometer_id: state.spectrometer_id.clone(),
        vacuum_chamber_id: state.vacuum_chamber_id.clone(),
        monitoring_api_url: request.monitoring_api_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::state::create_shared_state;

    #[tokio::test]
    async fn test_get_device_info() {
        let response = get_device_info().await;

        assert_eq!(response.device_type, "spectrometer");
        assert!(response.capabilities.has_spectrometer);
        assert!(response.capabilities.has_vacuum_chamber);
        assert!(response.capabilities.is_monochromatic);
    }

    #[tokio::test]
    async fn test_register() {
        let state = create_shared_state();

        let request = RegisterRequest {
            monitoring_api_url: "http://localhost:8200".to_string(),
            spectrometer_id: Some("spec-123".to_string()),
            vacuum_chamber_id: Some("vc-456".to_string()),
        };

        let response = register(State(state.clone()), Json(request)).await;

        assert_eq!(response.status, "registered");
        assert_eq!(response.spectrometer_id, Some("spec-123".to_string()));

        // Verify state was updated
        let state = state.read().await;
        assert!(state.is_registered());
    }
}
