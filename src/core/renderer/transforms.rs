//! Coordinate transform utilities — single source of truth.
//!
//! These functions convert between:
//! - Bar indices (data space) ↔ pixel X coordinates (screen space)
//! - Prices (data space) ↔ pixel Y coordinates (screen space)
//!
//! Used by geometry_generator, line_generator, and other renderers.

use crate::core::viewport::Viewport;

/// Convert a bar index to X pixel coordinate.
///
/// # Arguments
/// * `bar_idx` - The bar index (can be fractional for interpolation)
/// * `vp` - The current viewport
/// * `chart_w` - The chart width in pixels
#[inline]
pub fn bar_to_x(bar_idx: f64, vp: &Viewport, chart_w: f64) -> f64 {
    (bar_idx - vp.start_bar) / (vp.end_bar - vp.start_bar) * chart_w - 1.0
}

/// Convert a price to Y pixel coordinate.
///
/// Note: Y increases downward, so higher prices are at lower Y values.
/// Handles all price scale modes (Normal, Log, Percentage, IndexedTo100).
///
/// # Arguments
/// * `price` - The price value
/// * `vp` - The current viewport (uses price_min/price_max)
/// * `candle_h` - The candle area height in pixels
#[inline]
pub fn price_to_y(price: f64, vp: &Viewport, candle_h: f64) -> f64 {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 {
        return 0.0;
    }
    // Transform price to internal coordinate space (handles log/percentage modes)
    let internal = vp.price_to_internal(price);
    let frac = (internal - vp.price_min) / range;
    candle_h * (1.0 - frac)
}

/// Convert X pixel coordinate to bar index.
///
/// # Arguments
/// * `x_px` - X coordinate in pixels
/// * `vp` - The current viewport
/// * `chart_w` - The chart width in pixels
#[inline]
pub fn x_to_bar(x_px: f64, vp: &Viewport, chart_w: f64) -> f64 {
    vp.start_bar + ((x_px + 1.0) / chart_w) * (vp.end_bar - vp.start_bar)
}

/// Convert Y pixel coordinate to price.
///
/// Handles all price scale modes (Normal, Log, Percentage, IndexedTo100).
///
/// # Arguments
/// * `y_px` - Y coordinate in pixels
/// * `vp` - The current viewport
/// * `candle_h` - The candle area height in pixels
#[inline]
pub fn y_to_price(y_px: f64, vp: &Viewport, candle_h: f64) -> f64 {
    let frac = 1.0 - (y_px / candle_h);
    let internal = vp.price_min + frac * (vp.price_max - vp.price_min);
    // Transform back from internal coordinate space
    vp.internal_to_price(internal)
}

/// Snap to pixel center for Canvas2D (floor + 0.5 for crisp 1px lines).
#[inline]
pub fn snap_to_pixel(v: f64) -> f64 {
    v.floor() + 0.5
}

/// Snap to nearest pixel for grid lines (avoids subpixel blur).
#[inline]
pub fn snap_to_grid(v: f64) -> f64 {
    v.round()
}

/// Extract RGBA components from a color array.
#[inline]
pub fn color4(c: &[f32; 4]) -> (f32, f32, f32, f32) {
    (c[0], c[1], c[2], c[3])
}

#[cfg(test)]
mod tests {
    use super::{bar_to_x, x_to_bar};
    use crate::core::viewport::Viewport;

    #[test]
    fn bar_projection_matches_lwc_index_to_coordinate_formula() {
        let mut vp = Viewport::new(1000, 600);
        vp.set_range(10.0, 110.0);

        let x = bar_to_x(10.5, &vp, 1000.0);
        assert!(
            (x - 4.0).abs() < 1e-9,
            "expected first visible center at x=4, got {x}"
        );

        let x = bar_to_x(11.5, &vp, 1000.0);
        assert!(
            (x - 14.0).abs() < 1e-9,
            "expected second visible center at x=14, got {x}"
        );
    }

    #[test]
    fn x_to_bar_is_inverse_of_lwc_shifted_projection() {
        let mut vp = Viewport::new(1000, 600);
        vp.set_range(10.0, 110.0);

        let x = bar_to_x(42.5, &vp, 1000.0);
        let bar = x_to_bar(x, &vp, 1000.0);
        assert!(
            (bar - 42.5).abs() < 1e-9,
            "expected inverse projection to recover 42.5, got {bar}"
        );
    }
}
