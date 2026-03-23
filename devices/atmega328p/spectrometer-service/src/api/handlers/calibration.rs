use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

use crate::service::state::AppState;

pub async fn get_settings(State(state): State<AppState>) -> Json<serde_json::Value> {
    let cfg = state.config.read().await;

    Json(serde_json::json!({
        "gain": cfg.config.device_settings.gain,
        "fadc": cfg.config.device_settings.fadc,
        "count": cfg.config.device_settings.count,
        "last_updated": cfg.config.last_updated.to_rfc3339(),
    }))
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub gain: u8,
    pub fadc: f32,
    pub count: u8,
}

pub async fn update_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateSettingsRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Send commands to device
    for cmd in [
        format!("GAIN={}", req.gain),
        format!("FADC={}", req.fadc),
        format!("COUNT={}", req.count),
    ] {
        if let Err(e) = state.send_device_command(&cmd).await {
            tracing::warn!("Failed to send device command '{cmd}': {e}");
        }
    }

    // Save to config file
    let mut cfg = state.config.write().await;
    cfg.update_settings(req.gain, req.fadc, req.count);

    if let Err(e) = cfg.save() {
        tracing::error!("Failed to save config: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        );
    }

    let _ = state.broadcast_tx.send(serde_json::json!({
        "type": "settings_updated",
        "gain": req.gain,
        "fadc": req.fadc,
        "count": req.count,
    }));

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "applied",
            "gain": req.gain,
            "fadc": req.fadc,
            "count": req.count,
        })),
    )
}
