//! Compact, GPU-friendly data structures for OHLCV bars.
//!
//! Design decisions:
//! - All structs are `#[repr(C)]` + bytemuck Pod/Zeroable so they can be
//!   uploaded to GPU buffers with zero conversion.
//! - Timestamps are u64 (epoch millis) — avoids f64 precision issues for
//!   dates far into the future.
//! - Prices are f32 — sufficient for 7 significant digits; GPU-native.
//! - Volume is f32 to keep the struct 32-byte aligned (cache-line friendly).

use bytemuck::{Pod, Zeroable};
use arrow::array::{Float32Array, UInt64Array, Float32Builder, UInt64Builder};

/// A single OHLCV bar, 32 bytes, cache-line aligned.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Bar {
    pub timestamp: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub _pad: f32,
}

impl Bar {
    #[inline]
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    #[inline]
    pub fn body_top(&self) -> f32 {
        if self.is_bullish() { self.close } else { self.open }
    }

    #[inline]
    pub fn body_bottom(&self) -> f32 {
        if self.is_bullish() { self.open } else { self.close }
    }
}

/// A columnar array of bars stored using Apache Arrow.
pub struct BarArray {
    pub timestamps: UInt64Array,
    pub opens: Float32Array,
    pub highs: Float32Array,
    pub lows: Float32Array,
    pub closes: Float32Array,
    pub volumes: Float32Array,
    len: usize,
}

impl BarArray {
    pub fn new() -> Self {
        Self {
            timestamps: UInt64Array::from(Vec::<u64>::new()),
            opens: Float32Array::from(Vec::<f32>::new()),
            highs: Float32Array::from(Vec::<f32>::new()),
            lows: Float32Array::from(Vec::<f32>::new()),
            closes: Float32Array::from(Vec::<f32>::new()),
            volumes: Float32Array::from(Vec::<f32>::new()),
            len: 0,
        }
    }

    pub fn set(&mut self, bars: Vec<Bar>) {
        let len = bars.len();
        let mut ts = UInt64Builder::with_capacity(len);
        let mut o = Float32Builder::with_capacity(len);
        let mut h = Float32Builder::with_capacity(len);
        let mut l = Float32Builder::with_capacity(len);
        let mut c = Float32Builder::with_capacity(len);
        let mut v = Float32Builder::with_capacity(len);

        for bar in bars {
            // Sanitize: replace NaN/Infinity with 0, clamp volume ≥ 0,
            // ensure high ≥ max(open,close) and low ≤ min(open,close).
            let open  = if bar.open.is_finite()  { bar.open  } else { 0.0 };
            let close = if bar.close.is_finite() { bar.close } else { open };
            let high  = if bar.high.is_finite()  { bar.high.max(open).max(close) } else { open.max(close) };
            let low   = if bar.low.is_finite()   { bar.low.min(open).min(close)  } else { open.min(close) };
            let vol   = if bar.volume.is_finite() { bar.volume.max(0.0) } else { 0.0 };

            ts.append_value(bar.timestamp);
            o.append_value(open);
            h.append_value(high);
            l.append_value(low);
            c.append_value(close);
            v.append_value(vol);
        }

        self.timestamps = ts.finish();
        self.opens = o.finish();
        self.highs = h.finish();
        self.lows = l.finish();
        self.closes = c.finish();
        self.volumes = v.finish();
        self.len = len;
    }

    #[inline]
    pub fn get(&self, i: usize) -> Bar {
        Bar {
            timestamp: self.timestamps.value(i),
            open: self.opens.value(i),
            high: self.highs.value(i),
            low: self.lows.value(i),
            close: self.closes.value(i),
            volume: self.volumes.value(i),
            _pad: 0.0,
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

    /// Retrieve an LTTB downsampled version of this array.
    pub fn downsample_lttb(&self, threshold: usize) -> BarArray {
        let len = self.len;
        if threshold >= len || threshold < 3 {
            // Just copy if threshold is larger or data is too small
            let mut arr = BarArray::new();
            let mut bars = Vec::with_capacity(len);
            for i in 0..len { bars.push(self.get(i)); }
            arr.set(bars);
            return arr;
        }

        let mut sampled = Vec::with_capacity(threshold);
        sampled.push(self.get(0));

        let bucket_size = (len - 2) as f64 / (threshold - 2) as f64;
        let mut a = 0; // Currently selected point index

        for i in 0..(threshold - 2) {
            let bucket_start = (i as f64 * bucket_size).floor() as usize + 1;
            let bucket_end = ((i + 1) as f64 * bucket_size).floor() as usize + 1;
            
            let next_bucket_start = bucket_end;
            let next_bucket_end = (((i + 2) as f64 * bucket_size).floor() as usize + 1).min(len);

            let mut avg_x = 0.0;
            let mut avg_y = 0.0;
            let next_len = next_bucket_end - next_bucket_start;
            for j in next_bucket_start..next_bucket_end {
                avg_x += j as f64;
                avg_y += self.closes.value(j) as f64;
            }
            avg_x /= next_len as f64;
            avg_y /= next_len as f64;

            let point_a_x = a as f64;
            let point_a_y = self.closes.value(a) as f64;

            let mut max_area = -1.0;
            let mut max_area_idx = bucket_start;

            for j in bucket_start..bucket_end {
                let point_x = j as f64;
                let point_y = self.closes.value(j) as f64;
                
                let area = ((point_a_x - avg_x) * (point_y - point_a_y) -
                           (point_a_x - point_x) * (avg_y - point_a_y)).abs() * 0.5;
                           
                if area > max_area {
                    max_area = area;
                    max_area_idx = j;
                }
            }

            sampled.push(self.get(max_area_idx));
            a = max_area_idx;
        }

        sampled.push(self.get(len - 1));

        let mut arr = BarArray::new();
        arr.set(sampled);
        arr
    }
}
