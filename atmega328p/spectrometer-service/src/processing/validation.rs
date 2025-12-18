/// Measurement validator
///
/// Validates that measurements follow expected relationship: full > sample > dark
pub struct MeasurementValidator;

impl MeasurementValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate measurement relationship: full > sample > dark
    ///
    /// Returns Ok(()) if valid, Err with description if invalid
    pub fn validate(&self, dark_mean: f64, full_mean: f64, sample_mean: f64) -> Result<(), String> {
        if full_mean <= dark_mean {
            return Err(format!(
                "full ({:.2}) must be greater than dark ({:.2})",
                full_mean, dark_mean
            ));
        }

        if sample_mean <= dark_mean {
            return Err(format!(
                "sample ({:.2}) must be greater than dark ({:.2})",
                sample_mean, dark_mean
            ));
        }

        if sample_mean >= full_mean {
            return Err(format!(
                "sample ({:.2}) must be less than full ({:.2})",
                sample_mean, full_mean
            ));
        }

        Ok(())
    }

    /// Validate with warnings instead of errors for edge cases
    ///
    /// Returns (is_valid, optional_warning)
    pub fn validate_with_warnings(
        &self,
        dark_mean: f64,
        full_mean: f64,
        sample_mean: f64,
    ) -> (bool, Option<String>) {
        match self.validate(dark_mean, full_mean, sample_mean) {
            Ok(()) => (true, None),
            Err(msg) => (false, Some(msg)),
        }
    }
}

impl Default for MeasurementValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_measurement() {
        let validator = MeasurementValidator::new();

        // Valid: full (1000) > sample (500) > dark (100)
        let result = validator.validate(100.0, 1000.0, 500.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_sample_greater_than_full() {
        let validator = MeasurementValidator::new();

        // Invalid: sample > full
        let result = validator.validate(100.0, 1000.0, 1100.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sample"));
    }

    #[test]
    fn test_invalid_sample_equals_full() {
        let validator = MeasurementValidator::new();

        // Invalid: sample == full
        let result = validator.validate(100.0, 1000.0, 1000.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_dark_greater_than_sample() {
        let validator = MeasurementValidator::new();

        // Invalid: dark > sample
        let result = validator.validate(600.0, 1000.0, 500.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sample"));
    }

    #[test]
    fn test_invalid_dark_equals_sample() {
        let validator = MeasurementValidator::new();

        // Invalid: dark == sample
        let result = validator.validate(500.0, 1000.0, 500.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_full_less_than_dark() {
        let validator = MeasurementValidator::new();

        // Invalid: full < dark
        let result = validator.validate(1000.0, 500.0, 300.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("full"));
    }

    #[test]
    fn test_boundary_values() {
        let validator = MeasurementValidator::new();

        // Just barely valid
        let result = validator.validate(100.0, 102.0, 101.0);
        assert!(result.is_ok());

        // Floating point edge case
        let result = validator.validate(0.0, 1.0, 0.5);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_with_warnings() {
        let validator = MeasurementValidator::new();

        let (valid, warning) = validator.validate_with_warnings(100.0, 1000.0, 500.0);
        assert!(valid);
        assert!(warning.is_none());

        let (valid, warning) = validator.validate_with_warnings(100.0, 1000.0, 1100.0);
        assert!(!valid);
        assert!(warning.is_some());
    }
}
