use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Monitoring API: {0}")]
    Monitoring(String),
}
