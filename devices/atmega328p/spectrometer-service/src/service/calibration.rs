use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Maximum raw ADC value (24-bit) — indicates saturation/clipping
pub const MAX_ADC_VALUE: u32 = 16_777_215;

/// Persisted device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub device_settings: DeviceSettings,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSettings {
    pub gain: u8,
    pub fadc: f32,
    pub count: u8,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            gain: 2,
            fadc: 250.0,
            count: 4,
        }
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_settings: DeviceSettings::default(),
            last_updated: Utc::now(),
        }
    }
}

/// Runtime state for the calibration/device config
pub struct ConfigRuntime {
    pub config: DeviceConfig,
    config_path: PathBuf,
}

impl ConfigRuntime {
    pub fn load(path: PathBuf) -> Self {
        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(config) => {
                        tracing::info!("Loaded device config from {:?}", path);
                        config
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse device config: {}, using defaults", e);
                        DeviceConfig::default()
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read device config: {}, using defaults", e);
                    DeviceConfig::default()
                }
            }
        } else {
            tracing::info!("No device config found at {:?}, using defaults", path);
            DeviceConfig::default()
        };

        Self {
            config,
            config_path: path,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let toml_str =
            toml::to_string_pretty(&self.config).map_err(|e| format!("Serialize error: {e}"))?;
        std::fs::write(&self.config_path, toml_str).map_err(|e| format!("Write error: {e}"))?;
        tracing::info!("Saved device config to {:?}", self.config_path);
        Ok(())
    }

    pub fn update_settings(&mut self, gain: u8, fadc: f32, count: u8) {
        self.config.device_settings = DeviceSettings { gain, fadc, count };
        self.config.last_updated = Utc::now();
    }
}

pub type SharedConfig = Arc<RwLock<ConfigRuntime>>;

pub fn create_shared_config(path: PathBuf) -> SharedConfig {
    Arc::new(RwLock::new(ConfigRuntime::load(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_config_default() {
        let config = DeviceConfig::default();
        assert_eq!(config.device_settings.gain, 2);
        assert_eq!(config.device_settings.fadc, 250.0);
        assert_eq!(config.device_settings.count, 4);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_config.toml");

        let mut runtime = ConfigRuntime::load(path.clone());
        runtime.update_settings(4, 500.0, 3);
        runtime.save().unwrap();

        let runtime2 = ConfigRuntime::load(path);
        assert_eq!(runtime2.config.device_settings.gain, 4);
        assert_eq!(runtime2.config.device_settings.fadc, 500.0);
        assert_eq!(runtime2.config.device_settings.count, 3);
    }
}
