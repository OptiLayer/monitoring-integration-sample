/// Calibration processor for converting raw ADC values to percentage
///
/// Formula: (sample - dark) / (full - dark) * 100
pub struct CalibrationProcessor;

impl CalibrationProcessor {
    pub fn new() -> Self {
        Self
    }

    /// Calculate calibrated reading as percentage
    ///
    /// Returns value in range 0-100 (can exceed if sample > full)
    /// Returns 0.0 if full == dark (division by zero case)
    pub fn calculate(&self, dark_mean: f64, full_mean: f64, sample_mean: f64) -> f64 {
        let denominator = full_mean - dark_mean;

        if denominator.abs() < f64::EPSILON {
            return 0.0;
        }

        ((sample_mean - dark_mean) / denominator) * 100.0
    }
}

impl Default for CalibrationProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate arithmetic mean of values
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    #[test]
    fn test_calibration_basic() {
        let processor = CalibrationProcessor::new();

        // dark=100, full=1000, sample=550 -> (550-100)/(1000-100)*100 = 50%
        let result = processor.calculate(100.0, 1000.0, 550.0);
        assert_relative_eq!(result, 50.0, epsilon = 0.01);
    }

    #[test]
    fn test_calibration_zero_percent() {
        let processor = CalibrationProcessor::new();

        // sample == dark -> 0%
        let result = processor.calculate(100.0, 1000.0, 100.0);
        assert_relative_eq!(result, 0.0, epsilon = 0.01);
    }

    #[test]
    fn test_calibration_hundred_percent() {
        let processor = CalibrationProcessor::new();

        // sample == full -> 100%
        let result = processor.calculate(100.0, 1000.0, 1000.0);
        assert_relative_eq!(result, 100.0, epsilon = 0.01);
    }

    #[test]
    fn test_calibration_exceeds_hundred() {
        let processor = CalibrationProcessor::new();

        // sample > full -> > 100%
        let result = processor.calculate(100.0, 1000.0, 1100.0);
        assert!(result > 100.0);
    }

    #[test]
    fn test_calibration_negative() {
        let processor = CalibrationProcessor::new();

        // sample < dark -> negative %
        let result = processor.calculate(100.0, 1000.0, 50.0);
        assert!(result < 0.0);
    }

    #[test]
    fn test_calibration_division_by_zero() {
        let processor = CalibrationProcessor::new();

        // full == dark -> division by zero, returns 0
        let result = processor.calculate(100.0, 100.0, 50.0);
        assert_relative_eq!(result, 0.0, epsilon = 0.01);
    }

    #[test]
    fn test_mean_basic() {
        let values = vec![10.0, 20.0, 30.0];
        assert_relative_eq!(mean(&values), 20.0, epsilon = 0.01);
    }

    #[test]
    fn test_mean_empty() {
        let values: Vec<f64> = vec![];
        assert_relative_eq!(mean(&values), 0.0, epsilon = 0.01);
    }

    #[test]
    fn test_mean_single_value() {
        let values = vec![42.0];
        assert_relative_eq!(mean(&values), 42.0, epsilon = 0.01);
    }
}
