//! Line series data — lightweight (timestamp, value) columnar storage.
//!
//! Separate from the OHLCV `BarArray` — line series only need a single
//! value per data point (e.g. SMA, EMA, or any indicator output).

/// A single line data point.
#[derive(Debug, Clone, Copy)]
pub struct LinePoint {
    pub timestamp: u64,
    pub value: f32,
}

/// Columnar storage for line series data.
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
            let v = if p.value.is_finite() { p.value } else { 0.0 };
            self.values.push(v);
        }
        self.len = len;
    }

    /// Set data from parallel arrays (used by WASM layer).
    pub fn set_from_arrays(&mut self, timestamps: &[u64], values: &[f32]) {
        let len = timestamps.len().min(values.len());
        self.timestamps.clear();
        self.values.clear();
        self.timestamps.extend_from_slice(&timestamps[..len]);
        self.values.clear();
        for &v in &values[..len] {
            self.values.push(if v.is_finite() { v } else { 0.0 });
        }
        self.len = len;
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
}
