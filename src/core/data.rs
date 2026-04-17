//! Compact data structures for OHLCV bars.
//!
//! This module provides the core data types for storing financial bar data
//! (Open, High, Low, Close, Volume) in a format optimized for efficient
//! real-time updates.
//!
//! # Design Principles
//!
//! - **Predictable Layout**: `Bar` uses `#[repr(C)]` and stores 48 bytes per row
//! - **Precision**: Timestamps are u64 (epoch millis), logical prices use f64 with roughly 16 digits of precision
//! - **Render Seam**: logical values are projected to single-precision render space only at the renderer seam
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
//! use axiuscharts::{Bar, BarArray};
//!
//! let mut bars = BarArray::new();
//!
//! // Bulk load historical data
//! bars.set(vec![
//!     Bar::new(1700000000000, 100.0, 105.0, 98.0, 103.0, 1000.0),
//!     Bar::new(1700000060000, 103.0, 108.0, 101.0, 106.0, 1200.0),
//! ]).unwrap();
//!
//! // Stream real-time updates (O(1) each)
//! bars.append(Bar::new(1700000120000, 106.0, 110.0, 104.0, 109.0, 800.0)).unwrap();
//!
//! // Access data
//! if let Some(bar) = bars.get(0) {
//!     println!("First bar close: {}", bar.close);
//! }
//! ```

use arrow::array::{Float64Array, Float64Builder, UInt64Array, UInt64Builder};

/// Default chunk size for pending buffer before auto-flush.
const DEFAULT_CHUNK_SIZE: usize = 64;

/// A single OHLCV bar in the logical price domain.
///
/// This struct is designed for predictable interop and storage:
/// - Uses `#[repr(C)]` for stable field ordering
/// - Occupies 48 bytes with the current `u64 + 5 * f64` layout
/// - Stores logical prices as `f64`; renderers project to single-precision screen space at the final render seam
///
/// # Fields
///
/// - `timestamp`: Unix epoch milliseconds (u64 for precision past year 2038)
/// - `open`, `high`, `low`, `close`: Price values as `f64`
/// - `volume`: Trading volume as `f64`
///
/// # Example
///
/// ```rust
/// use axiuscharts::Bar;
///
/// let bar = Bar::new(1700000000000, 100.0, 105.0, 98.0, 103.0, 1000.0);
///
/// assert!(bar.is_bullish()); // close > open
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Bar {
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
    /// Opening price.
    pub open: f64,
    /// Highest price during the period.
    pub high: f64,
    /// Lowest price during the period.
    pub low: f64,
    /// Closing price.
    pub close: f64,
    /// Trading volume.
    pub volume: f64,
}

impl Bar {
    #[inline]
    pub fn new(timestamp: u64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        }
    }

    #[inline]
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    #[inline]
    pub fn body_top(&self) -> f64 {
        if self.is_bullish() {
            self.close
        } else {
            self.open
        }
    }

    #[inline]
    pub fn body_bottom(&self) -> f64 {
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
    pub opens: Float64Array,
    pub highs: Float64Array,
    pub lows: Float64Array,
    pub closes: Float64Array,
    pub volumes: Float64Array,

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
            opens: Float64Array::from(Vec::<f64>::new()),
            highs: Float64Array::from(Vec::<f64>::new()),
            lows: Float64Array::from(Vec::<f64>::new()),
            closes: Float64Array::from(Vec::<f64>::new()),
            volumes: Float64Array::from(Vec::<f64>::new()),
            pending: Vec::with_capacity(chunk_size),
            len: 0,
            chunk_size: chunk_size.max(1), // At least 1
            dirty: false,
        }
    }

    pub fn set(&mut self, bars: Vec<Bar>) -> Result<(), String> {
        // Clear pending buffer since we're replacing all data
        self.pending.clear();
        self.dirty = false;

        let len = bars.len();
        let mut ts = UInt64Builder::with_capacity(len);
        let mut o = Float64Builder::with_capacity(len);
        let mut h = Float64Builder::with_capacity(len);
        let mut l = Float64Builder::with_capacity(len);
        let mut c = Float64Builder::with_capacity(len);
        let mut v = Float64Builder::with_capacity(len);

        let mut last_timestamp: Option<u64> = None;
        for (index, bar) in bars.into_iter().enumerate() {
            if let Some(previous) = last_timestamp {
                if bar.timestamp <= previous {
                    return Err(format!(
                        "bar_data::set: timestamps must be strictly increasing at index {index}: {} <= {}",
                        bar.timestamp, previous
                    ));
                }
            }
            let normalized = Self::validate_bar("set", &bar, index)?;
            ts.append_value(normalized.timestamp);
            o.append_value(normalized.open);
            h.append_value(normalized.high);
            l.append_value(normalized.low);
            c.append_value(normalized.close);
            v.append_value(normalized.volume);
            last_timestamp = Some(normalized.timestamp);
        }

        self.timestamps = ts.finish();
        self.opens = o.finish();
        self.highs = h.finish();
        self.lows = l.finish();
        self.closes = c.finish();
        self.volumes = v.finish();
        self.len = len;
        Ok(())
    }

    /// Validate and normalize a bar:
    /// reject non-finite scalars, clamp volume ≥ 0, and ensure
    /// high ≥ max(open, close) plus low ≤ min(open, close).
    #[inline]
    fn validate_bar(context: &str, bar: &Bar, index: usize) -> Result<Bar, String> {
        for (field, value) in [
            ("open", bar.open),
            ("high", bar.high),
            ("low", bar.low),
            ("close", bar.close),
            ("volume", bar.volume),
        ] {
            if !value.is_finite() {
                return Err(format!(
                    "bar_data::{context}: {field} at index {index} must be finite, got {value}"
                ));
            }
        }

        Ok(Bar::new(
            bar.timestamp,
            bar.open,
            bar.high.max(bar.open).max(bar.close),
            bar.low.min(bar.open).min(bar.close),
            bar.close,
            bar.volume.max(0.0),
        ))
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
            Some(Bar::new(
                self.timestamps.value(i),
                self.opens.value(i),
                self.highs.value(i),
                self.lows.value(i),
                self.closes.value(i),
                self.volumes.value(i),
            ))
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
            Bar::new(
                self.timestamps.value(i),
                self.opens.value(i),
                self.highs.value(i),
                self.lows.value(i),
                self.closes.value(i),
                self.volumes.value(i),
            )
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
    pub fn append(&mut self, bar: Bar) -> Result<(), String> {
        if let Some(last_timestamp) = self.len.checked_sub(1).map(|index| self.timestamp(index)) {
            if bar.timestamp <= last_timestamp {
                return Err(format!(
                    "bar_data::append: timestamp must be > last timestamp ({} <= {})",
                    bar.timestamp, last_timestamp
                ));
            }
        }
        let normalized = Self::validate_bar("append", &bar, self.len)?;
        self.pending.push(normalized);
        self.len += 1;
        self.dirty = true;

        // Auto-flush if pending buffer is full
        if self.pending.len() >= self.chunk_size {
            self.flush();
        }
        Ok(())
    }

    /// Update the last bar in the array. No-op if the array is empty.
    /// This is O(1) - updates either the pending buffer or marks for rebuild.
    pub fn update_last(&mut self, bar: Bar) -> Result<bool, String> {
        if self.len == 0 {
            return Ok(false);
        }

        let last_timestamp = self.timestamp(self.len - 1);
        if bar.timestamp != last_timestamp {
            return Err(format!(
                "bar_data::update_last: timestamp must equal last timestamp ({} != {})",
                bar.timestamp, last_timestamp
            ));
        }
        let normalized = Self::validate_bar("update_last", &bar, self.len - 1)?;

        if !self.pending.is_empty() {
            // Last bar is in pending buffer - O(1) update
            let last_idx = self.pending.len() - 1;
            self.pending[last_idx] = normalized;
        } else {
            // Last bar is in Arrow arrays - need to rebuild just that bar
            // We can optimize this by putting the update in pending and marking dirty
            // Then flush will handle the merge
            self.rebuild_with_last_updated(&normalized);
        }

        self.dirty = true;
        Ok(true)
    }

    /// Rebuild Arrow arrays with the last element updated.
    /// This is only called when pending is empty and we need to update the last Arrow element.
    fn rebuild_with_last_updated(&mut self, bar: &Bar) {
        if self.len == 0 {
            return;
        }

        let mut ts = UInt64Builder::with_capacity(self.len);
        let mut o = Float64Builder::with_capacity(self.len);
        let mut h = Float64Builder::with_capacity(self.len);
        let mut l = Float64Builder::with_capacity(self.len);
        let mut c = Float64Builder::with_capacity(self.len);
        let mut v = Float64Builder::with_capacity(self.len);

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
        let mut o = Float64Builder::with_capacity(new_len);
        let mut h = Float64Builder::with_capacity(new_len);
        let mut l = Float64Builder::with_capacity(new_len);
        let mut c = Float64Builder::with_capacity(new_len);
        let mut v = Float64Builder::with_capacity(new_len);

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
    pub fn open(&self, i: usize) -> f64 {
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
    pub fn high(&self, i: usize) -> f64 {
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
    pub fn low(&self, i: usize) -> f64 {
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
    pub fn close(&self, i: usize) -> f64 {
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
    pub fn volume(&self, i: usize) -> f64 {
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
            arr.set(bars)
                .expect("downsample source bars should already be valid");
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
                avg_y += self.close(j);
            }
            avg_x /= next_len as f64;
            avg_y /= next_len as f64;

            let point_a_x = a as f64;
            let point_a_y = self.close(a);

            let mut max_area = -1.0;
            let mut max_area_idx = bucket_start;

            for j in bucket_start..bucket_end {
                let point_x = j as f64;
                let point_y = self.close(j);

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
        arr.set(sampled)
            .expect("downsampled bars should already be valid");
        arr
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(ts: u64, close: f64) -> Bar {
        Bar::new(ts, close - 1.0, close + 1.0, close - 2.0, close, 100.0)
    }

    // ── Bar struct tests ──

    #[test]
    fn test_bar_is_bullish() {
        let bullish = Bar::new(0, 100.0, 110.0, 95.0, 105.0, 1000.0);
        assert!(bullish.is_bullish());

        let bearish = Bar::new(0, 105.0, 110.0, 95.0, 100.0, 1000.0);
        assert!(!bearish.is_bullish());

        let doji = Bar::new(0, 100.0, 110.0, 95.0, 100.0, 1000.0);
        assert!(doji.is_bullish()); // Equal counts as bullish
    }

    #[test]
    fn test_bar_body_metrics() {
        let bar = Bar::new(0, 100.0, 110.0, 90.0, 105.0, 1000.0);
        assert_eq!(bar.body_top(), 105.0);
        assert_eq!(bar.body_bottom(), 100.0);
    }

    #[test]
    fn bar_preserves_crypto_precision() {
        let bar = Bar::new(
            1_700_000_000_000,
            103_842.57_f64,
            103_842.58_f64,
            103_842.56_f64,
            103_842.5712345_f64,
            1_000.0_f64,
        );
        assert_eq!(bar.close, 103_842.5712345_f64);
        let mut arr = BarArray::new();
        arr.set(vec![bar]).unwrap();
        assert_eq!(arr.get(0).unwrap().close, 103_842.5712345_f64);
    }

    #[test]
    fn bar_preserves_small_alt_precision() {
        let bar = Bar::new(
            1_700_000_000_000,
            0.0000001234_f64,
            0.0000001235_f64,
            0.0000001233_f64,
            0.00000012345678_f64,
            1.0_f64,
        );
        let mut arr = BarArray::new();
        arr.set(vec![bar]).unwrap();
        assert_eq!(arr.get(0).unwrap().close, 0.00000012345678_f64);
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
        arr.set(bars).unwrap();

        assert_eq!(arr.len(), 3);
        assert!(!arr.is_empty());
    }

    #[test]
    fn test_bar_array_get_bounds_checking() {
        let mut arr = BarArray::new();
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 51.0)])
            .unwrap();

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
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 60.0)])
            .unwrap();

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

        arr.append(make_bar(1000, 50.0)).unwrap();
        assert_eq!(arr.len(), 1);

        arr.append(make_bar(2000, 51.0)).unwrap();
        assert_eq!(arr.len(), 2);

        arr.append(make_bar(3000, 52.0)).unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_append_uses_pending_buffer() {
        let mut arr = BarArray::new();

        // Append without flush - should be in pending buffer
        arr.append(make_bar(1000, 50.0)).unwrap();

        // Should still be readable via get()
        let bar = arr.get(0).unwrap();
        assert_eq!(bar.close, 50.0);
    }

    #[test]
    fn test_append_auto_flush_at_chunk_size() {
        let mut arr = BarArray::with_chunk_size(4); // Small chunk for testing

        // Append exactly chunk_size bars
        for i in 0..4 {
            arr.append(make_bar(i as u64 * 1000, 50.0 + i as f64))
                .unwrap();
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
        arr.append(make_bar(1000, 50.0)).unwrap();
        arr.append(make_bar(2000, 51.0)).unwrap(); // Triggers flush

        // Add more bars (in pending)
        arr.append(make_bar(3000, 52.0)).unwrap();
        arr.append(make_bar(4000, 53.0)).unwrap(); // Triggers flush again

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
        arr.append(make_bar(1000, 50.0)).unwrap();
        arr.append(make_bar(2000, 51.0)).unwrap();

        // Update last bar (in pending buffer)
        arr.update_last(make_bar(2000, 99.0)).unwrap();

        let bar = arr.get(1).unwrap();
        assert_eq!(bar.close, 99.0);
    }

    #[test]
    fn test_update_last_in_arrow() {
        let mut arr = BarArray::new();
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 51.0)])
            .unwrap();

        // Update last bar (in Arrow arrays, pending is empty)
        arr.update_last(make_bar(2000, 99.0)).unwrap();

        let bar = arr.get(1).unwrap();
        assert_eq!(bar.close, 99.0);
    }

    #[test]
    fn test_update_last_on_empty_returns_false() {
        let mut arr = BarArray::new();
        assert!(!arr.update_last(make_bar(1000, 50.0)).unwrap());
        assert_eq!(arr.len(), 0);
    }

    // ── Invalid data rejection ──

    #[test]
    fn test_append_rejects_nan_values() {
        let mut arr = BarArray::new();
        let err = arr
            .append(Bar::new(
                1000,
                f64::NAN,
                f64::NAN,
                f64::NAN,
                f64::NAN,
                f64::NAN,
            ))
            .unwrap_err();
        assert!(err.contains("must be finite"));
    }

    #[test]
    fn test_set_rejects_infinity_values() {
        let mut arr = BarArray::new();
        let err = arr
            .set(vec![Bar::new(
                1000,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
            )])
            .unwrap_err();
        assert!(err.contains("must be finite"));
    }

    #[test]
    fn test_update_last_rejects_timestamp_mismatch() {
        let mut arr = BarArray::new();
        arr.append(make_bar(1000, 50.0)).unwrap();

        let err = arr.update_last(make_bar(2000, 60.0)).unwrap_err();
        assert!(err.contains("timestamp must equal last timestamp"));
    }

    #[test]
    fn test_append_rejects_non_increasing_timestamp() {
        let mut arr = BarArray::new();
        arr.append(make_bar(1000, 50.0)).unwrap();

        let err = arr.append(make_bar(1000, 60.0)).unwrap_err();
        assert!(err.contains("timestamp must be > last timestamp"));
    }

    // ── Direct accessor tests ──

    #[test]
    fn test_direct_accessors() {
        let mut arr = BarArray::new();
        arr.set(vec![
            Bar::new(1000, 10.0, 15.0, 8.0, 12.0, 100.0),
            Bar::new(2000, 12.0, 18.0, 11.0, 16.0, 200.0),
        ])
        .unwrap();

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
        arr.set(vec![make_bar(1000, 50.0), make_bar(2000, 60.0)])
            .unwrap();

        let bar0 = arr.get_unchecked(0);
        assert_eq!(bar0.close, 50.0);

        let bar1 = arr.get_unchecked(1);
        assert_eq!(bar1.close, 60.0);
    }

    #[test]
    fn test_bar_array_normalizes_finite_bounds() {
        let mut arr = BarArray::new();
        arr.append(Bar::new(1000, 10.0, 9.0, 12.0, 11.0, -5.0))
        .unwrap();

        let bar = arr.get(0).unwrap();
        assert_eq!(bar.high, 11.0);
        assert_eq!(bar.low, 10.0);
        assert_eq!(bar.volume, 0.0);
    }
}
