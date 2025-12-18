use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use regex::Regex;

use super::types::{MeasurementCycle, RawAdcValue, SeriesData};

// Pre-compiled regex patterns for efficiency
static SERIES_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^SERIES(\d)\s*=\s*\[([^\]]+)\]").unwrap());

static GAIN_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^GAIN=(\d+)").unwrap());

static FADC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^FADC=(\d+(?:\.\d+)?)").unwrap());

static COUNT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^COUNT=(\d+)").unwrap());

static MEASUREMENTS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^MEASUREMENTS\s*=\s*\[([^\]]+)\]").unwrap());

/// Parsed line variants from ATmega328P serial output
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLine {
    /// Series data: SERIES1/2/3 = [values]
    Series {
        number: u8,
        values: Vec<RawAdcValue>,
    },
    /// End of measurement cycle marker
    EndCycle,
    /// GAIN setting confirmation
    GainSet(u8),
    /// FADC setting confirmation
    FadcSet(f32),
    /// COUNT setting confirmation
    CountSet(u8),
    /// Debug measurements output
    Measurements(Vec<RawAdcValue>),
    /// ADC ready message
    AdcReady,
    /// Error message from device
    Error(String),
    /// Measurement cycle missing warning
    MeasurementCycleMissing,
    /// Unrecognized line
    Unknown(String),
}

/// Parse space-separated values into a Vec<u32>
fn parse_values(values_str: &str) -> Vec<RawAdcValue> {
    values_str
        .split_whitespace()
        .filter_map(|s| s.parse::<RawAdcValue>().ok())
        .collect()
}

/// Parse a single line from ATmega328P serial output
pub fn parse_line(input: &str) -> ParsedLine {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return ParsedLine::Unknown(String::new());
    }

    // SERIES1/2/3 = [values]
    if let Some(caps) = SERIES_REGEX.captures(trimmed) {
        let number: u8 = caps[1].parse().unwrap_or(0);
        let values = parse_values(&caps[2]);
        return ParsedLine::Series { number, values };
    }

    // END_CYCLE
    if trimmed == "END_CYCLE" {
        return ParsedLine::EndCycle;
    }

    // GAIN=<value>
    if let Some(caps) = GAIN_REGEX.captures(trimmed) {
        if let Ok(gain) = caps[1].parse::<u8>() {
            return ParsedLine::GainSet(gain);
        }
    }

    // FADC=<value>
    if let Some(caps) = FADC_REGEX.captures(trimmed) {
        if let Ok(fadc) = caps[1].parse::<f32>() {
            return ParsedLine::FadcSet(fadc);
        }
    }

    // COUNT=<value>
    if let Some(caps) = COUNT_REGEX.captures(trimmed) {
        if let Ok(count) = caps[1].parse::<u8>() {
            return ParsedLine::CountSet(count);
        }
    }

    // MEASUREMENTS = [values]
    if let Some(caps) = MEASUREMENTS_REGEX.captures(trimmed) {
        let values = parse_values(&caps[1]);
        return ParsedLine::Measurements(values);
    }

    // ADC ready
    if trimmed == "ADC ready" {
        return ParsedLine::AdcReady;
    }

    // Measurement cycle is missing
    if trimmed == "Measurement cycle is missing" {
        return ParsedLine::MeasurementCycleMissing;
    }

    // ERROR <message>
    if let Some(msg) = trimmed.strip_prefix("ERROR ") {
        return ParsedLine::Error(msg.to_string());
    }

    ParsedLine::Unknown(trimmed.to_string())
}

/// State machine for accumulating a complete measurement cycle
#[derive(Debug, Default)]
pub struct CycleAccumulator {
    series1: Option<Vec<RawAdcValue>>,
    series2: Option<Vec<RawAdcValue>>,
    series3: Option<Vec<RawAdcValue>>,
    timestamp: Option<DateTime<Utc>>,
}

impl CycleAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a parsed line and return a complete cycle if ready
    pub fn process_line(&mut self, line: ParsedLine) -> Option<MeasurementCycle> {
        match line {
            ParsedLine::Series { number: 1, values } => {
                if self.series1.is_none() {
                    self.timestamp = Some(Utc::now());
                }
                self.series1 = Some(values);
                None
            }
            ParsedLine::Series { number: 2, values } => {
                self.series2 = Some(values);
                None
            }
            ParsedLine::Series { number: 3, values } => {
                self.series3 = Some(values);
                None
            }
            ParsedLine::EndCycle => self.try_complete(),
            _ => None,
        }
    }

    /// Process a parsed line with an external timestamp (for log playback)
    pub fn process_line_with_timestamp(
        &mut self,
        line: ParsedLine,
        timestamp: DateTime<Utc>,
    ) -> Option<MeasurementCycle> {
        match line {
            ParsedLine::Series { number: 1, values } => {
                self.timestamp = Some(timestamp);
                self.series1 = Some(values);
                None
            }
            ParsedLine::Series { number: 2, values } => {
                self.series2 = Some(values);
                None
            }
            ParsedLine::Series { number: 3, values } => {
                self.series3 = Some(values);
                None
            }
            ParsedLine::EndCycle => self.try_complete(),
            _ => None,
        }
    }

    fn try_complete(&mut self) -> Option<MeasurementCycle> {
        // Only take values if all series are present
        if self.series1.is_none() || self.series2.is_none() || self.series3.is_none() {
            return None;
        }

        let s1 = self.series1.take().unwrap();
        let s2 = self.series2.take().unwrap();
        let s3 = self.series3.take().unwrap();
        let timestamp = self.timestamp.take().unwrap_or_else(Utc::now);

        Some(MeasurementCycle::with_timestamp(
            timestamp,
            SeriesData::new(s1),
            SeriesData::new(s2),
            SeriesData::new(s3),
        ))
    }

    pub fn reset(&mut self) {
        self.series1 = None;
        self.series2 = None;
        self.series3 = None;
        self.timestamp = None;
    }

    pub fn has_partial_data(&self) -> bool {
        self.series1.is_some() || self.series2.is_some() || self.series3.is_some()
    }

    pub fn missing_series(&self) -> Vec<u8> {
        let mut missing = Vec::new();
        if self.series1.is_none() {
            missing.push(1);
        }
        if self.series2.is_none() {
            missing.push(2);
        }
        if self.series3.is_none() {
            missing.push(3);
        }
        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_series_line() {
        let result = parse_line("SERIES1 = [1234567 1234568 1234569]");
        assert_eq!(result, ParsedLine::Series {
            number: 1,
            values: vec![1234567, 1234568, 1234569]
        });
    }

    #[test]
    fn test_parse_series_with_different_counts() {
        let result = parse_line("SERIES1 = [1000000]");
        assert_eq!(result, ParsedLine::Series {
            number: 1,
            values: vec![1000000]
        });

        let result = parse_line("SERIES2 = [1 2 3 4 5 6 7 8 9 10 11 12]");
        assert_eq!(result, ParsedLine::Series {
            number: 2,
            values: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        });
    }

    #[test]
    fn test_parse_end_cycle() {
        assert_eq!(parse_line("END_CYCLE"), ParsedLine::EndCycle);
    }

    #[test]
    fn test_parse_gain_response() {
        assert_eq!(parse_line("GAIN=4"), ParsedLine::GainSet(4));
        assert_eq!(parse_line("GAIN=128"), ParsedLine::GainSet(128));
    }

    #[test]
    fn test_parse_fadc_response() {
        assert_eq!(parse_line("FADC=500"), ParsedLine::FadcSet(500.0));
        assert_eq!(parse_line("FADC=62.5"), ParsedLine::FadcSet(62.5));
    }

    #[test]
    fn test_parse_count_response() {
        assert_eq!(parse_line("COUNT=3"), ParsedLine::CountSet(3));
        assert_eq!(parse_line("COUNT=12"), ParsedLine::CountSet(12));
    }

    #[test]
    fn test_parse_error_messages() {
        assert_eq!(
            parse_line("ERROR Unknown command"),
            ParsedLine::Error("Unknown command".to_string())
        );
        assert_eq!(
            parse_line("ERROR Invalid GAIN value"),
            ParsedLine::Error("Invalid GAIN value".to_string())
        );
    }

    #[test]
    fn test_parse_adc_ready() {
        assert_eq!(parse_line("ADC ready"), ParsedLine::AdcReady);
    }

    #[test]
    fn test_parse_measurement_cycle_missing() {
        assert_eq!(
            parse_line("Measurement cycle is missing"),
            ParsedLine::MeasurementCycleMissing
        );
    }

    #[test]
    fn test_parse_invalid_input() {
        assert_eq!(
            parse_line("some random text"),
            ParsedLine::Unknown("some random text".to_string())
        );
        assert_eq!(parse_line(""), ParsedLine::Unknown(String::new()));
        assert_eq!(parse_line("   "), ParsedLine::Unknown(String::new()));
    }

    #[test]
    fn test_parse_measurements() {
        assert_eq!(
            parse_line("MEASUREMENTS = [1000 2000 3000]"),
            ParsedLine::Measurements(vec![1000, 2000, 3000])
        );
    }

    #[test]
    fn test_cycle_accumulator_complete_cycle() {
        let mut acc = CycleAccumulator::new();

        assert!(
            acc.process_line(ParsedLine::Series {
                number: 1,
                values: vec![100, 101, 102]
            })
            .is_none()
        );

        assert!(
            acc.process_line(ParsedLine::Series {
                number: 2,
                values: vec![8000, 8001, 8002]
            })
            .is_none()
        );

        assert!(
            acc.process_line(ParsedLine::Series {
                number: 3,
                values: vec![4000, 4001, 4002]
            })
            .is_none()
        );

        let cycle = acc.process_line(ParsedLine::EndCycle).unwrap();
        assert_eq!(cycle.dark.values, vec![100, 101, 102]);
        assert_eq!(cycle.full.values, vec![8000, 8001, 8002]);
        assert_eq!(cycle.sample.values, vec![4000, 4001, 4002]);
    }

    #[test]
    fn test_cycle_accumulator_partial_cycle() {
        let mut acc = CycleAccumulator::new();

        acc.process_line(ParsedLine::Series {
            number: 1,
            values: vec![100],
        });
        acc.process_line(ParsedLine::Series {
            number: 2,
            values: vec![8000],
        });

        assert!(acc.process_line(ParsedLine::EndCycle).is_none());
        assert!(acc.has_partial_data());
        assert_eq!(acc.missing_series(), vec![3]);
    }

    #[test]
    fn test_cycle_accumulator_reset() {
        let mut acc = CycleAccumulator::new();

        acc.process_line(ParsedLine::Series {
            number: 1,
            values: vec![100],
        });
        assert!(acc.has_partial_data());

        acc.reset();
        assert!(!acc.has_partial_data());
        assert_eq!(acc.missing_series(), vec![1, 2, 3]);
    }

    #[test]
    fn test_cycle_accumulator_ignores_non_series_lines() {
        let mut acc = CycleAccumulator::new();

        assert!(acc.process_line(ParsedLine::GainSet(4)).is_none());
        assert!(acc.process_line(ParsedLine::AdcReady).is_none());
        assert!(!acc.has_partial_data());
    }
}
