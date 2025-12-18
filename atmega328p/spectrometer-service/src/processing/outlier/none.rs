use super::OutlierExcluder;

/// No-op outlier excluder - passes through all values
pub struct NoOutlierExcluder;

impl OutlierExcluder for NoOutlierExcluder {
    fn find_outliers(&self, _values: &[f64]) -> Vec<usize> {
        Vec::new()
    }

    fn filter(&self, values: &[f64]) -> Vec<f64> {
        values.to_vec()
    }

    fn name(&self) -> &'static str {
        "None"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_outlier_excluder_returns_empty() {
        let excluder = NoOutlierExcluder;
        let values = vec![1.0, 100.0, 2.0, 200.0];
        let outliers = excluder.find_outliers(&values);

        assert!(outliers.is_empty());
    }

    #[test]
    fn test_no_outlier_excluder_filter_returns_all() {
        let excluder = NoOutlierExcluder;
        let values = vec![1.0, 100.0, 2.0, 200.0];
        let filtered = excluder.filter(&values);

        assert_eq!(filtered, values);
    }
}
