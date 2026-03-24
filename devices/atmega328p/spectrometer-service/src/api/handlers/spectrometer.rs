use axum::Json;
use axum::extract::State;

use crate::api::models::*;
use crate::service::state::AppState;

/// GET /control_wavelength - Get current control wavelength
pub async fn get_control_wavelength(
    State(state): State<AppState>,
) -> Json<ControlWavelengthResponse> {
    let device = state.device.read().await;

    Json(ControlWavelengthResponse {
        control_wavelength: device.control_wavelength,
    })
}

/// POST /control_wavelength - Set control wavelength (dummy implementation)
pub async fn set_control_wavelength(
    State(state): State<AppState>,
    Json(request): Json<ControlWavelengthRequest>,
) -> Json<ControlWavelengthResponse> {
    let mut device = state.device.write().await;

    device.control_wavelength = request.wavelength;

    tracing::info!("Control wavelength set to {} nm", request.wavelength);

    Json(ControlWavelengthResponse {
        control_wavelength: device.control_wavelength,
    })
}

#[cfg(test)]
mod tests {

    use tokio::sync::{broadcast, mpsc};

    use super::*;
    use crate::service::calibration::create_shared_config;
    use crate::service::state::create_shared_state;

    fn test_state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _) = broadcast::channel(16);
        let (cmd_tx, _) = mpsc::channel(16);
        let state = AppState {
            device: create_shared_state(),
            config: create_shared_config(dir.path().join("cfg.toml")),
            broadcast_tx: tx,
            device_cmd_tx: cmd_tx,
        };
        (state, dir)
    }

    #[tokio::test]
    async fn test_get_control_wavelength() {
        let (state, _dir) = test_state();
        let response = get_control_wavelength(State(state)).await;
        assert_eq!(response.control_wavelength, 550.0);
    }

    #[tokio::test]
    async fn test_set_control_wavelength() {
        let (state, _dir) = test_state();

        let request = ControlWavelengthRequest { wavelength: 600.0 };
        let response = set_control_wavelength(State(state.clone()), Json(request)).await;
        assert_eq!(response.control_wavelength, 600.0);

        let device = state.device.read().await;
        assert_eq!(device.control_wavelength, 600.0);
    }
}
