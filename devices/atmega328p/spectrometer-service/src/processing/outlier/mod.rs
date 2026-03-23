pub mod grubbs;
pub mod none;

use std::collections::HashSet;

/// Trait for pluggable outlier exclusion algorithms
pub trait OutlierExcluder: Send + Sync {
    /// Returns indices of values to exclude as outliers
    fn find_outliers(&self, values: &[f64]) -> Vec<usize>;

    /// Filter values, returning only non-outliers
    fn filter(&self, values: &[f64]) -> Vec<f64> {
        let outlier_indices: HashSet<_> = self.find_outliers(values).into_iter().collect();

        values
            .iter()
            .enumerate()
            .filter(|(i, _)| !outlier_indices.contains(i))
            .map(|(_, &v)| v)
            .collect()
    }

    /// Name of the algorithm for logging/debugging
    fn name(&self) -> &'static str;
}

/// Configuration for outlier exclusion method
#[derive(Debug, Clone)]
pub enum OutlierMethod {
    /// No outlier exclusion
    None,
    /// Grubbs' test with given significance level (alpha)
    Grubbs { alpha: f64 },
}

impl Default for OutlierMethod {
    fn default() -> Self {
        // Enabled by default with alpha = 0.05
        OutlierMethod::Grubbs { alpha: 0.05 }
    }
}

impl OutlierMethod {
    /// Create an outlier excluder instance
    pub fn create(&self) -> Box<dyn OutlierExcluder> {
        match self {
            OutlierMethod::None => Box::new(none::NoOutlierExcluder),
            OutlierMethod::Grubbs { alpha } => Box::new(grubbs::GrubbsExcluder::new(*alpha)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_removes_outliers() {
        struct MockExcluder;
        impl OutlierExcluder for MockExcluder {
            fn find_outliers(&self, _values: &[f64]) -> Vec<usize> {
                vec![1, 3]
            }
            fn name(&self) -> &'static str {
                "mock"
            }
        }

        let excluder = MockExcluder;
        let values = vec![1.0, 100.0, 2.0, 200.0, 3.0];
        let filtered = excluder.filter(&values);

        assert_eq!(filtered, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_outlier_method_default_is_grubbs() {
        let method = OutlierMethod::default();
        assert!(matches!(method, OutlierMethod::Grubbs { .. }));
    }
}
