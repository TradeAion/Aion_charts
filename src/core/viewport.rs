//! Viewport — manages the visible range of bars, zoom level, and
//! orthographic projection for the time-price coordinate space.
//!
//! Design decisions:
//! - All state is in logical bar-indices and price-units, not pixels.
//! - The projection matrix is computed on demand and uploaded as a uniform.
//! - Zoom/pan are expressed as bar range [start_idx .. end_idx] so that
//!   LOD selection is trivial (just count visible bars).

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// GPU-uploadable uniform block for the orthographic camera.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ViewportUniforms {
    /// Column-major 4x4 ortho projection.
    pub projection: [f32; 16],
    /// Viewport width in pixels (for sub-pixel snapping in shaders).
    pub width_px: f32,
    /// Viewport height in pixels.
    pub height_px: f32,
    /// Visible bar count (for LOD decisions in shaders).
    pub visible_bars: f32,
    pub _pad: f32,
}

/// Logical viewport state — bar range + price range + screen size.
pub struct Viewport {
    /// First visible bar index (can be fractional for smooth scrolling).
    pub start_bar: f64,
    /// Last visible bar index.
    pub end_bar: f64,
    /// Minimum price in view (auto-scaled or user-locked).
    pub price_min: f64,
    /// Maximum price in view.
    pub price_max: f64,
    /// Screen dimensions in physical pixels.
    pub width: u32,
    pub height: u32,
    /// How much of the height is reserved for volume (0.0 – 1.0).
    pub volume_height_ratio: f32,
    /// True if price axis is locked by user.
    pub price_locked: bool,
}

impl Viewport {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            start_bar: 0.0,
            end_bar: 100.0,
            price_min: 0.0,
            price_max: 100.0,
            width,
            height,
            volume_height_ratio: 0.15,
            price_locked: false,
        }
    }

    /// Number of bars currently visible.
    #[inline]
    pub fn visible_bar_count(&self) -> f64 {
        self.end_bar - self.start_bar
    }

    /// Set visible bar range (zoom_to_range).
    pub fn set_range(&mut self, start: f64, end: f64) {
        self.start_bar = start;
        self.end_bar = end.max(start + 1.0);
    }

    /// Resize the viewport (called on canvas resize).
    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w.max(1);
        self.height = h.max(1);
    }

    /// Auto-fit price range to visible bars.
    pub fn auto_fit_price(&mut self, bars: &[crate::core::data::Bar]) {
        if bars.is_empty() {
            return;
        }
        let start = (self.start_bar.floor() as usize).min(bars.len().saturating_sub(1));
        let end = (self.end_bar.ceil() as usize).min(bars.len());
        if start >= end {
            return;
        }

        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for bar in &bars[start..end] {
            lo = lo.min(bar.low);
            hi = hi.max(bar.high);
        }

        // Add 5% padding
        let pad = (hi - lo) * 0.05;
        self.price_min = (lo - pad) as f64;
        self.price_max = (hi + pad) as f64;
    }

    /// Build the candle-area orthographic projection.
    /// X maps [start_bar .. end_bar] -> [-1, 1]
    /// Y maps [price_min .. price_max] -> [-1, 1] (bottom portion reserved for volume)
    pub fn candle_projection(&self) -> Mat4 {
        let _vol_frac = self.volume_height_ratio;
        // Candle area occupies the top (1 - _vol_frac) of the screen.
        // In NDC, bottom = -1, top = 1, so candle area is [vol_bottom .. 1.0]
        // We compute an ortho that maps bar-space to the full [-1, 1] then
        // the vertex shader will not need to know about the split.
        Mat4::orthographic_rh(
            self.start_bar as f32,
            self.end_bar as f32,
            self.price_min as f32,
            self.price_max as f32,
            -1.0,
            1.0,
        )
    }

    /// Build the volume-area orthographic projection.
    /// X same as candles. Y maps [0 .. max_volume] -> [-1, 1].
    pub fn volume_projection(&self, max_volume: f32) -> Mat4 {
        Mat4::orthographic_rh(
            self.start_bar as f32,
            self.end_bar as f32,
            0.0,
            max_volume.max(1.0),
            -1.0,
            1.0,
        )
    }

    /// Produce the GPU-ready uniform struct for candle rendering.
    pub fn candle_uniforms(&self) -> ViewportUniforms {
        let proj = self.candle_projection();
        ViewportUniforms {
            projection: proj.to_cols_array(),
            width_px: self.width as f32,
            height_px: self.height as f32,
            visible_bars: self.visible_bar_count() as f32,
            _pad: 0.0,
        }
    }

    /// Produce the GPU-ready uniform struct for volume rendering.
    pub fn volume_uniforms(&self, max_vol: f32) -> ViewportUniforms {
        let proj = self.volume_projection(max_vol);
        ViewportUniforms {
            projection: proj.to_cols_array(),
            width_px: self.width as f32,
            height_px: self.height as f32,
            visible_bars: self.visible_bar_count() as f32,
            _pad: 0.0,
        }
    }

    // --- Pan / Zoom helpers ---

    /// Pan by `delta_bars` (negative = left, positive = right).
    pub fn pan(&mut self, delta_bars: f64) {
        self.start_bar += delta_bars;
        self.end_bar += delta_bars;
    }

    /// Zoom around a focal bar index. factor > 1 zooms out, < 1 zooms in.
    pub fn zoom(&mut self, focal_bar: f64, factor: f64) {
        let left = focal_bar - self.start_bar;
        let right = self.end_bar - focal_bar;
        self.start_bar = focal_bar - left * factor;
        self.end_bar = focal_bar + right * factor;
        // Clamp minimum visible bars to 5
        if self.end_bar - self.start_bar < 5.0 {
            let mid = (self.start_bar + self.end_bar) / 2.0;
            self.start_bar = mid - 2.5;
            self.end_bar = mid + 2.5;
        }
    }
}
