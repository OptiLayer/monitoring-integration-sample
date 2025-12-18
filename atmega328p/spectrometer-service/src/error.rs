use thiserror::Error;

/// Main error type for the spectrometer service
#[derive(Error, Debug)]
pub enum SpectrometerError {
    #[error("Serial port error: {0}")]
    SerialPort(#[from] serialport::Error),

    #[error("Protocol parse error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Data source error: {0}")]
    DataSource(String),

    #[error("Channel send error")]
    ChannelSend,

    #[error("Device not registered with monitoring API")]
    NotRegistered,
}

/// Protocol-specific errors for ATmega328P communication
#[derive(Error, Debug, Clone)]
pub enum ProtocolError {
    #[error("Invalid GAIN value: {0}. Valid values: 1, 2, 4, 8, 16, 32, 64, 128")]
    InvalidGain(u8),

    #[error(
        "Invalid FADC value: {0}. Valid values: 500, 250, 125, 62.5, 50, 39.2, 33.3, 19.6, 16.7, 12.5, 10, 8.33, 6.25, 4.17 Hz"
    )]
    InvalidFadc(f32),

    #[error("Invalid COUNT value: {0}. Must be 1-12")]
    InvalidCount(u8),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Incomplete measurement cycle: missing series {0}")]
    IncompleteCycle(u8),

    #[error("Unexpected line: {0}")]
    UnexpectedLine(String),

    #[error("Invalid timestamp format: {0}")]
    InvalidTimestamp(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_error_display() {
        let err = ProtocolError::InvalidGain(5);
        assert!(err.to_string().contains("Invalid GAIN value: 5"));

        let err = ProtocolError::InvalidCount(15);
        assert!(err.to_string().contains("Invalid COUNT value: 15"));
    }

    #[test]
    fn test_spectrometer_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let spec_err: SpectrometerError = io_err.into();
        assert!(matches!(spec_err, SpectrometerError::Io(_)));
    }

    #[test]
    fn test_validation_error() {
        let err = SpectrometerError::Validation("full must be greater than sample".to_string());
        assert!(err.to_string().contains("full must be greater than sample"));
    }
}
