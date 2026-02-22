//! OHLC bar data storage — columnar arrays for timestamp, open, high, low, close.
//!
//! Used by the Bar (OHLC) series type. Similar to the main BarArray but
//! managed per-series (not the global candlestick data).

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
    ) {
        let count = timestamps.len()
            .min(open.len())
            .min(high.len())
            .min(low.len())
            .min(close.len());
        self.timestamps = timestamps[..count].to_vec();
        self.open = open[..count].to_vec();
        self.high = high[..count].to_vec();
        self.low = low[..count].to_vec();
        self.close = close[..count].to_vec();
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
}
