use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

use crate::service::calibration::SeriesMapping;
use crate::service::state::AppState;

pub async fn get_settings(State(state): State<AppState>) -> Json<serde_json::Value> {
    let cfg = state.config.read().await;
    let s = &cfg.config.device_settings;

    Json(serde_json::json!({
        "gain": s.gain,
        "fadc": s.fadc,
        "count": s.count,
        "series_mapping": {
            "dark": s.series_mapping.dark,
            "full": s.series_mapping.full,
            "sample": s.series_mapping.sample,
        },
        "last_updated": cfg.config.last_updated.to_rfc3339(),
    }))
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub gain: u8,
    pub fadc: f32,
    pub count: u8,
    #[serde(default)]
    pub series_mapping: Option<SeriesMappingRequest>,
}

#[derive(Deserialize)]
pub struct SeriesMappingRequest {
    pub dark: u8,
    pub full: u8,
    pub sample: u8,
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

    if let Some(m) = &req.series_mapping {
        cfg.config.device_settings.series_mapping = SeriesMapping {
            dark: m.dark,
            full: m.full,
            sample: m.sample,
        };
    }

    if let Err(e) = cfg.save() {
        tracing::error!("Failed to save config: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        );
    }

    let mapping = &cfg.config.device_settings.series_mapping;
    let _ = state.broadcast_tx.send(serde_json::json!({
        "type": "settings_updated",
        "gain": req.gain,
        "fadc": req.fadc,
        "count": req.count,
        "series_mapping": {
            "dark": mapping.dark,
            "full": mapping.full,
            "sample": mapping.sample,
        },
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
