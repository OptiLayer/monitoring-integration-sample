use axum::Router;
use axum::routing::{get, post};

use super::handlers::{device, spectrometer, vacuum_chamber};
use crate::service::SharedState;

/// Create the API router with all endpoints
pub fn create_router(state: SharedState) -> Router {
    Router::new()
        // Device info and registration
        .route("/device/info", get(device::get_device_info))
        .route("/register", post(device::register))
        // Spectrometer control
        .route(
            "/control_wavelength",
            get(spectrometer::get_control_wavelength).post(spectrometer::set_control_wavelength),
        )
        // Vacuum chamber control
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
        // Add state to all routes
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    use super::*;
    use crate::service::state::create_shared_state;

    #[tokio::test]
    async fn test_device_info_route() {
        let state = create_shared_state();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/device/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_control_wavelength_get() {
        let state = create_shared_state();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/control_wavelength")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_vacuum_chamber_status() {
        let state = create_shared_state();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/vacuum_chamber/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
