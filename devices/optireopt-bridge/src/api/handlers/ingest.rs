use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::Utc;
use serde_json::json;

use crate::service::state::{AppState, ScanFrame};

pub async fn ingest(
    State(state): State<AppState>,
    Json(frame): Json<ScanFrame>,
) -> impl IntoResponse {
    if frame.wavelength.len() != frame.values.len() {
        tracing::warn!(
            "Mismatched wavelength/values lengths ({} vs {}); rejecting frame",
            frame.wavelength.len(),
            frame.values.len(),
        );
        return StatusCode::BAD_REQUEST.into_response();
    }

    let now = Utc::now();
    let (api_url, spectrometer_id) = {
        let mut device = state.device.write().await;
        device.scans_received = device.scans_received.saturating_add(1);
        device.latest_frame = Some(frame.clone());
        device.last_frame_at = Some(now);
        (
            device.monitoring_api_url.clone(),
            device.spectrometer_id.clone(),
        )
    };

    if let (Some(url), Some(id)) = (api_url, spectrometer_id) {
        if let Err(e) = state
            .monitoring
            .post_spectral_data(&url, &id, &frame.values, Some(&frame.wavelength), now)
            .await
        {
            tracing::warn!("post_spectral_data failed: {}", e);
        }
    }

    let _ = state.broadcast_tx.send(json!({
        "type": "scan",
        "wavelengths": frame.wavelength,
        "values": frame.values,
        "rt_data": frame.rt_data,
        "timestamp": frame.timestamp,
    }));

    StatusCode::NO_CONTENT.into_response()
}
