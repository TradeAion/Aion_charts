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

/// A single OHLCV bar, 32 bytes, cache-line aligned.
///
/// Layout: [timestamp: 8][open: 4][high: 4][low: 4][close: 4][volume: 4][_pad: 4] = 32B
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Bar {
    /// Epoch milliseconds (UTC).
    pub timestamp: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    /// Volume for this bar.
    pub volume: f32,
    /// Padding to 32-byte boundary for GPU alignment.
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

/// A contiguous array of bars that can be sent to the GPU as a storage buffer
/// or received from JS as a typed array.
pub struct BarArray {
    bars: Vec<Bar>,
}

impl BarArray {
    pub fn new() -> Self {
        Self { bars: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self { bars: Vec::with_capacity(cap) }
    }

    /// Replace all bars. Takes ownership of the vec to avoid copies.
    pub fn set(&mut self, bars: Vec<Bar>) {
        self.bars = bars;
    }

    /// Append bars incrementally (streaming / real-time).
    pub fn append(&mut self, new_bars: &[Bar]) {
        self.bars.extend_from_slice(new_bars);
    }

    /// Update the last bar in-place (live tick).
    pub fn update_last(&mut self, bar: Bar) {
        if let Some(last) = self.bars.last_mut() {
            *last = bar;
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[Bar] {
        &self.bars
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bars.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bars.is_empty()
    }

    /// Raw bytes view for GPU upload — zero copy via bytemuck.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.bars)
    }
}
