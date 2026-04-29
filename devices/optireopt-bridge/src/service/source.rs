use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::monitoring::client::MonitoringClient;
use crate::service::state::{ScanFrame, SharedConfig, SharedDevice};

const MAX_BACKOFF_FACTOR: u64 = 10;

pub async fn run_source_loop(
    device: SharedDevice,
    config: SharedConfig,
    broadcast_tx: broadcast::Sender<serde_json::Value>,
    monitoring: Arc<MonitoringClient>,
) {
    let mut backoff_factor: u64 = 1;
    loop {
        let (source_url, base_backoff_ms) = {
            let cfg = config.read().await;
            (cfg.source_url.clone(), cfg.reconnect_ms)
        };

        tracing::info!("Connecting to OptiReOpt at {}", source_url);

        match connect_async(&source_url).await {
            Ok((ws_stream, _resp)) => {
                backoff_factor = 1;
                set_connected(&device, true).await;
                broadcast_status(&broadcast_tx, true, &source_url);

                let (_write, mut read) = ws_stream.split();
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            handle_frame(text.as_str(), &device, &broadcast_tx, &monitoring).await;
                        }
                        Ok(Message::Binary(_)) => {
                            tracing::debug!("Ignoring binary frame from source");
                        }
                        Ok(Message::Close(_)) => {
                            tracing::info!("Source closed connection");
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!("Source read error: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Source connect failed: {}", e);
            }
        }

        set_connected(&device, false).await;
        broadcast_status(&broadcast_tx, false, &source_url);

        let delay_ms = base_backoff_ms.saturating_mul(backoff_factor);
        tracing::info!("Reconnecting in {} ms", delay_ms);
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        backoff_factor = (backoff_factor * 2).min(MAX_BACKOFF_FACTOR);
    }
}

async fn handle_frame(
    text: &str,
    device: &SharedDevice,
    broadcast_tx: &broadcast::Sender<serde_json::Value>,
    monitoring: &Arc<MonitoringClient>,
) {
    let frame: ScanFrame = match serde_json::from_str(text) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Failed to parse source frame: {}", e);
            return;
        }
    };

    if frame.wavelength.len() != frame.values.len() {
        tracing::warn!(
            "Mismatched wavelength/values lengths ({} vs {}); dropping frame",
            frame.wavelength.len(),
            frame.values.len()
        );
        return;
    }

    // Capture latest + counters; copy registration info while we hold the lock.
    let (api_url, spectrometer_id) = {
        let mut state = device.write().await;
        state.scans_received = state.scans_received.saturating_add(1);
        state.latest_frame = Some(frame.clone());
        (
            state.monitoring_api_url.clone(),
            state.spectrometer_id.clone(),
        )
    };

    // Push to OptiMonitor if registered.
    if let (Some(url), Some(id)) = (api_url, spectrometer_id) {
        if let Err(e) = monitoring
            .post_spectral_data(
                &url,
                &id,
                &frame.values,
                Some(&frame.wavelength),
                Utc::now(),
            )
            .await
        {
            tracing::warn!("post_spectral_data failed: {}", e);
        }
    }

    // Rebroadcast to local subscribers (including the dashboard).
    let _ = broadcast_tx.send(json!({
        "type": "scan",
        "wavelengths": frame.wavelength,
        "values": frame.values,
        "rt_data": frame.rt_data,
        "timestamp": frame.timestamp,
    }));
}

async fn set_connected(device: &SharedDevice, connected: bool) {
    let mut s = device.write().await;
    s.source_connected = connected;
}

fn broadcast_status(
    broadcast_tx: &broadcast::Sender<serde_json::Value>,
    connected: bool,
    source_url: &str,
) {
    let _ = broadcast_tx.send(json!({
        "type": "source_status",
        "connection": if connected { "connected" } else { "unreachable" },
        "source": source_url,
    }));
}
