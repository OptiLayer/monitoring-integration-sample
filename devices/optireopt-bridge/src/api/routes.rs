use axum::Router;
use axum::routing::{get, post};

use super::handlers::{config, device, vacuum_chamber};
use super::{web_ui, websocket};
use crate::service::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(web_ui::index))
        .route("/ws", get(websocket::ws_handler))
        .route("/device/info", get(device::get_device_info))
        .route("/register", post(device::register))
        .route("/latest", get(device::get_latest))
        .route(
            "/config",
            get(config::get_config).post(config::update_config),
        )
        .route(
            "/vacuum_chamber/material",
            get(vacuum_chamber::get_material).post(vacuum_chamber::set_material),
        )
        .route(
            "/vacuum_chamber/start",
            post(vacuum_chamber::start_deposition),
        )
        .route(
            "/vacuum_chamber/stop",
            post(vacuum_chamber::stop_deposition),
        )
        .route("/vacuum_chamber/status", get(vacuum_chamber::get_status))
        .with_state(state)
}
