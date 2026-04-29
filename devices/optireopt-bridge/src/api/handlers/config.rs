use axum::Json;
use axum::extract::State;

use crate::api::models::{ConfigResponse, ConfigUpdateRequest};
use crate::service::state::AppState;

pub async fn get_config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let cfg = state.config.read().await;
    let device = state.device.read().await;
    Json(ConfigResponse {
        source_url: cfg.source_url.clone(),
        reconnect_ms: cfg.reconnect_ms,
        source_connected: device.source_connected,
        scans_received: device.scans_received,
    })
}

pub async fn update_config(
    State(state): State<AppState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Json<ConfigResponse> {
    {
        let mut cfg = state.config.write().await;
        if let Some(url) = request.source_url {
            cfg.source_url = url;
        }
        if let Some(ms) = request.reconnect_ms {
            cfg.reconnect_ms = ms;
        }
    }
    get_config(State(state)).await
}
