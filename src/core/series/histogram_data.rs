//! Histogram data storage — columnar arrays for timestamp, value, and optional per-bar color.
//!
//! Like LWC, each histogram bar can have an individual color override.
//! If no per-bar color is set, the series default color is used.

use crate::core::series::validation::{
    ensure_equal_len, ensure_finite_color, ensure_finite_histogram_point, ensure_finite_value,
    ensure_strictly_increasing_timestamps,
};

/// A single histogram data point.
#[derive(Debug, Clone, Copy)]
pub struct HistogramPoint {
    pub timestamp: u64,
    pub value: f32,
    /// Optional per-bar color override [R, G, B, A]. If all zeros, use series default.
    pub color: [f32; 4],
}

/// Columnar storage for histogram data.
#[derive(Debug, Clone, Default)]
pub struct HistogramDataArray {
    pub timestamps: Vec<u64>,
    pub values: Vec<f32>,
    /// Per-bar color overrides. Same length as `values`.
    /// `[0,0,0,0]` means "use series default color".
    pub colors: Vec<[f32; 4]>,
}

impl HistogramDataArray {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Set data from a vector of HistogramPoint.
    pub fn set_data(&mut self, data: Vec<HistogramPoint>) -> Result<(), String> {
        for i in 1..data.len() {
            if data[i].timestamp <= data[i - 1].timestamp {
                return Err(format!(
                    "histogram timestamps must be strictly increasing at index {}: {} <= {}",
                    i,
                    data[i].timestamp,
                    data[i - 1].timestamp
                ));
            }
        }
        self.timestamps.clear();
        self.values.clear();
        self.colors.clear();
        self.timestamps.reserve(data.len());
        self.values.reserve(data.len());
        self.colors.reserve(data.len());
        for (index, p) in data.into_iter().enumerate() {
            Self::validate_point("set_data", &p, index)?;
            self.timestamps.push(p.timestamp);
            self.values.push(p.value);
            self.colors.push(p.color);
        }
        Ok(())
    }

    /// Set data from parallel arrays (no per-bar color — all default).
    pub fn set_from_arrays(&mut self, timestamps: &[u64], values: &[f32]) -> Result<(), String> {
        ensure_equal_len("timestamps", timestamps.len(), "values", values.len())?;
        ensure_strictly_increasing_timestamps("histogram", timestamps)?;
        let count = timestamps.len();
        self.timestamps = timestamps[..count].to_vec();
        self.values.clear();
        self.values.reserve(count);
        for (index, &value) in values[..count].iter().enumerate() {
            Self::validate_value("set_from_arrays", value, index)?;
            self.values.push(value);
        }
        self.colors = vec![[0.0; 4]; count]; // all zeros = use default
        Ok(())
    }

    /// Set data from parallel arrays with per-bar colors.
    pub fn set_from_arrays_with_colors(
        &mut self,
        timestamps: &[u64],
        values: &[f32],
        colors: &[[f32; 4]],
    ) -> Result<(), String> {
        ensure_equal_len("timestamps", timestamps.len(), "values", values.len())?;
        ensure_equal_len("timestamps", timestamps.len(), "colors", colors.len())?;
        ensure_strictly_increasing_timestamps("histogram", timestamps)?;
        let count = timestamps.len();
        self.timestamps = timestamps[..count].to_vec();
        self.values.clear();
        self.values.reserve(count);
        self.colors.clear();
        self.colors.reserve(count);
        for (index, (&value, &color)) in values[..count]
            .iter()
            .zip(colors[..count].iter())
            .enumerate()
        {
            Self::validate_value("set_from_arrays_with_colors", value, index)?;
            Self::validate_color("set_from_arrays_with_colors", color, index)?;
            self.values.push(value);
            self.colors.push(color);
        }
        Ok(())
    }

    /// Append a single histogram point.
    pub fn push(&mut self, point: HistogramPoint) -> Result<(), String> {
        Self::validate_point("push", &point, self.values.len())?;
        if let Some(last_ts) = self.last_timestamp() {
            if point.timestamp <= last_ts {
                return Err(format!(
                    "push requires timestamp > last timestamp ({} <= {})",
                    point.timestamp, last_ts
                ));
            }
        }
        self.timestamps.push(point.timestamp);
        self.values.push(point.value);
        self.colors.push(point.color);
        Ok(())
    }

    /// Update the last point in-place. Returns false if the series is empty.
    pub fn update_last(&mut self, point: HistogramPoint) -> Result<bool, String> {
        if self.values.is_empty() {
            return Ok(false);
        }
        Self::validate_point("update_last", &point, self.values.len() - 1)?;
        let last_ts = self.timestamps[self.values.len() - 1];
        if point.timestamp != last_ts {
            return Err(format!(
                "update_last requires timestamp == last timestamp ({} != {})",
                point.timestamp, last_ts
            ));
        }
        let idx = self.values.len() - 1;
        self.timestamps[idx] = point.timestamp;
        self.values[idx] = point.value;
        self.colors[idx] = point.color;
        Ok(true)
    }

    /// Last timestamp, if any.
    #[inline]
    pub fn last_timestamp(&self) -> Option<u64> {
        self.timestamps.last().copied()
    }

    /// Returns true if the bar at index `i` has a per-bar color override.
    #[inline]
    pub fn has_color_override(&self, i: usize) -> bool {
        if i >= self.colors.len() {
            return false;
        }
        let c = self.colors[i];
        // If alpha > 0, it's a real override
        c[3] > 0.0
    }

    /// Get the effective color for bar `i`, falling back to `default_color`.
    #[inline]
    pub fn effective_color(&self, i: usize, default_color: [f32; 4]) -> [f32; 4] {
        if self.has_color_override(i) {
            self.colors[i]
        } else {
            default_color
        }
    }

    #[inline]
    fn validate_point(context: &str, point: &HistogramPoint, index: usize) -> Result<(), String> {
        ensure_finite_histogram_point(&format!("histogram_data::{context}"), point, index)
    }

    #[inline]
    fn validate_value(context: &str, value: f32, index: usize) -> Result<(), String> {
        ensure_finite_value(&format!("histogram_data::{context}"), "value", value, index)
    }

    #[inline]
    fn validate_color(context: &str, color: [f32; 4], index: usize) -> Result<(), String> {
        ensure_finite_color(&format!("histogram_data::{context}"), color, index)
    }
}

#[cfg(test)]
mod tests {
    use super::{HistogramDataArray, HistogramPoint};

    #[test]
    fn set_from_arrays_rejects_mismatch() {
        let mut arr = HistogramDataArray::new();
        let err = arr.set_from_arrays(&[1, 2], &[10.0]).unwrap_err();
        assert!(err.contains("length mismatch"));
    }

    #[test]
    fn set_from_arrays_rejects_non_increasing_timestamps() {
        let mut arr = HistogramDataArray::new();
        let err = arr
            .set_from_arrays(&[1, 2, 2], &[10.0, 11.0, 12.0])
            .unwrap_err();
        assert!(err.contains("strictly increasing"));
    }

    #[test]
    fn set_from_arrays_with_colors_rejects_mismatch() {
        let mut arr = HistogramDataArray::new();
        let err = arr
            .set_from_arrays_with_colors(&[1, 2], &[10.0, 11.0], &[[1.0, 1.0, 1.0, 1.0]])
            .unwrap_err();
        assert!(err.contains("length mismatch"));
    }

    #[test]
    fn set_data_rejects_non_finite_value() {
        let mut arr = HistogramDataArray::new();
        let err = arr
            .set_data(vec![HistogramPoint {
                timestamp: 1,
                value: f32::NAN,
                color: [0.0; 4],
            }])
            .unwrap_err();
        assert!(err.contains("must be finite"));
    }

    #[test]
    fn set_from_arrays_with_colors_rejects_non_finite_color() {
        let mut arr = HistogramDataArray::new();
        let err = arr
            .set_from_arrays_with_colors(&[1], &[10.0], &[[1.0, f32::INFINITY, 1.0, 1.0]])
            .unwrap_err();
        assert!(err.contains("must be finite"));
    }

    #[test]
    fn push_rejects_non_increasing_timestamp() {
        let mut arr = HistogramDataArray::new();
        arr.push(HistogramPoint {
            timestamp: 2,
            value: 10.0,
            color: [0.0; 4],
        })
        .unwrap();
        let err = arr
            .push(HistogramPoint {
                timestamp: 2,
                value: 11.0,
                color: [0.0; 4],
            })
            .unwrap_err();
        assert!(err.contains("timestamp > last timestamp"));
    }

    #[test]
    fn update_last_rejects_timestamp_mismatch() {
        let mut arr = HistogramDataArray::new();
        arr.push(HistogramPoint {
            timestamp: 2,
            value: 10.0,
            color: [0.0; 4],
        })
        .unwrap();
        let err = arr
            .update_last(HistogramPoint {
                timestamp: 3,
                value: 11.0,
                color: [0.0; 4],
            })
            .unwrap_err();
        assert!(err.contains("timestamp == last timestamp"));
    }
}
