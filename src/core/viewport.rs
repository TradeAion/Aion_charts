//! Viewport — manages the visible range of bars, zoom level, and
//! coordinate conversions between bar-index/price space and pixel space.
//!
//! With the unified geometry architecture, the Viewport no longer produces
//! GPU uniform blocks. Instead, GeometryGenerator uses Viewport's coordinate
//! helpers to compute pixel-space rectangles.
//!
//! Supports multiple price scale modes: Normal, Logarithmic, Percentage, IndexedTo100.

use crate::core::constants::{
    DEFAULT_PRICE_MAX, DEFAULT_SCALE_MARGIN_BOTTOM, DEFAULT_SCALE_MARGIN_TOP,
    DEFAULT_VOLUME_HEIGHT_RATIO, DEGENERATE_PRICE_RANGE_FALLBACK, MIN_VISIBLE_BARS,
};

/// Price scale mode — determines how prices are mapped to visual coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceScaleMode {
    /// Normal linear price scale.
    Normal,
    /// Logarithmic scale — better for assets with large price swings.
    Logarithmic,
    /// Percentage scale — shows price change as % from first visible value.
    Percentage,
    /// Indexed to 100 — shows price relative to first value, starting at 100.
    IndexedTo100,
}

impl Default for PriceScaleMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl PriceScaleMode {
    /// Parse from a string (for WASM API).
    pub fn from_str(s: &str) -> Self {
        match s {
            "logarithmic" | "log" => Self::Logarithmic,
            "percentage" | "percent" => Self::Percentage,
            "indexed_to_100" | "indexedTo100" | "indexed" => Self::IndexedTo100,
            _ => Self::Normal,
        }
    }
}

/// Adaptive log formula parameters (LWC pattern).
/// Used to handle negative values and values near zero gracefully.
#[derive(Debug, Clone, Copy)]
struct LogFormula {
    /// Log base adjustment for values near zero.
    log_base: f64,
    /// Offset added before taking log.
    log_offset: f64,
}

impl LogFormula {
    /// Create a log formula adapted to the given price range.
    fn for_range(min: f64, max: f64) -> Self {
        // LWC uses a complex adaptive formula. Simplified version:
        // For prices that can go to zero or negative, we need an offset.
        let _min_pos = min.max(1e-10);
        let log_offset = if min <= 0.0 { 1.0 - min } else { 0.0 };
        let log_base = 10.0_f64; // standard log10 scale
        Self {
            log_base,
            log_offset,
        }
    }

    /// Convert price to log space.
    #[inline]
    fn to_log(&self, price: f64) -> f64 {
        (price + self.log_offset).max(1e-10).ln() / self.log_base.ln()
    }

    /// Convert log space back to price.
    #[inline]
    fn from_log(&self, log_val: f64) -> f64 {
        self.log_base.powf(log_val) - self.log_offset
    }
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
    /// LWC scaleMargins.top — fraction of chart height reserved above data (default 0.2).
    pub scale_margin_top: f64,
    /// LWC scaleMargins.bottom — fraction of chart height reserved below data (default 0.1).
    pub scale_margin_bottom: f64,
    /// True when price range needs recalculation (LWC _invalidatedForRange pattern).
    pub price_invalidated: bool,
    /// Price scale mode (Normal, Logarithmic, Percentage, IndexedTo100).
    pub price_scale_mode: PriceScaleMode,
    /// First value for percentage/indexed modes (the reference price).
    pub first_value: f64,
    /// Cached log formula for logarithmic mode.
    log_formula: LogFormula,
}

impl Viewport {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            start_bar: 0.0,
            end_bar: DEFAULT_PRICE_MAX, // Default visible bars
            price_min: 0.0,
            price_max: DEFAULT_PRICE_MAX,
            width,
            height,
            volume_height_ratio: DEFAULT_VOLUME_HEIGHT_RATIO as f32,
            price_locked: false,
            scale_margin_top: DEFAULT_SCALE_MARGIN_TOP,
            scale_margin_bottom: DEFAULT_SCALE_MARGIN_BOTTOM,
            price_invalidated: true,
            price_scale_mode: PriceScaleMode::Normal,
            first_value: 0.0,
            log_formula: LogFormula {
                log_base: 10.0,
                log_offset: 0.0,
            },
        }
    }

    /// Set the price scale mode.
    pub fn set_price_scale_mode(&mut self, mode: PriceScaleMode) {
        if self.price_scale_mode != mode {
            self.price_scale_mode = mode;
            self.price_invalidated = true;
        }
    }

    /// Update the first_value reference for percentage/indexed modes.
    /// Should be called when visible data changes or mode is set.
    pub fn update_first_value(&mut self, bars: &crate::core::data::BarArray) {
        if bars.is_empty() {
            self.first_value = 0.0;
            return;
        }
        // Use the first visible bar's close as reference
        let start_idx = (self.start_bar.floor() as usize).min(bars.len().saturating_sub(1));
        self.first_value = bars.closes.value(start_idx) as f64;
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
        for i in start..end {
            // SAFETY: i is bounded by start..end which are clamped to bars.len()
            let bar = bars.get_unchecked(i);
            lo = lo.min(bar.low);
            hi = hi.max(bar.high);
        }

        // Update first_value for percentage/indexed modes
        // Flush to ensure Arrow arrays are up to date
        self.first_value = bars.close(start) as f64;

        // For log mode, update the formula
        if self.price_scale_mode == PriceScaleMode::Logarithmic {
            self.log_formula = LogFormula::for_range(lo as f64, hi as f64);
        }

        // Transform to internal coordinate space based on mode
        let (internal_lo, internal_hi) = match self.price_scale_mode {
            PriceScaleMode::Normal => (lo as f64, hi as f64),
            PriceScaleMode::Logarithmic => (
                self.log_formula.to_log(lo as f64),
                self.log_formula.to_log(hi as f64),
            ),
            PriceScaleMode::Percentage => {
                if self.first_value.abs() < 1e-10 {
                    (0.0, 100.0)
                } else {
                    let lo_pct = 100.0 * (lo as f64 - self.first_value) / self.first_value;
                    let hi_pct = 100.0 * (hi as f64 - self.first_value) / self.first_value;
                    (lo_pct, hi_pct)
                }
            }
            PriceScaleMode::IndexedTo100 => {
                if self.first_value.abs() < 1e-10 {
                    (100.0, 100.0)
                } else {
                    let lo_idx = 100.0 * lo as f64 / self.first_value;
                    let hi_idx = 100.0 * hi as f64 / self.first_value;
                    (lo_idx, hi_idx)
                }
            }
        };

        let raw_range = internal_hi - internal_lo;
        let internal_frac = 1.0 - self.scale_margin_top - self.scale_margin_bottom;
        if internal_frac <= 0.0 {
            return;
        }

        let full_range = if raw_range > 0.0 {
            raw_range / internal_frac
        } else {
            // Degenerate single price — extend by fallback value (LWC behavior)
            DEGENERATE_PRICE_RANGE_FALLBACK / internal_frac
        };
        self.price_min = internal_lo - full_range * self.scale_margin_bottom;
        self.price_max = self.price_min + full_range;
    }

    // --- Coordinate conversion helpers ---

    #[inline]
    pub fn bar_to_frac(&self, bar_idx: f64) -> f64 {
        (bar_idx - self.start_bar) / (self.end_bar - self.start_bar)
    }

    #[inline]
    pub fn price_to_frac(&self, price: f64) -> f64 {
        let internal = self.price_to_internal(price);
        (internal - self.price_min) / (self.price_max - self.price_min)
    }

    /// Convert a raw price to internal coordinate space based on scale mode.
    #[inline]
    pub fn price_to_internal(&self, price: f64) -> f64 {
        match self.price_scale_mode {
            PriceScaleMode::Normal => price,
            PriceScaleMode::Logarithmic => self.log_formula.to_log(price),
            PriceScaleMode::Percentage => {
                if self.first_value.abs() < 1e-10 {
                    0.0
                } else {
                    100.0 * (price - self.first_value) / self.first_value
                }
            }
            PriceScaleMode::IndexedTo100 => {
                if self.first_value.abs() < 1e-10 {
                    100.0
                } else {
                    100.0 * price / self.first_value
                }
            }
        }
    }

    /// Convert internal coordinate space back to raw price.
    #[inline]
    pub fn internal_to_price(&self, internal: f64) -> f64 {
        match self.price_scale_mode {
            PriceScaleMode::Normal => internal,
            PriceScaleMode::Logarithmic => self.log_formula.from_log(internal),
            PriceScaleMode::Percentage => {
                if self.first_value.abs() < 1e-10 {
                    0.0
                } else {
                    self.first_value * (1.0 + internal / 100.0)
                }
            }
            PriceScaleMode::IndexedTo100 => {
                if self.first_value.abs() < 1e-10 {
                    0.0
                } else {
                    self.first_value * internal / 100.0
                }
            }
        }
    }

    #[inline]
    pub fn pixel_to_bar(&self, x_px: f64, chart_width_px: f64) -> f64 {
        let frac = x_px / chart_width_px;
        self.start_bar + frac * (self.end_bar - self.start_bar)
    }

    #[inline]
    pub fn pixel_to_price(&self, y_px: f64, chart_height_px: f64) -> f64 {
        let frac = 1.0 - (y_px / chart_height_px);
        let internal = self.price_min + frac * (self.price_max - self.price_min);
        self.internal_to_price(internal)
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
    pub fn bar_index_at_pixel(
        &self,
        x_px: f64,
        chart_width_px: f64,
        data_len: usize,
    ) -> Option<usize> {
        let bar_f = self.pixel_to_bar(x_px, chart_width_px);
        let idx = bar_f.floor() as i64;
        if idx < 0 || idx >= data_len as i64 {
            None
        } else {
            Some(idx as usize)
        }
    }

    /// Convert a pixel X coordinate to a bar index for crosshair snapping.
    ///
    /// Like LWC: the crosshair can go into empty space (beyond data), but still
    /// snaps to the bar grid. Returns the grid-snapped index which may be >= data_len.
    /// Returns `None` only if the index would be negative.
    #[inline]
    pub fn bar_index_for_crosshair(&self, x_px: f64, chart_width_px: f64) -> Option<usize> {
        let bar_f = self.pixel_to_bar(x_px, chart_width_px);
        let idx = bar_f.floor() as i64;
        if idx < 0 {
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
    /// Handles all price scale modes (Normal, Log, Percentage, IndexedTo100).
    #[inline]
    pub fn price_to_css_y(&self, price: f64, pane_css_h: f64) -> f64 {
        let range = self.price_max - self.price_min;
        if range <= 0.0 {
            return 0.0;
        }
        let internal = self.price_to_internal(price);
        let frac = (internal - self.price_min) / range;
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
        if self.end_bar - self.start_bar < MIN_VISIBLE_BARS {
            let mid = (self.start_bar + self.end_bar) / 2.0;
            self.start_bar = mid - MIN_VISIBLE_BARS / 2.0;
            self.end_bar = mid + MIN_VISIBLE_BARS / 2.0;
        }
        self.price_invalidated = true;
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Basic viewport operations ──

    #[test]
    fn test_viewport_new() {
        let vp = Viewport::new(800, 600);
        assert_eq!(vp.width, 800);
        assert_eq!(vp.height, 600);
        assert!(!vp.price_locked);
    }

    #[test]
    fn test_visible_bar_count() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);
        assert_eq!(vp.visible_bar_count(), 100.0);

        vp.set_range(50.0, 150.0);
        assert_eq!(vp.visible_bar_count(), 100.0);
    }

    #[test]
    fn test_set_range_min_bars() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 0.5); // Less than 1 bar
                                // end_bar is forced to be at least start + 1
        assert!(vp.end_bar >= vp.start_bar + 1.0);
    }

    #[test]
    fn test_resize() {
        let mut vp = Viewport::new(800, 600);
        vp.resize(1024, 768);
        assert_eq!(vp.width, 1024);
        assert_eq!(vp.height, 768);
        assert!(vp.price_invalidated);
    }

    // ── Coordinate conversion ──

    #[test]
    fn test_bar_to_frac() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);

        assert_eq!(vp.bar_to_frac(0.0), 0.0);
        assert_eq!(vp.bar_to_frac(50.0), 0.5);
        assert_eq!(vp.bar_to_frac(100.0), 1.0);
    }

    #[test]
    fn test_price_to_frac() {
        let mut vp = Viewport::new(800, 600);
        vp.price_min = 100.0;
        vp.price_max = 200.0;

        assert_eq!(vp.price_to_frac(100.0), 0.0);
        assert_eq!(vp.price_to_frac(150.0), 0.5);
        assert_eq!(vp.price_to_frac(200.0), 1.0);
    }

    #[test]
    fn test_price_to_css_y_inverts() {
        let mut vp = Viewport::new(800, 600);
        vp.price_min = 100.0;
        vp.price_max = 200.0;

        // Higher prices should have lower Y values (top of screen)
        let y_low = vp.price_to_css_y(100.0, 600.0);
        let y_high = vp.price_to_css_y(200.0, 600.0);

        assert!(y_high < y_low); // High price = lower Y
    }

    // ── Zoom operations ──

    #[test]
    fn test_zoom_in_reduces_visible_bars() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);

        let initial_bars = vp.visible_bar_count();
        vp.zoom(50.0, 0.5); // Zoom in by factor of 0.5

        assert!(vp.visible_bar_count() < initial_bars);
    }

    #[test]
    fn test_zoom_out_increases_visible_bars() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);

        let initial_bars = vp.visible_bar_count();
        vp.zoom(50.0, 2.0); // Zoom out by factor of 2

        assert!(vp.visible_bar_count() > initial_bars);
    }

    #[test]
    fn test_zoom_respects_minimum_bars() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 10.0);

        // Zoom in extremely
        for _ in 0..20 {
            vp.zoom(5.0, 0.1);
        }

        // Should not go below MIN_VISIBLE_BARS
        assert!(vp.visible_bar_count() >= MIN_VISIBLE_BARS);
    }

    #[test]
    fn test_zoom_focal_point() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);

        // Zoom at focal point 25 (25% from left)
        let focal = 25.0;
        vp.zoom(focal, 0.5);

        // The focal bar should remain at approximately the same screen position
        // (This is a proportional zoom)
        let frac_after = vp.bar_to_frac(focal);
        assert!((frac_after - 0.25).abs() < 0.001);
    }

    // ── Pan operations ──

    #[test]
    fn test_pan() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);

        vp.pan(10.0);

        assert_eq!(vp.start_bar, 10.0);
        assert_eq!(vp.end_bar, 110.0);
    }

    #[test]
    fn test_pan_clamped_left() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);

        vp.pan_clamped(-50.0, 200); // Try to pan left beyond start

        // Should stop at start (with some allowance for left edge)
        assert!(vp.start_bar >= -100.0); // Can go some bars left
    }

    #[test]
    fn test_pan_clamped_right() {
        let mut vp = Viewport::new(800, 600);
        vp.set_range(0.0, 100.0);

        vp.pan_clamped(500.0, 200); // Try to pan way past end

        // Should be clamped to reasonable range
        assert!(vp.end_bar <= 400.0); // 2x data len max
    }

    // ── Price scale modes ──

    #[test]
    fn test_price_scale_mode_default_is_normal() {
        let vp = Viewport::new(800, 600);
        assert_eq!(vp.price_scale_mode, PriceScaleMode::Normal);
    }

    #[test]
    fn test_set_price_scale_mode() {
        let mut vp = Viewport::new(800, 600);
        vp.set_price_scale_mode(PriceScaleMode::Logarithmic);
        assert_eq!(vp.price_scale_mode, PriceScaleMode::Logarithmic);
        assert!(vp.price_invalidated);
    }

    #[test]
    fn test_price_scale_mode_from_str() {
        assert_eq!(PriceScaleMode::from_str("normal"), PriceScaleMode::Normal);
        assert_eq!(PriceScaleMode::from_str("log"), PriceScaleMode::Logarithmic);
        assert_eq!(
            PriceScaleMode::from_str("logarithmic"),
            PriceScaleMode::Logarithmic
        );
        assert_eq!(
            PriceScaleMode::from_str("percentage"),
            PriceScaleMode::Percentage
        );
        assert_eq!(
            PriceScaleMode::from_str("percent"),
            PriceScaleMode::Percentage
        );
        assert_eq!(
            PriceScaleMode::from_str("indexedTo100"),
            PriceScaleMode::IndexedTo100
        );
        assert_eq!(PriceScaleMode::from_str("unknown"), PriceScaleMode::Normal);
    }

    // ── Candle height fraction ──

    #[test]
    fn test_candle_height_frac() {
        let mut vp = Viewport::new(800, 600);
        vp.volume_height_ratio = 0.2;

        // Candle area should be 80% of height
        // Use approximate comparison due to f32->f64 conversion
        assert!((vp.candle_height_frac() - 0.8).abs() < 1e-6);
    }
}
