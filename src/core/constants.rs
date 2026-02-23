//! Centralized constants for RayCharts.
//!
//! This module eliminates magic numbers scattered throughout the codebase,
//! improving readability and maintainability. Constants are organized by category.

// ═══════════════════════════════════════════════════════════════════════════════
// Viewport & Zoom
// ═══════════════════════════════════════════════════════════════════════════════

/// Default number of visible bars when chart is first loaded or reset.
pub const DEFAULT_INITIAL_VISIBLE_BARS: f64 = 200.0;

/// Minimum number of bars that can be visible (maximum zoom in).
pub const MIN_VISIBLE_BARS: f64 = 5.0;

/// Default maximum price value when no data is loaded.
pub const DEFAULT_PRICE_MAX: f64 = 100.0;

/// Default top margin for price scale (percentage of visible range).
pub const DEFAULT_SCALE_MARGIN_TOP: f64 = 0.2;

/// Default bottom margin for price scale (percentage of visible range).
pub const DEFAULT_SCALE_MARGIN_BOTTOM: f64 = 0.1;

/// Volume pane height as ratio of main pane height.
pub const DEFAULT_VOLUME_HEIGHT_RATIO: f64 = 0.15;

/// Fallback price range when data is degenerate (all same price).
pub const DEGENERATE_PRICE_RANGE_FALLBACK: f64 = 10.0;

/// Threshold ratio from edge to trigger auto-scroll (10% from right edge).
pub const AUTO_SCROLL_THRESHOLD_RATIO: f64 = 0.1;

// ═══════════════════════════════════════════════════════════════════════════════
// Time Constants (milliseconds)
// ═══════════════════════════════════════════════════════════════════════════════

/// Time window for detecting double-click/double-tap (ms).
pub const DOUBLE_CLICK_WINDOW_MS: f64 = 500.0;

/// Maximum time between samples for velocity calculation (ms).
pub const VELOCITY_SAMPLE_WINDOW_MS: f64 = 100.0;

/// Maximum delta-time to trigger kinetic scrolling (ms).
pub const KINETIC_TRIGGER_WINDOW_MS: f64 = 50.0;

/// Frame time for physics calculations (ms) — ~60fps.
pub const PHYSICS_FRAME_MS: f64 = 16.0;

/// Animation cycle period for pulsing effects (ms).
pub const ANIMATION_CYCLE_MS: f64 = 1000.0;

// ═══════════════════════════════════════════════════════════════════════════════
// Interaction Physics
// ═══════════════════════════════════════════════════════════════════════════════

/// Divisor for wheel zoom — higher = slower zoom.
pub const ZOOM_FACTOR_DIVISOR: f64 = 10.0;

/// Multiplier for pinch gesture scale.
pub const PINCH_SCALE_MULTIPLIER: f64 = 5.0;

/// Minimum pinch scale factor.
pub const MIN_PINCH_SCALE: f64 = 0.1;

/// Friction coefficient for kinetic scrolling (per frame).
pub const KINETIC_FRICTION_COEFFICIENT: f64 = 0.95;

/// Minimum velocity to trigger kinetic scrolling.
pub const MIN_KINETIC_VELOCITY: f64 = 0.1;

/// Minimum velocity to continue gliding animation.
pub const MIN_GLIDE_VELOCITY: f64 = 0.01;

/// Smoothing factor for velocity calculation (0-1, higher = more smoothing).
pub const VELOCITY_SMOOTHING_FACTOR: f64 = 0.5;

// ═══════════════════════════════════════════════════════════════════════════════
// Wheel Event Handling
// ═══════════════════════════════════════════════════════════════════════════════

/// Multiplier for DOM_DELTA_PAGE wheel events.
pub const WHEEL_DELTA_PAGE_MULTIPLIER: f64 = 120.0;

/// Multiplier for DOM_DELTA_LINE wheel events.
pub const WHEEL_DELTA_LINE_MULTIPLIER: f64 = 32.0;

/// Divisor for normalizing wheel speed across browsers.
pub const WHEEL_SPEED_DIVISOR: f64 = 100.0;

/// Multiplier for converting wheel delta to scroll amount.
pub const SCROLL_MULTIPLIER: f64 = -80.0;

// ═══════════════════════════════════════════════════════════════════════════════
// Time Axis Interaction
// ═══════════════════════════════════════════════════════════════════════════════

/// Minimum visible bars during time axis drag.
pub const TIME_AXIS_MIN_BARS: f64 = 2.0;

/// Maximum bar count multiplier for time axis zoom out.
pub const TIME_AXIS_MAX_BAR_MULTIPLIER: f64 = 4.0;

// ═══════════════════════════════════════════════════════════════════════════════
// Price Scale Interaction
// ═══════════════════════════════════════════════════════════════════════════════

/// Coefficient for price scale offset during drag.
pub const PRICE_SCALE_OFFSET_COEFF: f64 = 0.2;

/// Minimum coefficient for price scale.
pub const MIN_PRICE_SCALE_COEFF: f64 = 0.1;

/// Divisor for price step calculation.
pub const PRICE_STEP_DIVISOR: f64 = 10.0;

// ═══════════════════════════════════════════════════════════════════════════════
// Pane Layout (CSS pixels)
// ═══════════════════════════════════════════════════════════════════════════════

/// Minimum height for indicator panes (CSS pixels).
pub const MIN_INDICATOR_PANE_HEIGHT_CSS: f64 = 50.0;

/// Minimum height for main chart pane (CSS pixels).
pub const MIN_MAIN_PANE_HEIGHT_CSS: f64 = 100.0;

/// Height of pane separator/divider (CSS pixels).
pub const PANE_SEPARATOR_HEIGHT_CSS: f64 = 4.0;

/// Stretch factor for main pane relative to indicator panes.
pub const MAIN_PANE_STRETCH_FACTOR: f64 = 3.0;

// ═══════════════════════════════════════════════════════════════════════════════
// Drawing Tools (CSS pixels)
// ═══════════════════════════════════════════════════════════════════════════════

/// Radius of anchor points for drawing tools (CSS pixels).
pub const ANCHOR_RADIUS_CSS: f64 = 5.0;

/// Padding from chart edge for legend overlay (CSS pixels).
pub const LEGEND_PADDING_CSS: f64 = 6.0;

/// Gap between legend label and value (CSS pixels).
pub const LEGEND_GAP_CSS: f64 = 4.0;

/// Base radius for pulsing last-price dot (CSS pixels).
pub const LAST_PRICE_DOT_BASE_RADIUS_CSS: f64 = 4.0;

// ═══════════════════════════════════════════════════════════════════════════════
// Tick Mark Spacing
// ═══════════════════════════════════════════════════════════════════════════════

/// Target spacing between Y-axis tick marks (CSS pixels).
pub const Y_TICK_TARGET_SPACING_CSS: f64 = 40.0;

/// Minimum number of Y-axis tick marks.
pub const Y_TICK_MIN_COUNT: f64 = 3.0;

/// Maximum number of Y-axis tick marks.
pub const Y_TICK_MAX_COUNT: f64 = 15.0;

/// Target spacing between X-axis tick marks (CSS pixels).
pub const X_TICK_TARGET_SPACING_CSS: f64 = 100.0;

/// Minimum number of X-axis tick marks.
pub const X_TICK_MIN_COUNT: f64 = 2.0;

// ═══════════════════════════════════════════════════════════════════════════════
// Animation & Visual Effects
// ═══════════════════════════════════════════════════════════════════════════════

/// Multiplier for dash pattern length.
pub const DASH_LENGTH_MULTIPLIER: f64 = 2.0;

/// Default pulse phase (0.0-1.0).
pub const DEFAULT_PULSE_PHASE: f64 = 0.5;

/// Minimum scale for pulse animation.
pub const PULSE_MIN_SCALE: f64 = 0.8;

/// Range of scale variation for pulse animation.
pub const PULSE_SCALE_RANGE: f64 = 0.4;

/// Minimum alpha for pulse animation.
pub const PULSE_MIN_ALPHA: f64 = 0.6;

/// Range of alpha variation for pulse animation.
pub const PULSE_ALPHA_RANGE: f64 = 0.4;

// ═══════════════════════════════════════════════════════════════════════════════
// Unit Tests — Sanity checks for constant values
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_constants_are_positive() {
        assert!(DEFAULT_INITIAL_VISIBLE_BARS > 0.0);
        assert!(MIN_VISIBLE_BARS > 0.0);
        assert!(DEFAULT_PRICE_MAX > 0.0);
    }

    #[test]
    fn test_min_visible_bars_less_than_default() {
        assert!(MIN_VISIBLE_BARS < DEFAULT_INITIAL_VISIBLE_BARS);
    }

    #[test]
    fn test_scale_margins_are_fractions() {
        assert!(DEFAULT_SCALE_MARGIN_TOP > 0.0 && DEFAULT_SCALE_MARGIN_TOP < 1.0);
        assert!(DEFAULT_SCALE_MARGIN_BOTTOM > 0.0 && DEFAULT_SCALE_MARGIN_BOTTOM < 1.0);
        assert!(DEFAULT_SCALE_MARGIN_TOP + DEFAULT_SCALE_MARGIN_BOTTOM < 1.0);
    }

    #[test]
    fn test_time_constants_are_positive() {
        assert!(DOUBLE_CLICK_WINDOW_MS > 0.0);
        assert!(VELOCITY_SAMPLE_WINDOW_MS > 0.0);
        assert!(KINETIC_TRIGGER_WINDOW_MS > 0.0);
        assert!(PHYSICS_FRAME_MS > 0.0);
        assert!(ANIMATION_CYCLE_MS > 0.0);
    }

    #[test]
    fn test_physics_constants_in_valid_range() {
        assert!(KINETIC_FRICTION_COEFFICIENT > 0.0 && KINETIC_FRICTION_COEFFICIENT < 1.0);
        assert!(VELOCITY_SMOOTHING_FACTOR >= 0.0 && VELOCITY_SMOOTHING_FACTOR <= 1.0);
        assert!(MIN_KINETIC_VELOCITY > 0.0);
        assert!(MIN_GLIDE_VELOCITY > 0.0);
        assert!(MIN_GLIDE_VELOCITY < MIN_KINETIC_VELOCITY);
    }

    #[test]
    fn test_layout_constants_are_positive() {
        assert!(MIN_INDICATOR_PANE_HEIGHT_CSS > 0.0);
        assert!(MIN_MAIN_PANE_HEIGHT_CSS > 0.0);
        assert!(PANE_SEPARATOR_HEIGHT_CSS > 0.0);
        assert!(MAIN_PANE_STRETCH_FACTOR > 0.0);
    }

    #[test]
    fn test_main_pane_larger_than_indicator() {
        assert!(MIN_MAIN_PANE_HEIGHT_CSS >= MIN_INDICATOR_PANE_HEIGHT_CSS);
    }

    #[test]
    fn test_drawing_constants_are_positive() {
        assert!(ANCHOR_RADIUS_CSS > 0.0);
        assert!(LEGEND_PADDING_CSS > 0.0);
        assert!(LAST_PRICE_DOT_BASE_RADIUS_CSS > 0.0);
    }

    #[test]
    fn test_tick_spacing_constants() {
        assert!(Y_TICK_TARGET_SPACING_CSS > 0.0);
        assert!(X_TICK_TARGET_SPACING_CSS > 0.0);
        assert!(Y_TICK_MIN_COUNT > 0.0);
        assert!(Y_TICK_MAX_COUNT > Y_TICK_MIN_COUNT);
        assert!(X_TICK_MIN_COUNT > 0.0);
    }

    #[test]
    fn test_pulse_animation_ranges() {
        assert!(PULSE_MIN_SCALE > 0.0 && PULSE_MIN_SCALE < 1.0);
        assert!(PULSE_SCALE_RANGE > 0.0);
        assert!(PULSE_MIN_ALPHA > 0.0 && PULSE_MIN_ALPHA < 1.0);
        assert!(PULSE_ALPHA_RANGE > 0.0);
    }
}
