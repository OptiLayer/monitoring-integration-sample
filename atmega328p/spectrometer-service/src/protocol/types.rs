use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::ProtocolError;

/// Raw ADC values from ATmega328P (24-bit, 0-16777215)
pub type RawAdcValue = u32;

/// Maximum valid ADC value (24-bit)
pub const MAX_ADC_VALUE: RawAdcValue = 16_777_215;

/// Validated GAIN values for AD7793 ADC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Gain {
    X1 = 1,
    X2 = 2,
    #[default]
    X4 = 4,
    X8 = 8,
    X16 = 16,
    X32 = 32,
    X64 = 64,
    X128 = 128,
}

impl Gain {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl TryFrom<u8> for Gain {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Gain::X1),
            2 => Ok(Gain::X2),
            4 => Ok(Gain::X4),
            8 => Ok(Gain::X8),
            16 => Ok(Gain::X16),
            32 => Ok(Gain::X32),
            64 => Ok(Gain::X64),
            128 => Ok(Gain::X128),
            _ => Err(ProtocolError::InvalidGain(value)),
        }
    }
}

/// Validated FADC (sampling frequency) values in Hz
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum AdcFrequency {
    #[default]
    Hz500,
    Hz250,
    Hz125,
    Hz62_5,
    Hz50,
    Hz39_2,
    Hz33_3,
    Hz19_6,
    Hz16_7,
    Hz12_5,
    Hz10,
    Hz8_33,
    Hz6_25,
    Hz4_17,
}

impl AdcFrequency {
    pub fn as_f32(&self) -> f32 {
        match self {
            AdcFrequency::Hz500 => 500.0,
            AdcFrequency::Hz250 => 250.0,
            AdcFrequency::Hz125 => 125.0,
            AdcFrequency::Hz62_5 => 62.5,
            AdcFrequency::Hz50 => 50.0,
            AdcFrequency::Hz39_2 => 39.2,
            AdcFrequency::Hz33_3 => 33.3,
            AdcFrequency::Hz19_6 => 19.6,
            AdcFrequency::Hz16_7 => 16.7,
            AdcFrequency::Hz12_5 => 12.5,
            AdcFrequency::Hz10 => 10.0,
            AdcFrequency::Hz8_33 => 8.33,
            AdcFrequency::Hz6_25 => 6.25,
            AdcFrequency::Hz4_17 => 4.17,
        }
    }
}

impl TryFrom<f32> for AdcFrequency {
    type Error = ProtocolError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        // Allow small tolerance for float comparison
        let tolerance = 0.1;
        if (value - 500.0).abs() < tolerance {
            Ok(AdcFrequency::Hz500)
        } else if (value - 250.0).abs() < tolerance {
            Ok(AdcFrequency::Hz250)
        } else if (value - 125.0).abs() < tolerance {
            Ok(AdcFrequency::Hz125)
        } else if (value - 62.5).abs() < tolerance {
            Ok(AdcFrequency::Hz62_5)
        } else if (value - 50.0).abs() < tolerance {
            Ok(AdcFrequency::Hz50)
        } else if (value - 39.2).abs() < tolerance {
            Ok(AdcFrequency::Hz39_2)
        } else if (value - 33.3).abs() < tolerance {
            Ok(AdcFrequency::Hz33_3)
        } else if (value - 19.6).abs() < tolerance {
            Ok(AdcFrequency::Hz19_6)
        } else if (value - 16.7).abs() < tolerance {
            Ok(AdcFrequency::Hz16_7)
        } else if (value - 12.5).abs() < tolerance {
            Ok(AdcFrequency::Hz12_5)
        } else if (value - 10.0).abs() < tolerance {
            Ok(AdcFrequency::Hz10)
        } else if (value - 8.33).abs() < tolerance {
            Ok(AdcFrequency::Hz8_33)
        } else if (value - 6.25).abs() < tolerance {
            Ok(AdcFrequency::Hz6_25)
        } else if (value - 4.17).abs() < tolerance {
            Ok(AdcFrequency::Hz4_17)
        } else {
            Err(ProtocolError::InvalidFadc(value))
        }
    }
}

/// COUNT value (1-12) - number of measurements per series
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementCount(u8);

impl MeasurementCount {
    pub fn new(count: u8) -> Result<Self, ProtocolError> {
        if !(1..=12).contains(&count) {
            return Err(ProtocolError::InvalidCount(count));
        }
        Ok(Self(count))
    }

    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl Default for MeasurementCount {
    fn default() -> Self {
        Self(3) // Default per datasheet
    }
}

impl TryFrom<u8> for MeasurementCount {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A single series of measurements (dark, full, or sample)
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesData {
    pub values: Vec<RawAdcValue>,
}

impl SeriesData {
    pub fn new(values: Vec<RawAdcValue>) -> Self {
        Self { values }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Convert to f64 values for processing
    pub fn to_f64(&self) -> Vec<f64> {
        self.values.iter().map(|&v| v as f64).collect()
    }
}

/// Series identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesType {
    Dark,   // SERIES1
    Full,   // SERIES2
    Sample, // SERIES3
}

impl SeriesType {
    pub fn from_number(n: u8) -> Option<Self> {
        match n {
            1 => Some(SeriesType::Dark),
            2 => Some(SeriesType::Full),
            3 => Some(SeriesType::Sample),
            _ => None,
        }
    }

    pub fn as_number(&self) -> u8 {
        match self {
            SeriesType::Dark => 1,
            SeriesType::Full => 2,
            SeriesType::Sample => 3,
        }
    }
}

/// Complete measurement cycle from ATmega328P
#[derive(Debug, Clone)]
pub struct MeasurementCycle {
    pub timestamp: DateTime<Utc>,
    pub dark: SeriesData,   // SERIES1
    pub full: SeriesData,   // SERIES2
    pub sample: SeriesData, // SERIES3
}

impl MeasurementCycle {
    pub fn new(dark: SeriesData, full: SeriesData, sample: SeriesData) -> Self {
        Self {
            timestamp: Utc::now(),
            dark,
            full,
            sample,
        }
    }

    pub fn with_timestamp(
        timestamp: DateTime<Utc>,
        dark: SeriesData,
        full: SeriesData,
        sample: SeriesData,
    ) -> Self {
        Self {
            timestamp,
            dark,
            full,
            sample,
        }
    }
}

/// Processed measurement result after outlier exclusion and calibration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedMeasurement {
    pub timestamp: DateTime<Utc>,
    pub dark_mean: f64,
    pub full_mean: f64,
    pub sample_mean: f64,
    /// Calibrated reading as percentage: (sample-dark)/(full-dark) * 100
    pub calibrated_reading: f64,
    pub is_valid: bool,
    pub validation_error: Option<String>,
}

impl ProcessedMeasurement {
    pub fn new(
        timestamp: DateTime<Utc>,
        dark_mean: f64,
        full_mean: f64,
        sample_mean: f64,
        calibrated_reading: f64,
    ) -> Self {
        Self {
            timestamp,
            dark_mean,
            full_mean,
            sample_mean,
            calibrated_reading,
            is_valid: true,
            validation_error: None,
        }
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.is_valid = false;
        self.validation_error = Some(error);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gain_try_from_valid() {
        assert_eq!(Gain::try_from(1).unwrap(), Gain::X1);
        assert_eq!(Gain::try_from(128).unwrap(), Gain::X128);
    }

    #[test]
    fn test_gain_try_from_invalid() {
        assert!(Gain::try_from(3).is_err());
        assert!(Gain::try_from(0).is_err());
        assert!(Gain::try_from(255).is_err());
    }

    #[test]
    fn test_adc_frequency_try_from_valid() {
        assert_eq!(AdcFrequency::try_from(500.0).unwrap(), AdcFrequency::Hz500);
        assert_eq!(AdcFrequency::try_from(62.5).unwrap(), AdcFrequency::Hz62_5);
    }

    #[test]
    fn test_adc_frequency_try_from_invalid() {
        assert!(AdcFrequency::try_from(100.0).is_err());
        assert!(AdcFrequency::try_from(0.0).is_err());
    }

    #[test]
    fn test_measurement_count_valid() {
        assert!(MeasurementCount::new(1).is_ok());
        assert!(MeasurementCount::new(12).is_ok());
    }

    #[test]
    fn test_measurement_count_invalid() {
        assert!(MeasurementCount::new(0).is_err());
        assert!(MeasurementCount::new(13).is_err());
    }

    #[test]
    fn test_series_data_to_f64() {
        let series = SeriesData::new(vec![1000000, 2000000, 3000000]);
        let f64_values = series.to_f64();
        assert_eq!(f64_values, vec![1000000.0, 2000000.0, 3000000.0]);
    }

    #[test]
    fn test_series_type_from_number() {
        assert_eq!(SeriesType::from_number(1), Some(SeriesType::Dark));
        assert_eq!(SeriesType::from_number(2), Some(SeriesType::Full));
        assert_eq!(SeriesType::from_number(3), Some(SeriesType::Sample));
        assert_eq!(SeriesType::from_number(4), None);
    }

    #[test]
    fn test_measurement_cycle_creation() {
        let dark = SeriesData::new(vec![100, 101, 102]);
        let full = SeriesData::new(vec![8000, 8001, 8002]);
        let sample = SeriesData::new(vec![4000, 4001, 4002]);

        let cycle = MeasurementCycle::new(dark.clone(), full.clone(), sample.clone());

        assert_eq!(cycle.dark.values, dark.values);
        assert_eq!(cycle.full.values, full.values);
        assert_eq!(cycle.sample.values, sample.values);
    }

    #[test]
    fn test_processed_measurement_with_error() {
        let measurement = ProcessedMeasurement::new(Utc::now(), 100.0, 8000.0, 4000.0, 49.4);

        assert!(measurement.is_valid);
        assert!(measurement.validation_error.is_none());

        let measurement = measurement.with_error("sample > full".to_string());
        assert!(!measurement.is_valid);
        assert!(measurement.validation_error.is_some());
    }
}
