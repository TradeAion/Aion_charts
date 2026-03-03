//! Compact data structures for OHLCV bars.
//!
//! This module provides the core data types for storing financial bar data
//! (Open, High, Low, Close, Volume) in a format optimized for efficient
//! real-time updates.
//!
//! # Design Principles
//!
//! - **Cache-Friendly**: 32-byte aligned structures fit cache lines
//! - **Precision**: Timestamps are u64 (epoch millis), prices are f32 (7 significant digits)
//! - **Streaming-Optimized**: O(1) append operations via pending buffer pattern
//!
//! # Performance Characteristics
//!
//! | Operation | Time Complexity | Notes |
//! |-----------|-----------------|-------|
//! | `set()` | O(n) | Full array rebuild |
//! | `append()` | O(1) amortized | Pending buffer, auto-flush at 64 bars |
//! | `get()` | O(1) | Bounds-checked |
//! | `get_unchecked()` | O(1) | No bounds check (unsafe) |
//! | `update_last()` | O(1) | In-place if in pending buffer |
//! | `flush()` | O(n) | Rebuild Arrow arrays from pending |
//!
//! # Example
//!
//! ```rust
//! use raycore::{Bar, BarArray};
//!
//! let mut bars = BarArray::new();
//!
//! // Bulk load historical data
//! bars.set(vec![
//!     Bar { timestamp: 1700000000000, open: 100.0, high: 105.0, low: 98.0, close: 103.0, volume: 1000.0, _pad: 0.0 },
//!     Bar { timestamp: 1700000060000, open: 103.0, high: 108.0, low: 101.0, close: 106.0, volume: 1200.0, _pad: 0.0 },
//! ]);
//!
//! // Stream real-time updates (O(1) each)
//! bars.append(Bar { timestamp: 1700000120000, open: 106.0, high: 110.0, low: 104.0, close: 109.0, volume: 800.0, _pad: 0.0 });
//!
//! // Access data
//! if let Some(bar) = bars.get(0) {
//!     println!("First bar close: {}", bar.close);
//! }
//! ```

use arrow::array::{Float32Array, Float32Builder, UInt64Array, UInt64Builder};

/// Default chunk size for pending buffer before auto-flush (64 bars = 2KB)
const DEFAULT_CHUNK_SIZE: usize = 64;

/// A single OHLCV bar, 32 bytes, cache-line aligned.
///
/// This struct is designed for cache-friendly access:
/// - Uses `#[repr(C)]` for predictable memory layout
/// - 32-byte size aligns with common CPU cache lines
///
/// # Fields
///
/// - `timestamp`: Unix epoch milliseconds (u64 for precision past year 2038)
/// - `open`, `high`, `low`, `close`: Price values as f32 (7 significant digits)
/// - `volume`: Trading volume as f32
/// - `_pad`: Padding to maintain 32-byte alignment (must be set to 0.0)
///
/// # Example
///
/// ```rust
/// use raycore::Bar;
///
/// let bar = Bar {
///     timestamp: 1700000000000, // Nov 14, 2023
///     open: 100.0,
///     high: 105.0,
///     low: 98.0,
///     close: 103.0,
///     volume: 1000.0,
///     _pad: 0.0,
/// };
///
/// assert!(bar.is_bullish()); // close > open
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Bar {
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
    /// Opening price.
    pub open: f32,
    /// Highest price during the period.
    pub high: f32,
    /// Lowest price during the period.
    pub low: f32,
    /// Closing price.
    pub close: f32,
    /// Trading volume.
    pub volume: f32,
    /// Padding for 32-byte alignment (must be 0.0).
    pub _pad: f32,
}

impl Bar {
    #[inline]
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    #[inline]
    pub fn body_top(&self) -> f32 {
        if self.is_bullish() {
            self.close
        } else {
            self.open
        }
    }

    #[inline]
    pub fn body_bottom(&self) -> f32 {
        if self.is_bullish() {
            self.open
        } else {
            self.close
        }
    }
}

/// A columnar array of bars stored using Apache Arrow.
///
/// Uses a two-tier storage strategy for efficient appends:
/// - `pending`: `Vec<Bar>` buffer for O(1) appends
/// - Arrow arrays: immutable, rebuilt only when flushed
///
/// The pending buffer auto-flushes when it reaches `chunk_size` bars,
/// or can be manually flushed with `flush()`.
pub struct BarArray {
    // Immutable Arrow arrays (for reads)
    pub timestamps: UInt64Array,
    pub opens: Float32Array,
    pub highs: Float32Array,
    pub lows: Float32Array,
    pub closes: Float32Array,
    pub volumes: Float32Array,

    // Pending buffer for O(1) appends
    pending: Vec<Bar>,

    // Total length (Arrow arrays + pending)
    len: usize,

    // Chunk size for auto-flush
    chunk_size: usize,

    // Flag indicating pending changes need flush before reads
    dirty: bool,
}

impl BarArray {
    pub fn new() -> Self {
        Self::with_chunk_size(DEFAULT_CHUNK_SIZE)
    }

    /// Create a new BarArray with a custom chunk size for pending buffer.
    /// Smaller chunk sizes mean more frequent rebuilds but lower memory overhead.
    /// Larger chunk sizes mean fewer rebuilds but reads from pending are slower.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            timestamps: UInt64Array::from(Vec::<u64>::new()),
            opens: Float32Array::from(Vec::<f32>::new()),
            highs: Float32Array::from(Vec::<f32>::new()),
            lows: Float32Array::from(Vec::<f32>::new()),
            closes: Float32Array::from(Vec::<f32>::new()),
            volumes: Float32Array::from(Vec::<f32>::new()),
            pending: Vec::with_capacity(chunk_size),
            len: 0,
            chunk_size: chunk_size.max(1), // At least 1
            dirty: false,
        }
    }

    pub fn set(&mut self, bars: Vec<Bar>) {
        // Clear pending buffer since we're replacing all data
        self.pending.clear();
        self.dirty = false;

        let len = bars.len();
        let mut ts = UInt64Builder::with_capacity(len);
        let mut o = Float32Builder::with_capacity(len);
        let mut h = Float32Builder::with_capacity(len);
        let mut l = Float32Builder::with_capacity(len);
        let mut c = Float32Builder::with_capacity(len);
        let mut v = Float32Builder::with_capacity(len);

        for bar in bars {
            let sanitized = Self::sanitize_bar(&bar);
            ts.append_value(sanitized.timestamp);
            o.append_value(sanitized.open);
            h.append_value(sanitized.high);
            l.append_value(sanitized.low);
            c.append_value(sanitized.close);
            v.append_value(sanitized.volume);
        }

        self.timestamps = ts.finish();
        self.opens = o.finish();
        self.highs = h.finish();
        self.lows = l.finish();
        self.closes = c.finish();
        self.volumes = v.finish();
        self.len = len;
    }

    /// Sanitize a bar: replace NaN/Infinity with sensible defaults,
    /// clamp volume ≥ 0, ensure high ≥ max(open,close) and low ≤ min(open,close).
    #[inline]
    fn sanitize_bar(bar: &Bar) -> Bar {
        let open = if bar.open.is_finite() { bar.open } else { 0.0 };
        let close = if bar.close.is_finite() {
            bar.close
        } else {
            open
        };
        let high = if bar.high.is_finite() {
            bar.high.max(open).max(close)
        } else {
            open.max(close)
        };
        let low = if bar.low.is_finite() {
            bar.low.min(open).min(close)
        } else {
            open.min(close)
        };
        let volume = if bar.volume.is_finite() {
            bar.volume.max(0.0)
        } else {
            0.0
        };

        Bar {
            timestamp: bar.timestamp,
            open,
            high,
            low,
            close,
            volume,
            _pad: 0.0,
        }
    }

    /// Get a bar at the given index, returns None if out of bounds.
    /// This method checks both the Arrow arrays and the pending buffer.
    #[inline]
    pub fn get(&self, i: usize) -> Option<Bar> {
        if i >= self.len {
            return None;
        }

        let arrow_len = self.timestamps.len();

        if i < arrow_len {
            // Read from Arrow arrays
            Some(Bar {
                timestamp: self.timestamps.value(i),
                open: self.opens.value(i),
                high: self.highs.value(i),
                low: self.lows.value(i),
                close: self.closes.value(i),
                volume: self.volumes.value(i),
                _pad: 0.0,
            })
        } else {
            // Read from pending buffer
            let pending_idx = i - arrow_len;
            self.pending.get(pending_idx).copied()
        }
    }

    /// Get a bar at the given index without bounds checking.
    ///
    /// # Safety
    /// Caller must ensure `i < self.len()`. Using an out-of-bounds index
    /// will panic (if reading from Arrow) or return garbage/panic (if reading from pending).
    #[inline]
    pub fn get_unchecked(&self, i: usize) -> Bar {
        let arrow_len = self.timestamps.len();

        if i < arrow_len {
            Bar {
                timestamp: self.timestamps.value(i),
                open: self.opens.value(i),
                high: self.highs.value(i),
                low: self.lows.value(i),
                close: self.closes.value(i),
                volume: self.volumes.value(i),
                _pad: 0.0,
            }
        } else {
            let pending_idx = i - arrow_len;
            self.pending[pending_idx]
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

    /// Append a single bar to the end of the array.
    /// This is O(1) - the bar is added to a pending buffer.
    /// The pending buffer is automatically flushed when it reaches chunk_size.
    pub fn append(&mut self, bar: Bar) {
        let sanitized = Self::sanitize_bar(&bar);
        self.pending.push(sanitized);
        self.len += 1;
        self.dirty = true;

        // Auto-flush if pending buffer is full
        if self.pending.len() >= self.chunk_size {
            self.flush();
        }
    }

    /// Update the last bar in the array. No-op if the array is empty.
    /// This is O(1) - updates either the pending buffer or marks for rebuild.
    pub fn update_last(&mut self, bar: Bar) {
        if self.len == 0 {
            return;
        }

        let sanitized = Self::sanitize_bar(&bar);

        if !self.pending.is_empty() {
            // Last bar is in pending buffer - O(1) update
            let last_idx = self.pending.len() - 1;
            self.pending[last_idx] = sanitized;
        } else {
            // Last bar is in Arrow arrays - need to rebuild just that bar
            // We can optimize this by putting the update in pending and marking dirty
            // Then flush will handle the merge
            self.rebuild_with_last_updated(&sanitized);
        }

        self.dirty = true;
    }

    /// Rebuild Arrow arrays with the last element updated.
    /// This is only called when pending is empty and we need to update the last Arrow element.
    fn rebuild_with_last_updated(&mut self, bar: &Bar) {
        if self.len == 0 {
            return;
        }

        let mut ts = UInt64Builder::with_capacity(self.len);
        let mut o = Float32Builder::with_capacity(self.len);
        let mut h = Float32Builder::with_capacity(self.len);
        let mut l = Float32Builder::with_capacity(self.len);
        let mut c = Float32Builder::with_capacity(self.len);
        let mut v = Float32Builder::with_capacity(self.len);

        // Copy all but last
        for i in 0..self.len - 1 {
            ts.append_value(self.timestamps.value(i));
            o.append_value(self.opens.value(i));
            h.append_value(self.highs.value(i));
            l.append_value(self.lows.value(i));
            c.append_value(self.closes.value(i));
            v.append_value(self.volumes.value(i));
        }

        // Write updated last bar
        ts.append_value(bar.timestamp);
        o.append_value(bar.open);
        h.append_value(bar.high);
        l.append_value(bar.low);
        c.append_value(bar.close);
        v.append_value(bar.volume);

        self.timestamps = ts.finish();
        self.opens = o.finish();
        self.highs = h.finish();
        self.lows = l.finish();
        self.closes = c.finish();
        self.volumes = v.finish();
    }

    /// Flush pending bars into the Arrow arrays.
    /// Call this when you need contiguous Arrow data.
    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            self.dirty = false;
            return;
        }

        let new_len = self.timestamps.len() + self.pending.len();
        let mut ts = UInt64Builder::with_capacity(new_len);
        let mut o = Float32Builder::with_capacity(new_len);
        let mut h = Float32Builder::with_capacity(new_len);
        let mut l = Float32Builder::with_capacity(new_len);
        let mut c = Float32Builder::with_capacity(new_len);
        let mut v = Float32Builder::with_capacity(new_len);

        // Copy existing Arrow data
        for i in 0..self.timestamps.len() {
            ts.append_value(self.timestamps.value(i));
            o.append_value(self.opens.value(i));
            h.append_value(self.highs.value(i));
            l.append_value(self.lows.value(i));
            c.append_value(self.closes.value(i));
            v.append_value(self.volumes.value(i));
        }

        // Append pending bars
        for bar in &self.pending {
            ts.append_value(bar.timestamp);
            o.append_value(bar.open);
            h.append_value(bar.high);
            l.append_value(bar.low);
            c.append_value(bar.close);
            v.append_value(bar.volume);
        }

        self.timestamps = ts.finish();
        self.opens = o.finish();
        self.highs = h.finish();
        self.lows = l.finish();
        self.closes = c.finish();
        self.volumes = v.finish();
        self.pending.clear();
        self.dirty = false;
    }

    /// Returns true if there are pending changes that haven't been flushed.
    /// Check this to ensure data is coherent before reads.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Ensure all data is in Arrow arrays (flush if needed).
    /// Call this before accessing the raw Arrow arrays.
    #[inline]
    pub fn ensure_flushed(&mut self) {
        if self.dirty {
            self.flush();
        }
    }

    /// Access timestamp at index.
    ///
    /// # Panics
    /// Panics if `i >= self.len()`. Use `get()` for safe access with bounds checking.
    #[inline]
    pub fn timestamp(&self, i: usize) -> u64 {
        let arrow_len = self.timestamps.len();
        if i < arrow_len {
            self.timestamps.value(i)
        } else {
            self.pending[i - arrow_len].timestamp
        }
    }

    /// Access open price at index.
    ///
    /// # Panics
    /// Panics if `i >= self.len()`. Use `get()` for safe access with bounds checking.
    #[inline]
    pub fn open(&self, i: usize) -> f32 {
        let arrow_len = self.opens.len();
        if i < arrow_len {
            self.opens.value(i)
        } else {
            self.pending[i - arrow_len].open
        }
    }

    /// Access high price at index.
    ///
    /// # Panics
    /// Panics if `i >= self.len()`. Use `get()` for safe access with bounds checking.
    #[inline]
    pub fn high(&self, i: usize) -> f32 {
        let arrow_len = self.highs.len();
        if i < arrow_len {
            self.highs.value(i)
        } else {
            self.pending[i - arrow_len].high
        }
    }

    /// Access low price at index.
    ///
    /// # Panics
    /// Panics if `i >= self.len()`. Use `get()` for safe access with bounds checking.
    #[inline]
    pub fn low(&self, i: usize) -> f32 {
        let arrow_len = self.lows.len();
        if i < arrow_len {
            self.lows.value(i)
        } else {
            self.pending[i - arrow_len].low
        }
    }

    /// Access close price at index.
    ///
    /// # Panics
    /// Panics if `i >= self.len()`. Use `get()` for safe access with bounds checking.
    #[inline]
    pub fn close(&self, i: usize) -> f32 {
        let arrow_len = self.closes.len();
        if i < arrow_len {
            self.closes.value(i)
        } else {
            self.pending[i - arrow_len].close
        }
    }

    /// Access volume at index.
    ///
    /// # Panics
    /// Panics if `i >= self.len()`. Use `get()` for safe access with bounds checking.
    #[inline]
    pub fn volume(&self, i: usize) -> f32 {
        let arrow_len = self.volumes.len();
        if i < arrow_len {
            self.volumes.value(i)
        } else {
            self.pending[i - arrow_len].volume
        }
    }

    /// Retrieve an LTTB downsampled version of this array.
    /// Returns a new BarArray with at most `threshold` bars.
    pub fn downsample_lttb(&self, threshold: usize) -> BarArray {
        let len = self.len;
        if threshold >= len || threshold < 3 {
            // Just copy if threshold is larger or data is too small
            let mut arr = BarArray::new();
            let mut bars = Vec::with_capacity(len);
            for i in 0..len {
                if let Some(bar) = self.get(i) {
                    bars.push(bar);
                }
            }
            arr.set(bars);
            return arr;
        }

        let mut sampled = Vec::with_capacity(threshold);

        // First point is always included
        if let Some(first) = self.get(0) {
            sampled.push(first);
        }

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
                avg_y += self.close(j) as f64;
            }
            avg_x /= next_len as f64;
            avg_y /= next_len as f64;

            let point_a_x = a as f64;
            let point_a_y = self.close(a) as f64;

            let mut max_area = -1.0;
            let mut max_area_idx = bucket_start;

            for j in bucket_start..bucket_end {
                let point_x = j as f64;
                let point_y = self.close(j) as f64;

                let area = ((point_a_x - avg_x) * (point_y - point_a_y)
                    - (point_a_x - point_x) * (avg_y - point_a_y))
                    .abs()
                    * 0.5;

                if area > max_area {
                    max_area = area;
                    max_area_idx = j;
                }
            }

            if let Some(bar) = self.get(max_area_idx) {
                sampled.push(bar);
            }
            a = max_area_idx;
        }

        // Last point is always included
        if let Some(last) = self.get(len - 1) {
            sampled.push(last);
        }

        let mut arr = BarArray::new();
        arr.set(sampled);
        arr
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(ts: u64, close: f32) -> Bar {
        Bar {
            timestamp: ts,
            open: close - 1.0,
            high: close + 1.0,
            low: close - 2.0,
            close,
            volume: 100.0,
            _pad: 0.0,
        }
    }

    // ── Bar struct tests ──

    #[test]
    fn test_bar_is_bullish() {
        let bullish = Bar {
            timestamp: 0,
            open: 100.0,
            high: 110.0,
            low: 95.0,
            close: 105.0,
            volume: 1000.0,
            _pad: 0.0,
        };
        assert!(bullish.is_bullish());

        let bearish = Bar {
            timestamp: 0,
            open: 105.0,
            high: 110.0,
            low: 95.0,
            close: 100.0,
            volume: 1000.0,
            _pad: 0.0,
        };
        assert!(!bearish.is_bullish());

        let doji = Bar {
            timestamp: 0,
            open: 100.0,
            high: 110.0,
            low: 95.0,
            close: 100.0,
            volume: 1000.0,
            _pad: 0.0,
        };
        assert!(doji.is_bullish()); // Equal counts as bullish
    }

    #[test]
    fn test_bar_body_metrics() {
        let bar = Bar {
            timestamp: 0,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
            volume: 1000.0,
            _pad: 0.0,
        };
        assert_eq!(bar.body_top(), 105.0);
        assert_eq!(bar.body_bottom(), 100.0);
    }

    // ── BarArray basic operations ──

    #[test]
    fn test_bar_array_new_is_empty() {
        let arr = BarArray::new();
        assert!(arr.is_empty());
        assert_eq!(arr.len(), 0);
    }

    #[test]
    fn test_bar_array_set() {
        let mut arr = BarArray::new();
        let bars = vec![
            make_bar(1000, 50.0),
            make_bar(2000, 51.0),
            make_bar(3000, 52.0),
        ];
        arr.set(bars);

        assert_eq!(arr.len(), 3);
        assert!(!arr.is_empty());
    }

    #[test]
    fn test_bar_array_get_bounds_checking() {
        let mut arr = BarArray::new();
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 51.0)]);

        // Valid indices
        assert!(arr.get(0).is_some());
        assert!(arr.get(1).is_some());

        // Invalid indices
        assert!(arr.get(2).is_none());
        assert!(arr.get(100).is_none());
    }

    #[test]
    fn test_bar_array_get_values() {
        let mut arr = BarArray::new();
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 60.0)]);

        let bar0 = arr.get(0).unwrap();
        assert_eq!(bar0.timestamp, 1000);
        assert_eq!(bar0.close, 50.0);

        let bar1 = arr.get(1).unwrap();
        assert_eq!(bar1.timestamp, 2000);
        assert_eq!(bar1.close, 60.0);
    }

    // ── O(1) append with pending buffer ──

    #[test]
    fn test_append_increments_length() {
        let mut arr = BarArray::new();
        assert_eq!(arr.len(), 0);

        arr.append(make_bar(1000, 50.0));
        assert_eq!(arr.len(), 1);

        arr.append(make_bar(2000, 51.0));
        assert_eq!(arr.len(), 2);

        arr.append(make_bar(3000, 52.0));
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_append_uses_pending_buffer() {
        let mut arr = BarArray::new();

        // Append without flush - should be in pending buffer
        arr.append(make_bar(1000, 50.0));

        // Should still be readable via get()
        let bar = arr.get(0).unwrap();
        assert_eq!(bar.close, 50.0);
    }

    #[test]
    fn test_append_auto_flush_at_chunk_size() {
        let mut arr = BarArray::with_chunk_size(4); // Small chunk for testing

        // Append exactly chunk_size bars
        for i in 0..4 {
            arr.append(make_bar(i as u64 * 1000, 50.0 + i as f32));
        }

        // Should have auto-flushed, all bars accessible
        assert_eq!(arr.len(), 4);
        for i in 0..4 {
            assert!(arr.get(i).is_some());
        }
    }

    #[test]
    fn test_mixed_arrow_and_pending_reads() {
        let mut arr = BarArray::with_chunk_size(2);

        // Add bars that will flush (2 bars)
        arr.append(make_bar(1000, 50.0));
        arr.append(make_bar(2000, 51.0)); // Triggers flush

        // Add more bars (in pending)
        arr.append(make_bar(3000, 52.0));
        arr.append(make_bar(4000, 53.0)); // Triggers flush again

        // All 4 bars should be accessible
        assert_eq!(arr.len(), 4);
        assert_eq!(arr.get(0).unwrap().close, 50.0);
        assert_eq!(arr.get(1).unwrap().close, 51.0);
        assert_eq!(arr.get(2).unwrap().close, 52.0);
        assert_eq!(arr.get(3).unwrap().close, 53.0);
    }

    // ── update_last tests ──

    #[test]
    fn test_update_last_in_pending() {
        let mut arr = BarArray::new();
        arr.append(make_bar(1000, 50.0));
        arr.append(make_bar(2000, 51.0));

        // Update last bar (in pending buffer)
        arr.update_last(make_bar(2000, 99.0));

        let bar = arr.get(1).unwrap();
        assert_eq!(bar.close, 99.0);
    }

    #[test]
    fn test_update_last_in_arrow() {
        let mut arr = BarArray::new();
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 51.0)]);

        // Update last bar (in Arrow arrays, pending is empty)
        arr.update_last(make_bar(2000, 99.0));

        let bar = arr.get(1).unwrap();
        assert_eq!(bar.close, 99.0);
    }

    #[test]
    fn test_update_last_on_empty_is_noop() {
        let mut arr = BarArray::new();
        arr.update_last(make_bar(1000, 50.0)); // Should not panic
        assert_eq!(arr.len(), 0);
    }

    // ── NaN/Infinity sanitization ──

    #[test]
    fn test_sanitize_nan_values() {
        let mut arr = BarArray::new();
        arr.append(Bar {
            timestamp: 1000,
            open: f32::NAN,
            high: f32::NAN,
            low: f32::NAN,
            close: f32::NAN,
            volume: f32::NAN,
            _pad: 0.0,
        });

        let bar = arr.get(0).unwrap();
        assert_eq!(bar.open, 0.0);
        assert_eq!(bar.high, 0.0);
        assert_eq!(bar.low, 0.0);
        assert_eq!(bar.close, 0.0);
        assert_eq!(bar.volume, 0.0);
    }

    #[test]
    fn test_sanitize_infinity_values() {
        let mut arr = BarArray::new();
        arr.append(Bar {
            timestamp: 1000,
            open: f32::INFINITY,
            high: f32::NEG_INFINITY,
            low: f32::INFINITY,
            close: f32::NEG_INFINITY,
            volume: f32::INFINITY,
            _pad: 0.0,
        });

        let bar = arr.get(0).unwrap();
        assert_eq!(bar.open, 0.0);
        assert_eq!(bar.high, 0.0);
        assert_eq!(bar.low, 0.0);
        assert_eq!(bar.close, 0.0);
        assert_eq!(bar.volume, 0.0);
    }

    // ── Direct accessor tests ──

    #[test]
    fn test_direct_accessors() {
        let mut arr = BarArray::new();
        arr.set(vec![
            Bar {
                timestamp: 1000,
                open: 10.0,
                high: 15.0,
                low: 8.0,
                close: 12.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 2000,
                open: 12.0,
                high: 18.0,
                low: 11.0,
                close: 16.0,
                volume: 200.0,
                _pad: 0.0,
            },
        ]);

        assert_eq!(arr.timestamp(0), 1000);
        assert_eq!(arr.timestamp(1), 2000);

        assert_eq!(arr.open(0), 10.0);
        assert_eq!(arr.high(0), 15.0);
        assert_eq!(arr.low(0), 8.0);
        assert_eq!(arr.close(0), 12.0);
        assert_eq!(arr.volume(0), 100.0);

        assert_eq!(arr.close(1), 16.0);
        assert_eq!(arr.volume(1), 200.0);
    }

    // ── get_unchecked for hot paths ──

    #[test]
    fn test_get_unchecked() {
        let mut arr = BarArray::new();
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 60.0)]);

        let bar0 = arr.get_unchecked(0);
        assert_eq!(bar0.close, 50.0);

        let bar1 = arr.get_unchecked(1);
        assert_eq!(bar1.close, 60.0);
    }
}
