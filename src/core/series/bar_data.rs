//! OHLC bar data storage — columnar arrays for timestamp, open, high, low, close.
//!
//! Used by the Bar (OHLC) series type. Similar to the main BarArray but
//! managed per-series (not the global candlestick data).

use crate::core::series::validation::{ensure_equal_len, ensure_strictly_increasing_timestamps};

/// A single OHLC data point.
#[derive(Debug, Clone, Copy)]
pub struct OhlcPoint {
    pub timestamp: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
}

/// Columnar storage for OHLC bar data.
#[derive(Debug, Clone, Default)]
pub struct OhlcDataArray {
    pub timestamps: Vec<u64>,
    pub open: Vec<f32>,
    pub high: Vec<f32>,
    pub low: Vec<f32>,
    pub close: Vec<f32>,
}

impl OhlcDataArray {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    /// Set data from a vector of OhlcPoint.
    pub fn set_data(&mut self, data: Vec<OhlcPoint>) {
        self.timestamps.clear();
        self.open.clear();
        self.high.clear();
        self.low.clear();
        self.close.clear();
        self.timestamps.reserve(data.len());
        self.open.reserve(data.len());
        self.high.reserve(data.len());
        self.low.reserve(data.len());
        self.close.reserve(data.len());
        for p in data {
            let p = Self::sanitize_point(p);
            self.timestamps.push(p.timestamp);
            self.open.push(p.open);
            self.high.push(p.high);
            self.low.push(p.low);
            self.close.push(p.close);
        }
    }

    /// Set data from parallel arrays.
    pub fn set_from_arrays(
        &mut self,
        timestamps: &[u64],
        open: &[f32],
        high: &[f32],
        low: &[f32],
        close: &[f32],
    ) -> Result<(), String> {
        ensure_equal_len("timestamps", timestamps.len(), "open", open.len())?;
        ensure_equal_len("timestamps", timestamps.len(), "high", high.len())?;
        ensure_equal_len("timestamps", timestamps.len(), "low", low.len())?;
        ensure_equal_len("timestamps", timestamps.len(), "close", close.len())?;
        ensure_strictly_increasing_timestamps("bar", timestamps)?;
        let count = timestamps.len();
        self.timestamps.clear();
        self.open.clear();
        self.high.clear();
        self.low.clear();
        self.close.clear();
        self.timestamps.reserve(count);
        self.open.reserve(count);
        self.high.reserve(count);
        self.low.reserve(count);
        self.close.reserve(count);

        for i in 0..count {
            let p = Self::sanitize_point(OhlcPoint {
                timestamp: timestamps[i],
                open: open[i],
                high: high[i],
                low: low[i],
                close: close[i],
            });
            self.timestamps.push(p.timestamp);
            self.open.push(p.open);
            self.high.push(p.high);
            self.low.push(p.low);
            self.close.push(p.close);
        }
        Ok(())
    }

    /// Append a single OHLC point.
    pub fn push(&mut self, point: OhlcPoint) {
        let p = Self::sanitize_point(point);
        self.timestamps.push(p.timestamp);
        self.open.push(p.open);
        self.high.push(p.high);
        self.low.push(p.low);
        self.close.push(p.close);
    }

    /// Update the last point in-place. Returns false if the series is empty.
    pub fn update_last(&mut self, point: OhlcPoint) -> bool {
        if self.timestamps.is_empty() {
            return false;
        }
        let p = Self::sanitize_point(point);
        let idx = self.timestamps.len() - 1;
        self.timestamps[idx] = p.timestamp;
        self.open[idx] = p.open;
        self.high[idx] = p.high;
        self.low[idx] = p.low;
        self.close[idx] = p.close;
        true
    }

    /// Last timestamp, if any.
    #[inline]
    pub fn last_timestamp(&self) -> Option<u64> {
        self.timestamps.last().copied()
    }

    /// Get a single point by index.
    #[inline]
    pub fn get(&self, i: usize) -> OhlcPoint {
        OhlcPoint {
            timestamp: self.timestamps[i],
            open: self.open[i],
            high: self.high[i],
            low: self.low[i],
            close: self.close[i],
        }
    }

    /// Returns true if the bar at index `i` is bullish (close >= open).
    #[inline]
    pub fn is_bullish(&self, i: usize) -> bool {
        self.close[i] >= self.open[i]
    }

    #[inline]
    fn sanitize_point(point: OhlcPoint) -> OhlcPoint {
        let open = if point.open.is_finite() {
            point.open
        } else {
            0.0
        };
        let close = if point.close.is_finite() {
            point.close
        } else {
            open
        };
        let high = if point.high.is_finite() {
            point.high.max(open).max(close)
        } else {
            open.max(close)
        };
        let low = if point.low.is_finite() {
            point.low.min(open).min(close)
        } else {
            open.min(close)
        };
        OhlcPoint {
            timestamp: point.timestamp,
            open,
            high,
            low,
            close,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OhlcDataArray, OhlcPoint};

    #[test]
    fn sanitize_point_enforces_ohlc_bounds() {
        let mut arr = OhlcDataArray::new();
        arr.push(OhlcPoint {
            timestamp: 1,
            open: 100.0,
            high: 90.0,
            low: 110.0,
            close: 105.0,
        });

        let p = arr.get(0);
        assert_eq!(p.high, 105.0);
        assert_eq!(p.low, 100.0);
    }

    #[test]
    fn sanitize_point_handles_non_finite() {
        let mut arr = OhlcDataArray::new();
        arr.push(OhlcPoint {
            timestamp: 1,
            open: f32::NAN,
            high: f32::NEG_INFINITY,
            low: f32::INFINITY,
            close: f32::NAN,
        });

        let p = arr.get(0);
        assert_eq!(p.open, 0.0);
        assert_eq!(p.close, 0.0);
        assert_eq!(p.high, 0.0);
        assert_eq!(p.low, 0.0);
    }

    #[test]
    fn set_from_arrays_rejects_mismatch() {
        let mut arr = OhlcDataArray::new();
        let err = arr
            .set_from_arrays(&[1, 2], &[1.0], &[2.0, 2.0], &[0.0, 0.0], &[1.0, 1.0])
            .unwrap_err();
        assert!(err.contains("length mismatch"));
    }

    #[test]
    fn set_from_arrays_rejects_non_increasing_timestamps() {
        let mut arr = OhlcDataArray::new();
        let err = arr
            .set_from_arrays(
                &[1, 2, 2],
                &[1.0, 1.0, 1.0],
                &[2.0, 2.0, 2.0],
                &[0.5, 0.5, 0.5],
                &[1.5, 1.5, 1.5],
            )
            .unwrap_err();
        assert!(err.contains("strictly increasing"));
    }
}
