/// Get percentile value for a sorted array of u64s sorted in non-descending order, using the
/// exclusive method (but also accounts for 0th and 100th percentile). The exclusive method is the
/// preferred way of calculating percentile for discrete data points. Sizes of volumes, pools and
/// replica counts are discrete. This is more reliable for percentile values between 10 and 90.
pub fn percentile_exclusive(sorted_data: &[u64], percentile: f64) -> Option<f64> {
    match sorted_data.len() {
        // Percentile for an empty data set is not defined.
        0 => None,
        // Only one data point. This is the only possible percentile value.
        1 => Some(sorted_data[0] as f64),
        len => {
            // Position of the required percentile value.
            let pos = (len as f64 + 1.0) * percentile / 100.0;

            // Positions before the first data point don't actually exist.
            if pos <= 1.0 {
                return Some(sorted_data[0] as f64);
            // Positions beyond the length of the data set don't actually exist.
            } else if pos >= len as f64 {
                return Some(sorted_data[len - 1] as f64);
            }

            /* Interpolate the value at fractional (decimal) position, because our
             * data is discrete and our data only exists at distinct integer positions.
             *
             * If the position is not fractional, the interpolated value will be the value
             * at the index pos-1. This is because the pos-th value is at pos-1 position,
             * because rust arrays are 0 indexed.
             */

            // 1 is subtracted from position number of data point because rust arrays
            // are indexed from 0.
            let lower = pos.floor() as usize - 1;
            let upper = pos.ceil() as usize - 1;
            Some(interpolate(sorted_data[lower], sorted_data[upper], pos))
        }
    }
}

/// Interpolate the value at a position between two values directly ahead of and behind
/// the position.
pub fn interpolate(lower_value: u64, upper_value: u64, pos: f64) -> f64 {
    // fraction is 0 for non-fractional position, i.e. the data position is an integer.
    let fraction = pos - pos.floor();
    // For an integer position, the result is simply the lower_value.
    lower_value as f64 + fraction * (upper_value as f64 - lower_value as f64)
}

#[cfg(test)]
mod tests {
    use crate::math::{interpolate, percentile_exclusive};

    #[test]
    fn test_percentile_exclusive() {
        let sorted_data: &[u64; 7] = &[1, 2, 3, 4, 5, 6, 7];
        assert_eq!(percentile_exclusive(sorted_data, 30.0), Some(2.4));
        assert_eq!(percentile_exclusive(sorted_data, 50.0), Some(4.0));
        assert_eq!(percentile_exclusive(sorted_data, 75.0), Some(6.0));
        assert_eq!(percentile_exclusive(sorted_data, 0.0), Some(1.0));
        assert_eq!(percentile_exclusive(sorted_data, 100.0), Some(7.0));
    }

    #[test]
    fn test_interpolate() {
        assert_eq!(interpolate(10, 20, 5.5), 15.0);
        assert_eq!(interpolate(10, 20, 0.3), 13.0);
        assert_eq!(interpolate(10, 20, 50.0), 10.0);
    }
}
