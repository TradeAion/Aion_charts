//! Line series data — lightweight (timestamp, value) columnar storage.
//!
//! Separate from the OHLCV `BarArray` — line series only need a single
//! value per data point (e.g. SMA, EMA, or any indicator output).

use crate::core::series::validation::{
    ensure_equal_len, ensure_finite_line_point, ensure_finite_value,
    ensure_strictly_increasing_timestamps,
};

/// A single line data point.
#[derive(Debug, Clone, Copy)]
pub struct LinePoint {
    pub timestamp: u64,
    pub value: f64,
}

/// Columnar storage for line series data.
#[derive(Debug, Clone)]
pub struct LineDataArray {
    pub timestamps: Vec<u64>,
    pub values: Vec<f64>,
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
    pub fn set(&mut self, points: Vec<LinePoint>) -> Result<(), String> {
        for i in 1..points.len() {
            if points[i].timestamp <= points[i - 1].timestamp {
                return Err(format!(
                    "line timestamps must be strictly increasing at index {}: {} <= {}",
                    i,
                    points[i].timestamp,
                    points[i - 1].timestamp
                ));
            }
        }
        for (index, point) in points.iter().enumerate() {
            Self::validate_point("set", point, index)?;
        }

        let len = points.len();
        self.timestamps.clear();
        self.values.clear();
        self.timestamps.reserve(len);
        self.values.reserve(len);

        for p in points {
            self.timestamps.push(p.timestamp);
            self.values.push(p.value);
        }
        self.len = len;
        Ok(())
    }

    /// Set data from parallel arrays (used by WASM layer).
    pub fn set_from_arrays(&mut self, timestamps: &[u64], values: &[f64]) -> Result<(), String> {
        ensure_equal_len("timestamps", timestamps.len(), "values", values.len())?;
        ensure_strictly_increasing_timestamps("line", timestamps)?;
        let len = timestamps.len();
        self.timestamps.clear();
        self.values.clear();
        self.timestamps.extend_from_slice(&timestamps[..len]);
        for (index, &v) in values[..len].iter().enumerate() {
            Self::validate_value("set_from_arrays", v, index)?;
            self.values.push(v);
        }
        self.len = len;
        Ok(())
    }

    /// Append a single data point.
    pub fn push(&mut self, point: LinePoint) -> Result<(), String> {
        Self::validate_point("push", &point, self.len)?;
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
        self.len += 1;
        Ok(())
    }

    /// Update the last point in-place. Returns false if the series is empty.
    pub fn update_last(&mut self, point: LinePoint) -> Result<bool, String> {
        if self.len == 0 {
            return Ok(false);
        }
        Self::validate_point("update_last", &point, self.len - 1)?;
        let last_ts = self.timestamps[self.len - 1];
        if point.timestamp != last_ts {
            return Err(format!(
                "update_last requires timestamp == last timestamp ({} != {})",
                point.timestamp, last_ts
            ));
        }
        let idx = self.len - 1;
        self.timestamps[idx] = point.timestamp;
        self.values[idx] = point.value;
        Ok(true)
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

    fn validate_point(context: &str, point: &LinePoint, index: usize) -> Result<(), String> {
        ensure_finite_line_point(&format!("line_data::{context}"), point, index)
    }

    #[inline]
    fn validate_value(context: &str, value: f64, index: usize) -> Result<(), String> {
        ensure_finite_value(&format!("line_data::{context}"), "value", value, index)
    }
}

#[cfg(test)]
mod tests {
    use super::{LineDataArray, LinePoint};

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

    #[test]
    fn set_rejects_non_finite_points() {
        let mut arr = LineDataArray::new();
        let err = arr
            .set(vec![LinePoint {
                timestamp: 1,
                value: f64::NAN,
            }])
            .unwrap_err();
        assert!(err.contains("must be finite"));
    }

    #[test]
    fn push_rejects_non_finite_points() {
        let mut arr = LineDataArray::new();
        let err = arr
            .push(LinePoint {
                timestamp: 1,
                value: f64::INFINITY,
            })
            .unwrap_err();
        assert!(err.contains("must be finite"));
    }

    #[test]
    fn update_last_rejects_non_finite_points() {
        let mut arr = LineDataArray::new();
        arr.push(LinePoint {
            timestamp: 1,
            value: 10.0,
        })
        .unwrap();

        let err = arr
            .update_last(LinePoint {
                timestamp: 1,
                value: f64::NEG_INFINITY,
            })
            .unwrap_err();
        assert!(err.contains("must be finite"));
    }
}
