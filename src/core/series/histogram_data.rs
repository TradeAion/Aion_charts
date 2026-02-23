//! Histogram data storage — columnar arrays for timestamp, value, and optional per-bar color.
//!
//! Like LWC, each histogram bar can have an individual color override.
//! If no per-bar color is set, the series default color is used.

use crate::core::series::validation::{ensure_equal_len, ensure_strictly_increasing_timestamps};

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
    pub fn set_data(&mut self, data: Vec<HistogramPoint>) {
        self.timestamps.clear();
        self.values.clear();
        self.colors.clear();
        self.timestamps.reserve(data.len());
        self.values.reserve(data.len());
        self.colors.reserve(data.len());
        for p in data {
            self.timestamps.push(p.timestamp);
            self.values.push(Self::sanitize_value(p.value));
            self.colors.push(Self::sanitize_color(p.color));
        }
    }

    /// Set data from parallel arrays (no per-bar color — all default).
    pub fn set_from_arrays(&mut self, timestamps: &[u64], values: &[f32]) -> Result<(), String> {
        ensure_equal_len("timestamps", timestamps.len(), "values", values.len())?;
        ensure_strictly_increasing_timestamps("histogram", timestamps)?;
        let count = timestamps.len();
        self.timestamps = timestamps[..count].to_vec();
        self.values = values[..count]
            .iter()
            .map(|&v| Self::sanitize_value(v))
            .collect();
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
        self.values = values[..count]
            .iter()
            .map(|&v| Self::sanitize_value(v))
            .collect();
        self.colors = colors[..count]
            .iter()
            .map(|&c| Self::sanitize_color(c))
            .collect();
        Ok(())
    }

    /// Append a single histogram point.
    pub fn push(&mut self, point: HistogramPoint) {
        self.timestamps.push(point.timestamp);
        self.values.push(Self::sanitize_value(point.value));
        self.colors.push(Self::sanitize_color(point.color));
    }

    /// Update the last point in-place. Returns false if the series is empty.
    pub fn update_last(&mut self, point: HistogramPoint) -> bool {
        if self.values.is_empty() {
            return false;
        }
        let idx = self.values.len() - 1;
        self.timestamps[idx] = point.timestamp;
        self.values[idx] = Self::sanitize_value(point.value);
        self.colors[idx] = Self::sanitize_color(point.color);
        true
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
    fn sanitize_value(v: f32) -> f32 {
        if v.is_finite() {
            v
        } else {
            0.0
        }
    }

    #[inline]
    fn sanitize_color(c: [f32; 4]) -> [f32; 4] {
        [
            if c[0].is_finite() { c[0] } else { 0.0 },
            if c[1].is_finite() { c[1] } else { 0.0 },
            if c[2].is_finite() { c[2] } else { 0.0 },
            if c[3].is_finite() { c[3] } else { 0.0 },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::HistogramDataArray;

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
}
