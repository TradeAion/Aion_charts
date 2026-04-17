//! OHLC bar data storage — columnar arrays for timestamp, open, high, low, close.
//!
//! Used by the Bar (OHLC) series type. Similar to the main BarArray but
//! managed per-series (not the global candlestick data).

use crate::core::series::validation::{
    ensure_equal_len, ensure_finite_ohlc_point, ensure_strictly_increasing_timestamps,
};

/// A single OHLC data point.
#[derive(Debug, Clone, Copy)]
pub struct OhlcPoint {
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// Columnar storage for OHLC bar data.
#[derive(Debug, Clone, Default)]
pub struct OhlcDataArray {
    pub timestamps: Vec<u64>,
    pub open: Vec<f64>,
    pub high: Vec<f64>,
    pub low: Vec<f64>,
    pub close: Vec<f64>,
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
    pub fn set_data(&mut self, data: Vec<OhlcPoint>) -> Result<(), String> {
        for i in 1..data.len() {
            if data[i].timestamp <= data[i - 1].timestamp {
                return Err(format!(
                    "bar timestamps must be strictly increasing at index {}: {} <= {}",
                    i,
                    data[i].timestamp,
                    data[i - 1].timestamp
                ));
            }
        }
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
        for (index, p) in data.into_iter().enumerate() {
            let p = Self::validate_point("set_data", p, index)?;
            self.timestamps.push(p.timestamp);
            self.open.push(p.open);
            self.high.push(p.high);
            self.low.push(p.low);
            self.close.push(p.close);
        }
        Ok(())
    }

    /// Set data from parallel arrays.
    pub fn set_from_arrays(
        &mut self,
        timestamps: &[u64],
        open: &[f64],
        high: &[f64],
        low: &[f64],
        close: &[f64],
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
            let p = Self::validate_point(
                "set_from_arrays",
                OhlcPoint {
                    timestamp: timestamps[i],
                    open: open[i],
                    high: high[i],
                    low: low[i],
                    close: close[i],
                },
                i,
            )?;
            self.timestamps.push(p.timestamp);
            self.open.push(p.open);
            self.high.push(p.high);
            self.low.push(p.low);
            self.close.push(p.close);
        }
        Ok(())
    }

    /// Append a single OHLC point.
    pub fn push(&mut self, point: OhlcPoint) -> Result<(), String> {
        let p = Self::validate_point("push", point, self.timestamps.len())?;
        if let Some(last_ts) = self.last_timestamp() {
            if p.timestamp <= last_ts {
                return Err(format!(
                    "push requires timestamp > last timestamp ({} <= {})",
                    p.timestamp, last_ts
                ));
            }
        }
        self.timestamps.push(p.timestamp);
        self.open.push(p.open);
        self.high.push(p.high);
        self.low.push(p.low);
        self.close.push(p.close);
        Ok(())
    }

    /// Update the last point in-place. Returns false if the series is empty.
    pub fn update_last(&mut self, point: OhlcPoint) -> Result<bool, String> {
        if self.timestamps.is_empty() {
            return Ok(false);
        }
        let p = Self::validate_point("update_last", point, self.timestamps.len() - 1)?;
        let last_ts = self.timestamps[self.timestamps.len() - 1];
        if p.timestamp != last_ts {
            return Err(format!(
                "update_last requires timestamp == last timestamp ({} != {})",
                p.timestamp, last_ts
            ));
        }
        let idx = self.timestamps.len() - 1;
        self.timestamps[idx] = p.timestamp;
        self.open[idx] = p.open;
        self.high[idx] = p.high;
        self.low[idx] = p.low;
        self.close[idx] = p.close;
        Ok(true)
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
    fn validate_point(context: &str, point: OhlcPoint, index: usize) -> Result<OhlcPoint, String> {
        ensure_finite_ohlc_point(&format!("bar_data::{context}"), &point, index)?;

        Ok(OhlcPoint {
            timestamp: point.timestamp,
            open: point.open,
            high: point.high.max(point.open).max(point.close),
            low: point.low.min(point.open).min(point.close),
            close: point.close,
        })
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
        })
        .unwrap();

        let p = arr.get(0);
        assert_eq!(p.high, 105.0);
        assert_eq!(p.low, 100.0);
    }

    #[test]
    fn validate_point_rejects_non_finite() {
        let mut arr = OhlcDataArray::new();
        let err = arr
            .push(OhlcPoint {
                timestamp: 1,
                open: f64::NAN,
                high: f64::NEG_INFINITY,
                low: f64::INFINITY,
                close: f64::NAN,
            })
            .unwrap_err();
        assert!(err.contains("must be finite"));
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

    #[test]
    fn update_last_rejects_timestamp_mismatch() {
        let mut arr = OhlcDataArray::new();
        arr.push(OhlcPoint {
            timestamp: 1,
            open: 10.0,
            high: 11.0,
            low: 9.0,
            close: 10.5,
        })
        .unwrap();

        let err = arr
            .update_last(OhlcPoint {
                timestamp: 2,
                open: 10.0,
                high: 11.0,
                low: 9.0,
                close: 10.5,
            })
            .unwrap_err();
        assert!(err.contains("timestamp == last timestamp"));
    }
}
