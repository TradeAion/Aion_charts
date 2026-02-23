//! Line series data — lightweight (timestamp, value) columnar storage.
//!
//! Separate from the OHLCV `BarArray` — line series only need a single
//! value per data point (e.g. SMA, EMA, or any indicator output).

use crate::core::series::validation::{ensure_equal_len, ensure_strictly_increasing_timestamps};

/// A single line data point.
#[derive(Debug, Clone, Copy)]
pub struct LinePoint {
    pub timestamp: u64,
    pub value: f32,
}

/// Columnar storage for line series data.
#[derive(Debug, Clone)]
pub struct LineDataArray {
    pub timestamps: Vec<u64>,
    pub values: Vec<f32>,
    len: usize,
}

impl LineDataArray {
    pub fn new() -> Self {
        Self {
            timestamps: Vec::new(),
            values: Vec::new(),
            len: 0,
        }
    }

    /// Replace all data points.
    pub fn set(&mut self, points: Vec<LinePoint>) {
        let len = points.len();
        self.timestamps.clear();
        self.values.clear();
        self.timestamps.reserve(len);
        self.values.reserve(len);

        for p in points {
            self.timestamps.push(p.timestamp);
            self.values.push(Self::sanitize_value(p.value));
        }
        self.len = len;
    }

    /// Set data from parallel arrays (used by WASM layer).
    pub fn set_from_arrays(&mut self, timestamps: &[u64], values: &[f32]) -> Result<(), String> {
        ensure_equal_len("timestamps", timestamps.len(), "values", values.len())?;
        ensure_strictly_increasing_timestamps("line", timestamps)?;
        let len = timestamps.len();
        self.timestamps.clear();
        self.values.clear();
        self.timestamps.extend_from_slice(&timestamps[..len]);
        for &v in &values[..len] {
            self.values.push(Self::sanitize_value(v));
        }
        self.len = len;
        Ok(())
    }

    /// Append a single data point.
    pub fn push(&mut self, point: LinePoint) {
        self.timestamps.push(point.timestamp);
        self.values.push(Self::sanitize_value(point.value));
        self.len += 1;
    }

    /// Update the last point in-place. Returns false if the series is empty.
    pub fn update_last(&mut self, point: LinePoint) -> bool {
        if self.len == 0 {
            return false;
        }
        let idx = self.len - 1;
        self.timestamps[idx] = point.timestamp;
        self.values[idx] = Self::sanitize_value(point.value);
        true
    }

    /// Last timestamp, if any.
    #[inline]
    pub fn last_timestamp(&self) -> Option<u64> {
        self.timestamps.last().copied()
    }

    #[inline]
    pub fn get(&self, i: usize) -> LinePoint {
        LinePoint {
            timestamp: self.timestamps[i],
            value: self.values[i],
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    fn sanitize_value(v: f32) -> f32 {
        if v.is_finite() {
            v
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LineDataArray;

    #[test]
    fn set_from_arrays_rejects_mismatch() {
        let mut arr = LineDataArray::new();
        let err = arr.set_from_arrays(&[1, 2], &[10.0]).unwrap_err();
        assert!(err.contains("length mismatch"));
    }

    #[test]
    fn set_from_arrays_rejects_non_increasing_timestamps() {
        let mut arr = LineDataArray::new();
        let err = arr
            .set_from_arrays(&[1, 2, 2], &[10.0, 11.0, 12.0])
            .unwrap_err();
        assert!(err.contains("strictly increasing"));
    }
}
