//! Viewport — manages the visible range of bars, zoom level, and
//! coordinate conversions between bar-index/price space and pixel space.
//!
//! With the unified geometry architecture, the Viewport no longer produces
//! GPU uniform blocks. Instead, GeometryGenerator uses Viewport's coordinate
//! helpers to compute pixel-space rectangles.

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

    #[inline]
    pub fn visible_bar_count(&self) -> f64 {
        self.end_bar - self.start_bar
    }

    pub fn set_range(&mut self, start: f64, end: f64) {
        self.start_bar = start;
        self.end_bar = end.max(start + 1.0);
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w.max(1);
        self.height = h.max(1);
    }

    pub fn auto_fit_price(&mut self, bars: &[crate::core::data::Bar]) {
        if bars.is_empty() { return; }
        let start = (self.start_bar.floor() as usize).min(bars.len().saturating_sub(1));
        let end = (self.end_bar.ceil() as usize).min(bars.len());
        if start >= end { return; }

        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for bar in &bars[start..end] {
            lo = lo.min(bar.low);
            hi = hi.max(bar.high);
        }

        let pad = (hi - lo) * 0.05;
        self.price_min = (lo - pad) as f64;
        self.price_max = (hi + pad) as f64;
    }

    // --- Coordinate conversion helpers ---

    #[inline]
    pub fn bar_to_frac(&self, bar_idx: f64) -> f64 {
        (bar_idx - self.start_bar) / (self.end_bar - self.start_bar)
    }

    #[inline]
    pub fn price_to_frac(&self, price: f64) -> f64 {
        (price - self.price_min) / (self.price_max - self.price_min)
    }

    #[inline]
    pub fn pixel_to_bar(&self, x_px: f64, chart_width_px: f64) -> f64 {
        let frac = x_px / chart_width_px;
        self.start_bar + frac * (self.end_bar - self.start_bar)
    }

    #[inline]
    pub fn pixel_to_price(&self, y_px: f64, chart_height_px: f64) -> f64 {
        let frac = 1.0 - (y_px / chart_height_px);
        self.price_min + frac * (self.price_max - self.price_min)
    }

    // --- Pan / Zoom helpers ---

    pub fn pan(&mut self, delta_bars: f64) {
        self.start_bar += delta_bars;
        self.end_bar += delta_bars;
    }

    pub fn pan_clamped(&mut self, delta_bars: f64, data_len: usize) {
        let span = self.end_bar - self.start_bar;
        let half = span * 0.5;
        let lo = -half;
        let hi = data_len as f64 + half - span;

        let new_start = (self.start_bar + delta_bars).clamp(lo, hi);
        self.start_bar = new_start;
        self.end_bar = new_start + span;
    }

    pub fn zoom(&mut self, focal_bar: f64, factor: f64) {
        let left = focal_bar - self.start_bar;
        let right = self.end_bar - focal_bar;
        self.start_bar = focal_bar - left * factor;
        self.end_bar = focal_bar + right * factor;
        if self.end_bar - self.start_bar < 5.0 {
            let mid = (self.start_bar + self.end_bar) / 2.0;
            self.start_bar = mid - 2.5;
            self.end_bar = mid + 2.5;
        }
    }
}
