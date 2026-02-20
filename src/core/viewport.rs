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
    /// LWC scaleMargins.top — fraction of chart height reserved above data (default 0.2).
    pub scale_margin_top: f64,
    /// LWC scaleMargins.bottom — fraction of chart height reserved below data (default 0.1).
    pub scale_margin_bottom: f64,
    /// True when price range needs recalculation (LWC _invalidatedForRange pattern).
    pub price_invalidated: bool,
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
            scale_margin_top: 0.2,
            scale_margin_bottom: 0.1,
            price_invalidated: true,
        }
    }

    #[inline]
    pub fn visible_bar_count(&self) -> f64 {
        self.end_bar - self.start_bar
    }

    pub fn set_range(&mut self, start: f64, end: f64) {
        self.start_bar = start;
        self.end_bar = end.max(start + 1.0);
        self.price_invalidated = true;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w.max(1);
        self.height = h.max(1);
        self.price_invalidated = true;
    }

    /// Auto-fit price range to visible bars with LWC scaleMargins.
    ///
    /// LWC PriceScale uses scaleMargins { top: 0.2, bottom: 0.1 } by default,
    /// meaning the data occupies the inner 70% of the chart height, with 20%
    /// padding above and 10% below.
    pub fn auto_fit_price(&mut self, bars: &crate::core::data::BarArray) {
        if bars.is_empty() { return; }
        let start = (self.start_bar.floor() as usize).min(bars.len().saturating_sub(1));
        let end = (self.end_bar.ceil() as usize).min(bars.len());
        if start >= end { return; }

        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for i in start..end {
            let bar = bars.get(i);
            lo = lo.min(bar.low);
            hi = hi.max(bar.high);
        }

        let raw_range = (hi - lo) as f64;
        let internal_frac = 1.0 - self.scale_margin_top - self.scale_margin_bottom;
        if internal_frac <= 0.0 { return; }

        let full_range = if raw_range > 0.0 {
            raw_range / internal_frac
        } else {
            // Degenerate single price — extend by 10 units (LWC behavior)
            10.0 / internal_frac
        };
        self.price_min = lo as f64 - full_range * self.scale_margin_bottom;
        self.price_max = self.price_min + full_range;
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

    /// Convert a pixel X coordinate to the bar index whose slot contains it.
    ///
    /// Bar `i` occupies index range `[i, i+1)` — its center is at `i + 0.5`.
    /// `pixel_to_bar` returns a float in that range; `.floor()` gives the
    /// correct integer index.  This matches LWC's `coordinateToIndex` which
    /// uses `Math.ceil(floatIndex)` with a −0.5 offset (equivalent result).
    ///
    /// Returns `None` when the pixel maps outside `0..data_len`.
    #[inline]
    pub fn bar_index_at_pixel(&self, x_px: f64, chart_width_px: f64, data_len: usize) -> Option<usize> {
        let bar_f = self.pixel_to_bar(x_px, chart_width_px);
        let idx = bar_f.floor() as i64;
        if idx < 0 || idx >= data_len as i64 {
            None
        } else {
            Some(idx as usize)
        }
    }

    /// Compute the CSS-pixel X coordinate of bar `idx`'s center.
    ///
    /// This is the inverse of `bar_index_at_pixel` — it maps an integer bar
    /// index to the center of its slot in CSS-pixel space.
    #[inline]
    pub fn bar_center_css(&self, idx: usize, pane_css_w: f64) -> f64 {
        let frac = (idx as f64 + 0.5 - self.start_bar) / (self.end_bar - self.start_bar);
        (frac * pane_css_w).clamp(0.0, pane_css_w)
    }

    /// Fraction of pane height used for the candle area (1.0 − volume_height_ratio).
    #[inline]
    pub fn candle_height_frac(&self) -> f64 {
        1.0 - self.volume_height_ratio as f64
    }

    /// Convert a price to a CSS-pixel Y coordinate within the pane.
    ///
    /// The candle area occupies the top `candle_height_frac()` of the pane;
    /// volume occupies the bottom.  Y increases downward (0 = top of pane).
    #[inline]
    pub fn price_to_css_y(&self, price: f64, pane_css_h: f64) -> f64 {
        let range = self.price_max - self.price_min;
        if range <= 0.0 { return 0.0; }
        let frac = (price - self.price_min) / range;
        let candle_css_h = pane_css_h * self.candle_height_frac();
        (1.0 - frac) * candle_css_h
    }

    // --- Pan / Zoom helpers ---

    pub fn pan(&mut self, delta_bars: f64) {
        self.start_bar += delta_bars;
        self.end_bar += delta_bars;
        self.price_invalidated = true;
    }

    pub fn pan_clamped(&mut self, delta_bars: f64, data_len: usize) {
        let span = self.end_bar - self.start_bar;
        let half = span * 0.5;
        let lo = -half;
        let hi = data_len as f64 + half - span;

        let new_start = (self.start_bar + delta_bars).clamp(lo, hi);
        self.start_bar = new_start;
        self.end_bar = new_start + span;
        self.price_invalidated = true;
    }

    /// Clamp viewport so it doesn't scroll too far past data boundaries.
    /// Allows half a screen of whitespace on each side.
    pub fn clamp_to_data(&mut self, data_len: usize) {
        let span = self.end_bar - self.start_bar;
        let half = span * 0.5;
        let lo = -half;
        let hi = data_len as f64 + half - span;
        if self.start_bar < lo {
            self.start_bar = lo;
            self.end_bar = lo + span;
        } else if self.start_bar > hi {
            self.start_bar = hi;
            self.end_bar = hi + span;
        }
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
        self.price_invalidated = true;
    }
}
