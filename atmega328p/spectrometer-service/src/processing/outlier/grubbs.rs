use statrs::distribution::{ContinuousCDF, StudentsT};

use super::OutlierExcluder;

/// Grubbs' test for outlier detection
///
/// Iteratively identifies and removes outliers from a dataset.
/// Requires at least 3 values to perform the test.
pub struct GrubbsExcluder {
    alpha: f64,
}

impl GrubbsExcluder {
    pub fn new(alpha: f64) -> Self {
        Self { alpha }
    }

    /// Calculate Grubbs' test statistic for a specific value
    fn grubbs_statistic(values: &[f64], index: usize) -> f64 {
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return 0.0;
        }

        (values[index] - mean).abs() / std_dev
    }

    /// Calculate critical value for Grubbs' test
    fn critical_value(&self, n: usize) -> f64 {
        let n = n as f64;
        let df = n - 2.0;

        if df <= 0.0 {
            return f64::INFINITY;
        }

        // Two-tailed t critical value
        let t_dist = StudentsT::new(0.0, 1.0, df).unwrap();
        let t_crit = t_dist.inverse_cdf(1.0 - self.alpha / (2.0 * n));

        // Grubbs critical value formula
        ((n - 1.0) / n.sqrt()) * (t_crit.powi(2) / (n - 2.0 + t_crit.powi(2))).sqrt()
    }
}

impl OutlierExcluder for GrubbsExcluder {
    fn find_outliers(&self, values: &[f64]) -> Vec<usize> {
        if values.len() < 3 {
            return Vec::new();
        }

        let mut outliers = Vec::new();
        let mut remaining: Vec<(usize, f64)> =
            values.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        loop {
            if remaining.len() < 3 {
                break;
            }

            let current_values: Vec<f64> = remaining.iter().map(|(_, v)| *v).collect();
            let critical = self.critical_value(current_values.len());

            // Find value with maximum Grubbs statistic
            let mut max_idx = 0;
            let mut max_g = 0.0;

            for (i, _) in remaining.iter().enumerate() {
                let g = Self::grubbs_statistic(&current_values, i);
                if g > max_g {
                    max_g = g;
                    max_idx = i;
                }
            }

            if max_g > critical {
                let (original_idx, _) = remaining.remove(max_idx);
                outliers.push(original_idx);
            } else {
                break;
            }
        }

        outliers
    }

    fn name(&self) -> &'static str {
        "Grubbs"
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    #[test]
    fn test_grubbs_no_outliers() {
        let excluder = GrubbsExcluder::new(0.05);
        let values = vec![10.0, 11.0, 10.5, 10.2, 10.8];
        let outliers = excluder.find_outliers(&values);

        assert!(outliers.is_empty());
    }

    #[test]
    fn test_grubbs_single_outlier() {
        let excluder = GrubbsExcluder::new(0.05);
        let values = vec![10.0, 11.0, 10.5, 100.0, 10.2]; // 100.0 is obvious outlier
        let outliers = excluder.find_outliers(&values);

        assert_eq!(outliers.len(), 1);
        assert_eq!(outliers[0], 3); // Index of 100.0
    }

    #[test]
    fn test_grubbs_multiple_outliers() {
        let excluder = GrubbsExcluder::new(0.05);
        // More values needed for reliable outlier detection
        let values = vec![
            10.0, 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7,  // Normal values
            500.0, // Clear outlier
        ];
        let outliers = excluder.find_outliers(&values);

        assert!(outliers.len() >= 1);
        assert!(outliers.contains(&8)); // Index of 500.0
    }

    #[test]
    fn test_grubbs_small_dataset() {
        let excluder = GrubbsExcluder::new(0.05);

        // Less than 3 values - no outlier detection possible
        let values = vec![10.0, 100.0];
        let outliers = excluder.find_outliers(&values);
        assert!(outliers.is_empty());

        let values = vec![10.0];
        let outliers = excluder.find_outliers(&values);
        assert!(outliers.is_empty());
    }

    #[test]
    fn test_grubbs_identical_values() {
        let excluder = GrubbsExcluder::new(0.05);
        let values = vec![10.0, 10.0, 10.0, 10.0, 10.0];
        let outliers = excluder.find_outliers(&values);

        assert!(outliers.is_empty()); // No outliers when all values are identical
    }

    #[test]
    fn test_grubbs_filter() {
        let excluder = GrubbsExcluder::new(0.05);
        let values = vec![10.0, 11.0, 10.5, 100.0, 10.2];
        let filtered = excluder.filter(&values);

        // 100.0 should be removed
        assert!(!filtered.contains(&100.0));
        assert!(filtered.contains(&10.0));
        assert!(filtered.contains(&11.0));
    }

    #[test]
    fn test_grubbs_different_alpha_levels() {
        // More strict (lower alpha) should find fewer outliers
        let strict = GrubbsExcluder::new(0.01);
        let lenient = GrubbsExcluder::new(0.10);

        // Borderline outlier case
        let values = vec![10.0, 11.0, 10.5, 15.0, 10.2];

        let strict_outliers = strict.find_outliers(&values);
        let lenient_outliers = lenient.find_outliers(&values);

        // Lenient should find at least as many outliers as strict
        assert!(lenient_outliers.len() >= strict_outliers.len());
    }

    #[test]
    fn test_grubbs_statistic_calculation() {
        let values = vec![10.0, 10.0, 10.0, 100.0];
        let g = GrubbsExcluder::grubbs_statistic(&values, 3);

        // The outlier should have a high G statistic
        assert!(g > 1.0);
    }
}
