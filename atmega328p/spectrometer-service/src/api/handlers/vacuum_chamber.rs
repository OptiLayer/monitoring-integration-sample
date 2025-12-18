use axum::Json;
use axum::extract::State;

use crate::api::models::*;
use crate::service::SharedState;

/// GET /vacuum_chamber/material - Get current material
pub async fn get_material(State(state): State<SharedState>) -> Json<MaterialResponse> {
    let state = state.read().await;

    Json(MaterialResponse {
        material: state.current_material.clone(),
    })
}

/// POST /vacuum_chamber/material - Set material
pub async fn set_material(
    State(state): State<SharedState>,
    body: String,
) -> Json<MaterialResponse> {
    let mut state = state.write().await;

    // Body is plain string (JSON string or raw)
    let material = body.trim().trim_matches('"').to_string();
    state.current_material = material.clone();

    tracing::info!("Material set to {}", material);

    Json(MaterialResponse { material })
}

/// POST /vacuum_chamber/start - Start deposition
pub async fn start_deposition(State(state): State<SharedState>) -> Json<DepositionResponse> {
    let mut state = state.write().await;

    state.is_depositing = true;
    state.is_running = true;

    tracing::info!("Deposition started");

    Json(DepositionResponse {
        status: "running".to_string(),
    })
}

/// POST /vacuum_chamber/stop - Stop deposition
pub async fn stop_deposition(State(state): State<SharedState>) -> Json<DepositionResponse> {
    let mut state = state.write().await;

    state.is_depositing = false;
    state.is_running = false;

    tracing::info!("Deposition stopped");

    Json(DepositionResponse {
        status: "stopped".to_string(),
    })
}

/// GET /vacuum_chamber/status - Get chamber status
pub async fn get_status(State(state): State<SharedState>) -> Json<VacuumChamberStatusResponse> {
    let state = state.read().await;

    Json(VacuumChamberStatusResponse {
        status: if state.is_depositing {
            "running".to_string()
        } else {
            "stopped".to_string()
        },
        is_depositing: state.is_depositing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::state::create_shared_state;

    #[tokio::test]
    async fn test_get_material() {
        let state = create_shared_state();

        let response = get_material(State(state)).await;

        assert_eq!(response.material, "H"); // Default
    }

    #[tokio::test]
    async fn test_set_material() {
        let state = create_shared_state();

        let response = set_material(State(state.clone()), "L".to_string()).await;

        assert_eq!(response.material, "L");

        let state = state.read().await;
        assert_eq!(state.current_material, "L");
    }

    #[tokio::test]
    async fn test_set_material_json_string() {
        let state = create_shared_state();

        // JSON-encoded string with quotes
        let response = set_material(State(state.clone()), "\"H\"".to_string()).await;

        assert_eq!(response.material, "H");
    }

    #[tokio::test]
    async fn test_start_stop_deposition() {
        let state = create_shared_state();

        // Start
        let response = start_deposition(State(state.clone())).await;
        assert_eq!(response.status, "running");

        {
            let s = state.read().await;
            assert!(s.is_depositing);
            assert!(s.is_running);
        }

        // Stop
        let response = stop_deposition(State(state.clone())).await;
        assert_eq!(response.status, "stopped");

        {
            let s = state.read().await;
            assert!(!s.is_depositing);
            assert!(!s.is_running);
        }
    }

    #[tokio::test]
    async fn test_get_status() {
        let state = create_shared_state();

        let response = get_status(State(state.clone())).await;
        assert_eq!(response.status, "stopped");
        assert!(!response.is_depositing);

        start_deposition(State(state.clone())).await;

        let response = get_status(State(state)).await;
        assert_eq!(response.status, "running");
        assert!(response.is_depositing);
    }
}
