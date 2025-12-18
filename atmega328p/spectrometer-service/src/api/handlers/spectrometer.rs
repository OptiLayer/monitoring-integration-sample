use axum::Json;
use axum::extract::State;

use crate::api::models::*;
use crate::service::SharedState;

/// GET /control_wavelength - Get current control wavelength
pub async fn get_control_wavelength(
    State(state): State<SharedState>,
) -> Json<ControlWavelengthResponse> {
    let state = state.read().await;

    Json(ControlWavelengthResponse {
        control_wavelength: state.control_wavelength,
    })
}

/// POST /control_wavelength - Set control wavelength (dummy implementation)
pub async fn set_control_wavelength(
    State(state): State<SharedState>,
    Json(request): Json<ControlWavelengthRequest>,
) -> Json<ControlWavelengthResponse> {
    let mut state = state.write().await;

    state.control_wavelength = request.wavelength;

    tracing::info!("Control wavelength set to {} nm", request.wavelength);

    Json(ControlWavelengthResponse {
        control_wavelength: state.control_wavelength,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::state::create_shared_state;

    #[tokio::test]
    async fn test_get_control_wavelength() {
        let state = create_shared_state();

        let response = get_control_wavelength(State(state)).await;

        assert_eq!(response.control_wavelength, 550.0); // Default value
    }

    #[tokio::test]
    async fn test_set_control_wavelength() {
        let state = create_shared_state();

        let request = ControlWavelengthRequest { wavelength: 600.0 };
        let response = set_control_wavelength(State(state.clone()), Json(request)).await;

        assert_eq!(response.control_wavelength, 600.0);

        // Verify state was updated
        let state = state.read().await;
        assert_eq!(state.control_wavelength, 600.0);
    }
}
