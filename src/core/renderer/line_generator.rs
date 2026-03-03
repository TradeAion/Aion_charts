//! Line geometry generator — produces geometry for line series.
//!
//! Supports two rendering modes:
//! - **LineSegment**: Smooth anti-aliased diagonal lines for Canvas2D stroke rendering.
//! - **ColoredRect**: Staircase/walk-line approach as an alternative.
//!
//! The LineSegment approach produces proper diagonal lines that connect data
//! points directly, rendered with anti-aliasing by Canvas2D.

use crate::core::indicators::render::types::{DrawInstruction, RenderOrderKey};
use crate::core::renderer::baseline_utils::emit_split_segment_by_baseline;
use crate::core::renderer::draw_list::{ColoredRect, LineSegment};
use crate::core::renderer::transforms::{bar_to_x, price_to_y};
use crate::core::series::{Series, SeriesType};
use crate::core::viewport::Viewport;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub enum IndicatorGeometryPrimitive {
    Line(LineSegment),
    Rect(ColoredRect),
}

#[derive(Debug, Clone, Copy)]
pub struct IndicatorGeometryCommand {
    pub order: RenderOrderKey,
    pub sequence: u32,
    pub primitive: IndicatorGeometryPrimitive,
}

// ── Coordinate helpers imported from transforms.rs ───────────────────────────

/// Map a timestamp to a fractional bar index by binary-searching the bar
/// timestamps array. Returns None if the timestamp is outside the range.
fn timestamp_to_bar_index(ts: u64, bar_timestamps: &[u64]) -> Option<f64> {
    if bar_timestamps.is_empty() {
        return None;
    }
    // Exact match via binary search
    match bar_timestamps.binary_search(&ts) {
        Ok(idx) => Some(idx as f64),
        Err(idx) => {
            // Interpolate between surrounding bars
            if idx == 0 {
                // Before first bar — extrapolate left
                if bar_timestamps.len() >= 2 {
                    let dt = bar_timestamps[1] as f64 - bar_timestamps[0] as f64;
                    if dt > 0.0 {
                        let offset = (bar_timestamps[0] as f64 - ts as f64) / dt;
                        return Some(-offset);
                    }
                }
                None
            } else if idx >= bar_timestamps.len() {
                // After last bar — extrapolate right
                let n = bar_timestamps.len();
                if n >= 2 {
                    let dt = bar_timestamps[n - 1] as f64 - bar_timestamps[n - 2] as f64;
                    if dt > 0.0 {
                        let offset = (ts as f64 - bar_timestamps[n - 1] as f64) / dt;
                        return Some((n - 1) as f64 + offset);
                    }
                }
                None
            } else {
                // Between two bars — linear interpolation
                let t0 = bar_timestamps[idx - 1] as f64;
                let t1 = bar_timestamps[idx] as f64;
                let dt = t1 - t0;
                if dt > 0.0 {
                    let frac = (ts as f64 - t0) / dt;
                    Some((idx - 1) as f64 + frac)
                } else {
                    Some(idx as f64)
                }
            }
        }
    }
}

/// Generate pixel-space (x, y) points for a line series.
///
/// Shared by both rect-based rendering (Solid) and Canvas2D strokePath (dashed).
/// Returns empty vec if the series has fewer than 2 visible points.
pub fn generate_line_series_points(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
) -> Vec<(f64, f64)> {
    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    // Volume area calculation (same as candlestick rendering)
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let mut points: Vec<(f64, f64)> = Vec::with_capacity(data.len());

    for i in 0..data.len() {
        let pt = data.get(i);
        let bar_idx = match timestamp_to_bar_index(pt.timestamp, bar_timestamps) {
            Some(bi) => bi,
            None => i as f64, // fallback: use index directly
        };

        // Skip points far outside visible range (with margin)
        if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
            // But keep at least the boundary points for correct clipping
            if !points.is_empty() || i + 1 < data.len() {
                // Check if the next point is visible — if so, include this one
                let next_visible = if i + 1 < data.len() {
                    let next_ts = data.get(i + 1).timestamp;
                    if let Some(next_bi) = timestamp_to_bar_index(next_ts, bar_timestamps) {
                        next_bi >= viewport.start_bar - 2.0 && next_bi <= viewport.end_bar + 2.0
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !next_visible && (points.is_empty() || bar_idx < viewport.start_bar - 2.0) {
                    continue;
                }
            } else {
                continue;
            }
        }

        let px_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
        let px_y = price_to_y(pt.value as f64, viewport, candle_h).round();
        points.push((px_x, px_y));
    }

    points
}

// ═══════════════════════════════════════════════════════════════════════════════
// LineSegment Generators (anti-aliased lines)
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate LineSegment elements for a line series (Solid style only).
///
/// Produces smooth anti-aliased diagonal line segments for rendering.
/// Each segment connects two consecutive data points directly.
///
/// Dashed line series (Dotted, Dashed, LargeDashed, SparseDotted) are skipped
/// here — they are rendered via Canvas2D strokePath in the overlay renderer.
pub fn generate_line_segments(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    v_ratio: f64,
) -> Vec<LineSegment> {
    if series.series_type() != SeriesType::Line || !series.line_options.visible {
        return Vec::new();
    }

    // Skip dashed line series — they use Canvas2D strokePath rendering
    if series.line_options.line_style.is_dashed() {
        return Vec::new();
    }

    let opts = &series.line_options;
    let color = opts.color;
    let [r, g, b, a] = color;

    // Line width in physical pixels (round to whole pixel for crisp rendering)
    let line_w = (opts.line_width * v_ratio).round().max(1.0) as f32;

    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.len() < 2 {
        return Vec::new();
    }

    // For odd-width lines (1px, 3px), offset by 0.5 for pixel-center alignment
    // Points are already integer-rounded by generate_line_series_points
    let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

    let mut segments = Vec::with_capacity(points.len() - 1);

    for i in 0..points.len() - 1 {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];

        segments.push(LineSegment {
            x1: (x1 + correction) as f32,
            y1: (y1 + correction) as f32,
            x2: (x2 + correction) as f32,
            y2: (y2 + correction) as f32,
            width: line_w,
            r,
            g,
            b,
            a,
            _pad: 0.0,
        });
    }

    segments
}

/// Generate LineSegment elements for the line portion of an area series.
pub fn generate_area_line_segments(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    v_ratio: f64,
) -> Vec<LineSegment> {
    if series.series_type() != SeriesType::Area || !series.area_options.visible {
        return Vec::new();
    }

    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.area_options;
    let color = opts.line_color;
    let [r, g, b, a] = color;

    let line_w = (opts.line_width * v_ratio).round().max(1.0) as f32;

    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.len() < 2 {
        return Vec::new();
    }

    // For odd-width lines (1px, 3px), offset by 0.5 for pixel-center alignment
    let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

    let mut segments = Vec::with_capacity(points.len() - 1);

    for i in 0..points.len() - 1 {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];

        segments.push(LineSegment {
            x1: (x1 + correction) as f32,
            y1: (y1 + correction) as f32,
            x2: (x2 + correction) as f32,
            y2: (y2 + correction) as f32,
            width: line_w,
            r,
            g,
            b,
            a,
            _pad: 0.0,
        });
    }

    segments
}

/// Generate LineSegment elements for the line portion of a baseline series.
/// Uses two-tone coloring: above baseline uses top_line_color, below uses bottom_line_color.
pub fn generate_baseline_line_segments(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    v_ratio: f64,
) -> Vec<LineSegment> {
    if series.series_type() != SeriesType::Baseline || !series.baseline_options.visible {
        return Vec::new();
    }

    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.baseline_options;
    let top_color = opts.top_line_color;
    let bottom_color = opts.bottom_line_color;

    let line_w = (opts.line_width * v_ratio).round().max(1.0) as f32;

    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let base_y = price_to_y(opts.base_value, viewport, candle_h).round();

    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.len() < 2 {
        return Vec::new();
    }

    // For odd-width lines (1px, 3px), offset by 0.5 for pixel-center alignment
    let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

    let mut segments = Vec::with_capacity(points.len() - 1);

    for i in 0..points.len() - 1 {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];
        emit_split_segment_by_baseline(
            x1 + correction,
            y1 + correction,
            x2 + correction,
            y2 + correction,
            base_y + correction,
            top_color,
            bottom_color,
            |sx1, sy1, sx2, sy2, color| {
                segments.push(LineSegment {
                    x1: sx1 as f32,
                    y1: sy1 as f32,
                    x2: sx2 as f32,
                    y2: sy2 as f32,
                    width: line_w,
                    r: color[0],
                    g: color[1],
                    b: color[2],
                    a: color[3],
                    _pad: 0.0,
                });
            },
        );
    }

    segments
}

// ═══════════════════════════════════════════════════════════════════════════════
// ColoredRect Generators (fallback for Canvas2D)
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate ColoredRect segments for a single line series (Solid style only).
///
/// Uses the "connected horizontal segments" approach: for each data point,
/// draw a thin horizontal rect from the previous X to the current X at the
/// current Y, producing a simple stepped/connected line. For smoother
/// appearance, we draw a diagonal approximation using many small rects.
///
/// Dashed line series (Dotted, Dashed, LargeDashed, SparseDotted) are skipped
/// here — they are rendered via Canvas2D strokePath in the overlay renderer.
pub fn generate_line_rects(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    if series.series_type() != SeriesType::Line || !series.line_options.visible {
        return Vec::new();
    }

    // Skip dashed line series — they use Canvas2D strokePath rendering
    if series.line_options.line_style.is_dashed() {
        return Vec::new();
    }

    let opts = &series.line_options;
    let color = opts.color;
    let (cr, cg, cb, ca) = (color[0], color[1], color[2], color[3]);

    // Line width in physical pixels
    let line_w = (opts.line_width * v_ratio).round().max(1.0);
    let half_w = (line_w * 0.5).floor();

    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.len() < 2 {
        return Vec::new();
    }

    // Generate line segments between consecutive points
    let mut rects = Vec::with_capacity(points.len() * 2);

    for i in 0..points.len() - 1 {
        let (x0, y0) = points[i];
        let (x1, y1) = points[i + 1];

        let dx = x1 - x0;
        let dy = y1 - y0;

        if dx.abs() < 0.5 && dy.abs() < 0.5 {
            // Same pixel — draw a dot
            rects.push(ColoredRect {
                x: (x0 - half_w) as f32,
                y: (y0 - half_w) as f32,
                w: line_w as f32,
                h: line_w as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
            continue;
        }

        // Draw the line as a series of axis-aligned rects
        // Strategy: horizontal segment at y0, then vertical connector to y1
        // This matches the LWC walk-line approach

        // Horizontal segment: from x0 to x1 at y0
        let h_left = x0.min(x1);
        let h_right = x0.max(x1);
        let h_width = h_right - h_left;
        if h_width > 0.0 {
            rects.push(ColoredRect {
                x: h_left as f32,
                y: (y0 - half_w) as f32,
                w: h_width as f32,
                h: line_w as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }

        // Vertical connector: from y0 to y1 at x1
        if (y1 - y0).abs() > 0.5 {
            let v_top = y0.min(y1);
            let v_bottom = y0.max(y1);
            let v_height = v_bottom - v_top;
            rects.push(ColoredRect {
                x: (x1 - half_w) as f32,
                y: v_top as f32,
                w: line_w as f32,
                h: v_height as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }
    }

    rects
}

/// Generate fill rects for an area series.
///
/// Produces vertical strips of color between the line and the base (bottom
/// of the candle area, or a custom base_value). Each strip spans from the
/// data point's Y to the base Y. A simple linear color interpolation from
/// `top_color` (at the line) to `bottom_color` (at the base) is applied
/// per-strip based on the Y position relative to the full candle height.
pub fn generate_area_fill_rects(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    _v_ratio: f64,
) -> Vec<ColoredRect> {
    if series.series_type() != SeriesType::Area || !series.area_options.visible {
        return Vec::new();
    }

    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.area_options;
    let top_color = opts.top_color;
    let bottom_color = opts.bottom_color;

    // Volume area calculation (same as candlestick rendering)
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    // Base Y: bottom of candle area (or custom base_value)
    let base_y = if let Some(base_val) = opts.base_value {
        price_to_y(base_val, viewport, candle_h).round()
    } else {
        candle_h // bottom of candle area
    };

    // Pre-compute (px_x, px_y) pairs for visible points
    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.is_empty() {
        return Vec::new();
    }

    // Number of gradient bands per strip for smooth gradient
    let num_bands = 8usize;
    let mut rects = Vec::with_capacity(points.len() * num_bands);

    for i in 0..points.len() {
        let (x0, y0) = points[i];

        // Determine the horizontal extent of this strip
        let left = if i == 0 {
            x0
        } else {
            ((points[i - 1].0 + x0) / 2.0).round()
        };
        let right = if i == points.len() - 1 {
            x0
        } else {
            ((x0 + points[i + 1].0) / 2.0).round()
        };

        let strip_w = right - left;
        if strip_w <= 0.0 {
            continue;
        }

        // Fill from line Y (y0) to base_y with gradient bands
        let fill_top = y0.min(base_y);
        let fill_bottom = y0.max(base_y);
        let fill_h = fill_bottom - fill_top;

        if fill_h <= 0.5 {
            continue;
        }

        let band_h = fill_h / num_bands as f64;

        for b in 0..num_bands {
            let band_top = fill_top + b as f64 * band_h;
            let band_bottom = if b == num_bands - 1 {
                fill_bottom
            } else {
                fill_top + (b + 1) as f64 * band_h
            };
            let bh = band_bottom - band_top;
            if bh <= 0.0 {
                continue;
            }

            // Interpolation factor: 0.0 at the line (y0), 1.0 at the base
            let t = if fill_h > 0.0 {
                let band_mid = band_top + bh * 0.5;
                if opts.invert_filled_area {
                    1.0 - ((band_mid - fill_top) / fill_h) as f32
                } else {
                    ((band_mid - fill_top) / fill_h) as f32
                }
            } else {
                0.0
            };

            // If the line is above the base (normal case), t=0 is at line (top_color),
            // t=1 is at base (bottom_color). If inverted, swap.
            let (c_from, c_to) = if y0 <= base_y {
                // Normal: line is above base
                (top_color, bottom_color)
            } else {
                // Inverted: line is below base
                (bottom_color, top_color)
            };

            let r = c_from[0] + (c_to[0] - c_from[0]) * t;
            let g = c_from[1] + (c_to[1] - c_from[1]) * t;
            let b_c = c_from[2] + (c_to[2] - c_from[2]) * t;
            let a = c_from[3] + (c_to[3] - c_from[3]) * t;

            rects.push(ColoredRect {
                x: left as f32,
                y: band_top as f32,
                w: strip_w as f32,
                h: bh as f32,
                r,
                g,
                b: b_c,
                a,
            });
        }
    }

    rects
}

/// Generate the line portion of an area series (same algorithm as line series).
pub fn generate_area_line_rects(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    if series.series_type() != SeriesType::Area || !series.area_options.visible {
        return Vec::new();
    }

    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.area_options;
    let color = opts.line_color;
    let (cr, cg, cb, ca) = (color[0], color[1], color[2], color[3]);

    let line_w = (opts.line_width * v_ratio).round().max(1.0);
    let half_w = (line_w * 0.5).floor();

    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.len() < 2 {
        return Vec::new();
    }

    let mut rects = Vec::with_capacity(points.len() * 2);

    for i in 0..points.len() - 1 {
        let (x0, y0) = points[i];
        let (x1, y1) = points[i + 1];

        let dx = x1 - x0;
        let dy = y1 - y0;

        if dx.abs() < 0.5 && dy.abs() < 0.5 {
            rects.push(ColoredRect {
                x: (x0 - half_w) as f32,
                y: (y0 - half_w) as f32,
                w: line_w as f32,
                h: line_w as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
            continue;
        }

        let h_left = x0.min(x1);
        let h_right = x0.max(x1);
        let h_width = h_right - h_left;
        if h_width > 0.0 {
            rects.push(ColoredRect {
                x: h_left as f32,
                y: (y0 - half_w) as f32,
                w: h_width as f32,
                h: line_w as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }

        if (y1 - y0).abs() > 0.5 {
            let v_top = y0.min(y1);
            let v_bottom = y0.max(y1);
            let v_height = v_bottom - v_top;
            rects.push(ColoredRect {
                x: (x1 - half_w) as f32,
                y: v_top as f32,
                w: line_w as f32,
                h: v_height as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }
    }

    rects
}

/// Generate ColoredRect bars for a histogram series.
///
/// Each data point produces a vertical bar from the base value to the data value.
/// Bars are centered on the bar position and sized to fill the bar width with
/// a small gap (like LWC histogram). Per-bar color overrides are supported.
pub fn generate_histogram_rects(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    _v_ratio: f64,
) -> Vec<ColoredRect> {
    if series.series_type() != SeriesType::Histogram || !series.histogram_options.visible {
        return Vec::new();
    }

    let data = &series.histogram_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.histogram_options;
    let default_color = opts.color;

    // Volume area calculation (same as candlestick rendering)
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    // Base Y position
    let base_y = price_to_y(opts.base, viewport, candle_h).round();

    // Bar width: fill most of the bar slot with a small gap
    let visible_bars = (viewport.end_bar - viewport.start_bar).max(1.0);
    let bar_slot_w = pane_w / visible_bars;
    // LWC uses ~80% of bar width for histogram bars
    let bar_w = (bar_slot_w * 0.8).max(1.0);

    let mut rects = Vec::with_capacity(data.len());

    for i in 0..data.len() {
        let ts = data.timestamps[i];
        let value = data.values[i];

        let bar_idx = match timestamp_to_bar_index(ts, bar_timestamps) {
            Some(bi) => bi,
            None => i as f64,
        };

        // Skip points outside visible range (with margin)
        if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
            continue;
        }

        let px_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
        let px_y = price_to_y(value as f64, viewport, candle_h).round();

        // Bar extends from base_y to px_y
        let top = px_y.min(base_y);
        let bottom = px_y.max(base_y);
        let height = bottom - top;

        if height < 0.5 {
            continue;
        }

        // Get effective color (per-bar override or default)
        let color = data.effective_color(i, default_color);

        let left = (px_x - bar_w * 0.5).round();

        rects.push(ColoredRect {
            x: left as f32,
            y: top as f32,
            w: bar_w as f32,
            h: height as f32,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        });
    }

    rects
}

/// Generate ColoredRect elements for a Bar (OHLC) series.
///
/// Each bar is rendered as:
/// - A vertical line from high to low (the "stem")
/// - A horizontal tick on the left for the open price
/// - A horizontal tick on the right for the close price
/// Color is determined by bullish (close >= open) or bearish (close < open).
pub fn generate_bar_ohlc_rects(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    _v_ratio: f64,
) -> Vec<ColoredRect> {
    if series.series_type() != SeriesType::Bar || !series.bar_options.visible {
        return Vec::new();
    }

    let data = &series.bar_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.bar_options;

    // Volume area calculation
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    // Bar width calculations
    let visible_bars = (viewport.end_bar - viewport.start_bar).max(1.0);
    let bar_slot_w = pane_w / visible_bars;
    // Stem width: 1px for thin bars, otherwise scale with bar width
    let stem_w = if opts.thin_bars {
        1.0
    } else {
        (bar_slot_w * 0.1).max(1.0).round()
    };
    // Tick width: ~40% of bar slot on each side
    let tick_w = (bar_slot_w * 0.4).max(2.0).round();
    let tick_h = stem_w; // tick height matches stem width

    let mut rects = Vec::with_capacity(data.len() * 3);

    for i in 0..data.len() {
        let ts = data.timestamps[i];
        let bar_idx = match timestamp_to_bar_index(ts, bar_timestamps) {
            Some(bi) => bi,
            None => i as f64,
        };

        // Skip bars outside visible range
        if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
            continue;
        }

        let open = data.open[i];
        let high = data.high[i];
        let low = data.low[i];
        let close = data.close[i];

        let is_bullish = close >= open;
        let color = if is_bullish {
            opts.up_color
        } else {
            opts.down_color
        };
        let (cr, cg, cb, ca) = (color[0], color[1], color[2], color[3]);

        let center_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
        let high_y = price_to_y(high as f64, viewport, candle_h).round();
        let low_y = price_to_y(low as f64, viewport, candle_h).round();
        let open_y = price_to_y(open as f64, viewport, candle_h).round();
        let close_y = price_to_y(close as f64, viewport, candle_h).round();

        let half_stem = (stem_w * 0.5).floor();

        // 1. Vertical stem: high to low
        let stem_height = (low_y - high_y).max(1.0);
        rects.push(ColoredRect {
            x: (center_x - half_stem) as f32,
            y: high_y as f32,
            w: stem_w as f32,
            h: stem_height as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });

        // 2. Open tick: horizontal line to the left of center
        if opts.open_visible {
            rects.push(ColoredRect {
                x: (center_x - tick_w) as f32,
                y: (open_y - half_stem) as f32,
                w: tick_w as f32,
                h: tick_h as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }

        // 3. Close tick: horizontal line to the right of center
        rects.push(ColoredRect {
            x: center_x as f32,
            y: (close_y - half_stem) as f32,
            w: tick_w as f32,
            h: tick_h as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });
    }

    rects
}

/// Generate fill rects for a baseline series — two-tone gradient fill above/below base value.
///
/// Above the baseline: gradient from `top_fill_color1` (at line) to `top_fill_color2` (at baseline).
/// Below the baseline: gradient from `bottom_fill_color1` (at baseline) to `bottom_fill_color2` (at line).
pub fn generate_baseline_fill_rects(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    _v_ratio: f64,
) -> Vec<ColoredRect> {
    if series.series_type() != SeriesType::Baseline || !series.baseline_options.visible {
        return Vec::new();
    }

    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.baseline_options;

    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    // Base Y: the horizontal line that divides above/below
    let base_y = price_to_y(opts.base_value, viewport, candle_h).round();

    // Pre-compute pixel positions for visible points
    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.is_empty() {
        return Vec::new();
    }

    let num_bands = 6usize;
    let mut rects = Vec::with_capacity(points.len() * num_bands * 2);

    for i in 0..points.len() {
        let (x0, y0) = points[i];

        // Determine horizontal extent of this strip
        let left = if i == 0 {
            x0
        } else {
            ((points[i - 1].0 + x0) / 2.0).round()
        };
        let right = if i == points.len() - 1 {
            x0
        } else {
            ((x0 + points[i + 1].0) / 2.0).round()
        };

        let strip_w = right - left;
        if strip_w <= 0.0 {
            continue;
        }

        // Above baseline: fill from y0 up to base_y (when y0 < base_y, i.e. price > base)
        if y0 < base_y {
            let fill_top = y0;
            let fill_bottom = base_y;
            let fill_h = fill_bottom - fill_top;

            if fill_h > 0.5 {
                let band_h = fill_h / num_bands as f64;
                for b in 0..num_bands {
                    let band_top = fill_top + b as f64 * band_h;
                    let band_bottom = if b == num_bands - 1 {
                        fill_bottom
                    } else {
                        fill_top + (b + 1) as f64 * band_h
                    };
                    let bh = band_bottom - band_top;
                    if bh <= 0.0 {
                        continue;
                    }

                    // t=0 at line (top_fill_color1), t=1 at baseline (top_fill_color2)
                    let t = ((band_top + bh * 0.5 - fill_top) / fill_h) as f32;
                    let c1 = opts.top_fill_color1;
                    let c2 = opts.top_fill_color2;

                    rects.push(ColoredRect {
                        x: left as f32,
                        y: band_top as f32,
                        w: strip_w as f32,
                        h: bh as f32,
                        r: c1[0] + (c2[0] - c1[0]) * t,
                        g: c1[1] + (c2[1] - c1[1]) * t,
                        b: c1[2] + (c2[2] - c1[2]) * t,
                        a: c1[3] + (c2[3] - c1[3]) * t,
                    });
                }
            }
        }

        // Below baseline: fill from base_y down to y0 (when y0 > base_y, i.e. price < base)
        if y0 > base_y {
            let fill_top = base_y;
            let fill_bottom = y0;
            let fill_h = fill_bottom - fill_top;

            if fill_h > 0.5 {
                let band_h = fill_h / num_bands as f64;
                for b in 0..num_bands {
                    let band_top = fill_top + b as f64 * band_h;
                    let band_bottom = if b == num_bands - 1 {
                        fill_bottom
                    } else {
                        fill_top + (b + 1) as f64 * band_h
                    };
                    let bh = band_bottom - band_top;
                    if bh <= 0.0 {
                        continue;
                    }

                    // t=0 at baseline (bottom_fill_color1), t=1 at line (bottom_fill_color2)
                    let t = ((band_top + bh * 0.5 - fill_top) / fill_h) as f32;
                    let c1 = opts.bottom_fill_color1;
                    let c2 = opts.bottom_fill_color2;

                    rects.push(ColoredRect {
                        x: left as f32,
                        y: band_top as f32,
                        w: strip_w as f32,
                        h: bh as f32,
                        r: c1[0] + (c2[0] - c1[0]) * t,
                        g: c1[1] + (c2[1] - c1[1]) * t,
                        b: c1[2] + (c2[2] - c1[2]) * t,
                        a: c1[3] + (c2[3] - c1[3]) * t,
                    });
                }
            }
        }
    }

    rects
}

/// Generate the line portion of a baseline series — two-tone line that changes
/// color at the baseline crossing. Above baseline uses `top_line_color`,
/// below uses `bottom_line_color`.
pub fn generate_baseline_line_rects(
    series: &Series,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    if series.series_type() != SeriesType::Baseline || !series.baseline_options.visible {
        return Vec::new();
    }

    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.baseline_options;
    let top_color = opts.top_line_color;
    let bottom_color = opts.bottom_line_color;

    let line_w = (opts.line_width * v_ratio).round().max(1.0);
    let half_w = (line_w * 0.5).floor();

    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let base_y = price_to_y(opts.base_value, viewport, candle_h).round();

    let points = generate_line_series_points(series, viewport, bar_timestamps, pane_w, pane_h);

    if points.len() < 2 {
        return Vec::new();
    }

    let mut rects = Vec::with_capacity(points.len() * 3);

    for i in 0..points.len() - 1 {
        let (x0, y0) = points[i];
        let (x1, y1) = points[i + 1];

        // Determine segment color based on midpoint relative to baseline
        let mid_y = (y0 + y1) * 0.5;
        let color = if mid_y <= base_y {
            top_color
        } else {
            bottom_color
        };
        let (cr, cg, cb, ca) = (color[0], color[1], color[2], color[3]);

        let dx = x1 - x0;
        let dy = y1 - y0;

        if dx.abs() < 0.5 && dy.abs() < 0.5 {
            rects.push(ColoredRect {
                x: (x0 - half_w) as f32,
                y: (y0 - half_w) as f32,
                w: line_w as f32,
                h: line_w as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
            continue;
        }

        // Horizontal segment: from x0 to x1 at y0
        let h_left = x0.min(x1);
        let h_right = x0.max(x1);
        let h_width = h_right - h_left;
        if h_width > 0.0 {
            rects.push(ColoredRect {
                x: h_left as f32,
                y: (y0 - half_w) as f32,
                w: h_width as f32,
                h: line_w as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }

        // Vertical connector: from y0 to y1 at x1
        if (y1 - y0).abs() > 0.5 {
            let v_top = y0.min(y1);
            let v_bottom = y0.max(y1);
            let v_height = v_bottom - v_top;
            rects.push(ColoredRect {
                x: (x1 - half_w) as f32,
                y: v_top as f32,
                w: line_w as f32,
                h: v_height as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }
    }

    rects
}

/// Generate line rects for ALL visible overlay series (line, area, histogram, bar, baseline).
pub fn generate_all_line_rects(
    series: &crate::core::series::SeriesCollection,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    let mut all_rects = Vec::new();
    for s in series.iter() {
        match s.series_type() {
            SeriesType::Line if s.line_options.visible => {
                let rects = generate_line_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(rects);
            }
            SeriesType::Area if s.area_options.visible => {
                // Area fill first (behind the line)
                let fill_rects = generate_area_fill_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(fill_rects);
                // Then the line on top
                let line_rects = generate_area_line_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(line_rects);
            }
            SeriesType::Histogram if s.histogram_options.visible => {
                let rects = generate_histogram_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(rects);
            }
            SeriesType::Bar if s.bar_options.visible => {
                let rects = generate_bar_ohlc_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(rects);
            }
            SeriesType::Baseline if s.baseline_options.visible => {
                // Fill first (behind the line)
                let fill_rects = generate_baseline_fill_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(fill_rects);
                // Then the two-tone line on top
                let line_rects = generate_baseline_line_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(line_rects);
            }
            _ => {}
        }
    }
    all_rects
}

/// Generate LineSegments for ALL visible overlay line series (line, area, baseline).
///
/// This is used by the Canvas2D backend to render smooth anti-aliased indicator lines.
/// Histogram and Bar series don't use line segments — they continue to use rects.
///
/// Returns (line_segments, fill_rects) where:
/// - line_segments: smooth lines for Line, Area (line portion), and Baseline series
/// - fill_rects: area/baseline fill rects that still need rect rendering
pub fn generate_all_overlay_geometry(
    series: &crate::core::series::SeriesCollection,
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> (Vec<LineSegment>, Vec<ColoredRect>) {
    let mut all_segments = Vec::new();
    let mut all_rects = Vec::new();

    for s in series.iter() {
        match s.series_type() {
            SeriesType::Line if s.line_options.visible => {
                // Use smooth line segments for solid lines
                if !s.line_options.line_style.is_dashed() {
                    let segments = generate_line_segments(
                        s,
                        viewport,
                        bar_timestamps,
                        pane_w,
                        pane_h,
                        h_ratio,
                        v_ratio,
                    );
                    all_segments.extend(segments);
                }
                // Dashed lines are handled by Canvas2D overlay renderer
            }
            SeriesType::Area if s.area_options.visible => {
                // Area fill (rects) first
                let fill_rects = generate_area_fill_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(fill_rects);

                // Then smooth line segments on top
                let segments = generate_area_line_segments(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_segments.extend(segments);
            }
            SeriesType::Histogram if s.histogram_options.visible => {
                // Histogram uses rects only
                let rects = generate_histogram_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(rects);
            }
            SeriesType::Bar if s.bar_options.visible => {
                // Bar (OHLC) uses rects only
                let rects = generate_bar_ohlc_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(rects);
            }
            SeriesType::Baseline if s.baseline_options.visible => {
                // Baseline fill (rects) first
                let fill_rects = generate_baseline_fill_rects(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_rects.extend(fill_rects);

                // Then smooth two-tone line segments on top
                let segments = generate_baseline_line_segments(
                    s,
                    viewport,
                    bar_timestamps,
                    pane_w,
                    pane_h,
                    h_ratio,
                    v_ratio,
                );
                all_segments.extend(segments);
            }
            _ => {}
        }
    }

    (all_segments, all_rects)
}

fn push_indicator_line(
    line_segments: &mut Vec<LineSegment>,
    ordered_commands: &mut Vec<IndicatorGeometryCommand>,
    order: RenderOrderKey,
    sequence: &mut u32,
    segment: LineSegment,
) {
    line_segments.push(segment);
    ordered_commands.push(IndicatorGeometryCommand {
        order,
        sequence: *sequence,
        primitive: IndicatorGeometryPrimitive::Line(segment),
    });
    *sequence = sequence.saturating_add(1);
}

fn push_indicator_rect(
    fill_rects: &mut Vec<ColoredRect>,
    ordered_commands: &mut Vec<IndicatorGeometryCommand>,
    order: RenderOrderKey,
    sequence: &mut u32,
    rect: ColoredRect,
) {
    fill_rects.push(rect);
    ordered_commands.push(IndicatorGeometryCommand {
        order,
        sequence: *sequence,
        primitive: IndicatorGeometryPrimitive::Rect(rect),
    });
    *sequence = sequence.saturating_add(1);
}

/// Generate geometry for indicator runtime draw instructions.
///
/// V1 supports line/shape plus basic histogram/bar/candle visual primitives.
pub fn generate_indicator_instruction_geometry(
    instructions: &[DrawInstruction],
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> (Vec<LineSegment>, Vec<ColoredRect>) {
    let (line_segments, fill_rects, _) = generate_indicator_instruction_geometry_internal(
        instructions,
        viewport,
        bar_timestamps,
        pane_w,
        pane_h,
        h_ratio,
        v_ratio,
    );
    (line_segments, fill_rects)
}

pub fn generate_indicator_instruction_commands(
    instructions: &[DrawInstruction],
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> Vec<IndicatorGeometryCommand> {
    let (_, _, commands) = generate_indicator_instruction_geometry_internal(
        instructions,
        viewport,
        bar_timestamps,
        pane_w,
        pane_h,
        h_ratio,
        v_ratio,
    );
    commands
}

fn generate_indicator_instruction_geometry_internal(
    instructions: &[DrawInstruction],
    viewport: &Viewport,
    bar_timestamps: &[u64],
    pane_w: f64,
    pane_h: f64,
    _h_ratio: f64,
    v_ratio: f64,
) -> (
    Vec<LineSegment>,
    Vec<ColoredRect>,
    Vec<IndicatorGeometryCommand>,
) {
    let mut line_segments = Vec::new();
    let mut fill_rects = Vec::new();
    let mut ordered_commands = Vec::new();

    if instructions.is_empty() {
        return (line_segments, fill_rects, ordered_commands);
    }

    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let visible_bars = (viewport.end_bar - viewport.start_bar).max(1.0);
    let bar_slot_w = pane_w / visible_bars;
    let mut series_points_by_id: HashMap<String, Vec<(u64, f64)>> = HashMap::new();
    for instruction in instructions {
        match instruction {
            DrawInstruction::PlotLine {
                series_id, points, ..
            }
            | DrawInstruction::PlotArea {
                series_id, points, ..
            } => {
                if points.len() >= 2 {
                    series_points_by_id.insert(series_id.clone(), points.clone());
                }
            }
            _ => {}
        }
    }

    for instruction in instructions {
        let order = instruction.order_key();
        let mut sequence = 0u32;
        match instruction {
            DrawInstruction::PlotLine {
                points,
                color,
                width,
                ..
            } => {
                if points.len() < 2 {
                    continue;
                }

                let line_w = (*width as f64 * v_ratio).round().max(1.0) as f32;
                let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

                let mut projected: Vec<(f64, f64)> = Vec::with_capacity(points.len());
                for (ts, value) in points {
                    let Some(bar_idx) = timestamp_to_bar_index(*ts, bar_timestamps) else {
                        continue;
                    };
                    if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                        continue;
                    }
                    let px_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
                    let px_y = price_to_y(*value, viewport, candle_h).round();
                    projected.push((px_x, px_y));
                }

                if projected.len() < 2 {
                    continue;
                }

                for i in 0..(projected.len() - 1) {
                    let (x1, y1) = projected[i];
                    let (x2, y2) = projected[i + 1];
                    push_indicator_line(
                        &mut line_segments,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        LineSegment {
                            x1: (x1 + correction) as f32,
                            y1: (y1 + correction) as f32,
                            x2: (x2 + correction) as f32,
                            y2: (y2 + correction) as f32,
                            width: line_w,
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                            _pad: 0.0,
                        },
                    );
                }
            }
            DrawInstruction::PlotArea {
                points,
                top_color,
                bottom_color,
                ..
            } => {
                if points.len() < 2 {
                    continue;
                }

                let mut projected: Vec<(f64, f64)> = Vec::with_capacity(points.len());
                for (ts, value) in points {
                    let Some(bar_idx) = timestamp_to_bar_index(*ts, bar_timestamps) else {
                        continue;
                    };
                    if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                        continue;
                    }
                    let px_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
                    let px_y = price_to_y(*value, viewport, candle_h).round();
                    projected.push((px_x, px_y));
                }

                if projected.len() < 2 {
                    continue;
                }

                for i in 0..projected.len() {
                    let (x0, y0) = projected[i];
                    let left = if i == 0 {
                        x0
                    } else {
                        ((projected[i - 1].0 + x0) / 2.0).round()
                    };
                    let right = if i + 1 == projected.len() {
                        x0
                    } else {
                        ((x0 + projected[i + 1].0) / 2.0).round()
                    };
                    let strip_w = (right - left).max(1.0) as f32;
                    let top = y0.min(candle_h);
                    let bottom = y0.max(candle_h);
                    let fill_h = (bottom - top).max(1.0) as f32;
                    let t = ((y0 / candle_h).clamp(0.0, 1.0)) as f32;
                    let r = top_color[0] + (bottom_color[0] - top_color[0]) * t;
                    let g = top_color[1] + (bottom_color[1] - top_color[1]) * t;
                    let b = top_color[2] + (bottom_color[2] - top_color[2]) * t;
                    let a = top_color[3] + (bottom_color[3] - top_color[3]) * t;
                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: left as f32,
                            y: top as f32,
                            w: strip_w,
                            h: fill_h,
                            r,
                            g,
                            b,
                            a,
                        },
                    );
                }
            }
            DrawInstruction::PlotHistogram {
                points,
                color,
                base,
                ..
            } => {
                let base_y = price_to_y(*base, viewport, candle_h).round();
                let bar_w = (bar_slot_w * 0.8).max(1.0).round() as f32;

                for (timestamp, value) in points {
                    let Some(bar_idx) = timestamp_to_bar_index(*timestamp, bar_timestamps) else {
                        continue;
                    };
                    if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                        continue;
                    }
                    let center_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
                    let y = price_to_y(*value, viewport, candle_h).round();
                    let top = y.min(base_y);
                    let bottom = y.max(base_y);
                    let height = (bottom - top).max(1.0) as f32;
                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: center_x as f32 - bar_w * 0.5,
                            y: top as f32,
                            w: bar_w,
                            h: height,
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        },
                    );
                }
            }
            DrawInstruction::PlotBar {
                points,
                up_color,
                down_color,
                ..
            } => {
                let stem_w = (bar_slot_w * 0.1).max(1.0).round() as f32;
                let tick_w = (bar_slot_w * 0.4).max(2.0).round() as f32;
                let half_stem = stem_w * 0.5;

                for (timestamp, open, high, low, close) in points {
                    let Some(bar_idx) = timestamp_to_bar_index(*timestamp, bar_timestamps) else {
                        continue;
                    };
                    if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                        continue;
                    }
                    let color = if close >= open { up_color } else { down_color };
                    let center_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round() as f32;
                    let high_y = price_to_y(*high, viewport, candle_h).round() as f32;
                    let low_y = price_to_y(*low, viewport, candle_h).round() as f32;
                    let open_y = price_to_y(*open, viewport, candle_h).round() as f32;
                    let close_y = price_to_y(*close, viewport, candle_h).round() as f32;

                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: center_x - half_stem,
                            y: high_y,
                            w: stem_w,
                            h: (low_y - high_y).max(1.0),
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        },
                    );

                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: center_x - tick_w,
                            y: open_y - half_stem,
                            w: tick_w,
                            h: stem_w,
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        },
                    );

                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: center_x,
                            y: close_y - half_stem,
                            w: tick_w,
                            h: stem_w,
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        },
                    );
                }
            }
            DrawInstruction::PlotCandle {
                points,
                up_color,
                down_color,
                ..
            } => {
                let body_w = (bar_slot_w * 0.7).max(1.0).round() as f32;
                let wick_w = (body_w * 0.2).max(1.0).round() as f32;
                let half_body = body_w * 0.5;
                let half_wick = wick_w * 0.5;

                for (timestamp, open, high, low, close) in points {
                    let Some(bar_idx) = timestamp_to_bar_index(*timestamp, bar_timestamps) else {
                        continue;
                    };
                    if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                        continue;
                    }

                    let color = if close >= open { up_color } else { down_color };
                    let center_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round() as f32;
                    let high_y = price_to_y(*high, viewport, candle_h).round() as f32;
                    let low_y = price_to_y(*low, viewport, candle_h).round() as f32;
                    let open_y = price_to_y(*open, viewport, candle_h).round() as f32;
                    let close_y = price_to_y(*close, viewport, candle_h).round() as f32;
                    let body_top = open_y.min(close_y);
                    let body_bottom = open_y.max(close_y);

                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: center_x - half_wick,
                            y: high_y,
                            w: wick_w,
                            h: (low_y - high_y).max(1.0),
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        },
                    );

                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: center_x - half_body,
                            y: body_top,
                            w: body_w,
                            h: (body_bottom - body_top).max(1.0),
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        },
                    );
                }
            }
            DrawInstruction::DrawBox {
                x1,
                y1,
                x2,
                y2,
                line_color,
                fill_color,
                ..
            } => {
                let Some(left_bar_idx) = timestamp_to_bar_index(*x1, bar_timestamps) else {
                    continue;
                };
                let Some(right_bar_idx) = timestamp_to_bar_index(*x2, bar_timestamps) else {
                    continue;
                };
                let left_x = bar_to_x(left_bar_idx + 0.5, viewport, pane_w).round();
                let right_x = bar_to_x(right_bar_idx + 0.5, viewport, pane_w).round();
                let top_y = price_to_y(*y1, viewport, candle_h).round();
                let bottom_y = price_to_y(*y2, viewport, candle_h).round();
                let x_min = left_x.min(right_x);
                let x_max = left_x.max(right_x);
                let y_min = top_y.min(bottom_y);
                let y_max = top_y.max(bottom_y);
                let w = (x_max - x_min).max(1.0) as f32;
                let h = (y_max - y_min).max(1.0) as f32;

                push_indicator_rect(
                    &mut fill_rects,
                    &mut ordered_commands,
                    order,
                    &mut sequence,
                    ColoredRect {
                        x: x_min as f32,
                        y: y_min as f32,
                        w,
                        h,
                        r: fill_color[0],
                        g: fill_color[1],
                        b: fill_color[2],
                        a: fill_color[3],
                    },
                );

                let border_w = 1.0_f32.max(v_ratio as f32);
                push_indicator_rect(
                    &mut fill_rects,
                    &mut ordered_commands,
                    order,
                    &mut sequence,
                    ColoredRect {
                        x: x_min as f32,
                        y: y_min as f32,
                        w,
                        h: border_w,
                        r: line_color[0],
                        g: line_color[1],
                        b: line_color[2],
                        a: line_color[3],
                    },
                );
                push_indicator_rect(
                    &mut fill_rects,
                    &mut ordered_commands,
                    order,
                    &mut sequence,
                    ColoredRect {
                        x: x_min as f32,
                        y: y_max as f32 - border_w,
                        w,
                        h: border_w,
                        r: line_color[0],
                        g: line_color[1],
                        b: line_color[2],
                        a: line_color[3],
                    },
                );
                push_indicator_rect(
                    &mut fill_rects,
                    &mut ordered_commands,
                    order,
                    &mut sequence,
                    ColoredRect {
                        x: x_min as f32,
                        y: y_min as f32,
                        w: border_w,
                        h,
                        r: line_color[0],
                        g: line_color[1],
                        b: line_color[2],
                        a: line_color[3],
                    },
                );
                push_indicator_rect(
                    &mut fill_rects,
                    &mut ordered_commands,
                    order,
                    &mut sequence,
                    ColoredRect {
                        x: x_max as f32 - border_w,
                        y: y_min as f32,
                        w: border_w,
                        h,
                        r: line_color[0],
                        g: line_color[1],
                        b: line_color[2],
                        a: line_color[3],
                    },
                );
            }
            DrawInstruction::DrawPolyline {
                points,
                color,
                width,
                ..
            } => {
                if points.len() < 2 {
                    continue;
                }
                let line_w = (*width as f64 * v_ratio).round().max(1.0) as f32;
                let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
                let mut projected: Vec<(f64, f64)> = Vec::with_capacity(points.len());
                for (ts, value) in points {
                    let Some(bar_idx) = timestamp_to_bar_index(*ts, bar_timestamps) else {
                        continue;
                    };
                    if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                        continue;
                    }
                    let px_x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
                    let px_y = price_to_y(*value, viewport, candle_h).round();
                    projected.push((px_x, px_y));
                }
                if projected.len() < 2 {
                    continue;
                }
                for i in 0..(projected.len() - 1) {
                    let (x1, y1) = projected[i];
                    let (x2, y2) = projected[i + 1];
                    push_indicator_line(
                        &mut line_segments,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        LineSegment {
                            x1: (x1 + correction) as f32,
                            y1: (y1 + correction) as f32,
                            x2: (x2 + correction) as f32,
                            y2: (y2 + correction) as f32,
                            width: line_w,
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                            _pad: 0.0,
                        },
                    );
                }
            }
            DrawInstruction::DrawLine {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
                ..
            } => {
                let Some(bar_idx_1) = timestamp_to_bar_index(*x1, bar_timestamps) else {
                    continue;
                };
                let Some(bar_idx_2) = timestamp_to_bar_index(*x2, bar_timestamps) else {
                    continue;
                };
                let px_x1 = bar_to_x(bar_idx_1 + 0.5, viewport, pane_w).round();
                let px_y1 = price_to_y(*y1, viewport, candle_h).round();
                let px_x2 = bar_to_x(bar_idx_2 + 0.5, viewport, pane_w).round();
                let px_y2 = price_to_y(*y2, viewport, candle_h).round();
                let line_w = (*width as f64 * v_ratio).round().max(1.0) as f32;
                let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
                push_indicator_line(
                    &mut line_segments,
                    &mut ordered_commands,
                    order,
                    &mut sequence,
                    LineSegment {
                        x1: (px_x1 + correction) as f32,
                        y1: (px_y1 + correction) as f32,
                        x2: (px_x2 + correction) as f32,
                        y2: (px_y2 + correction) as f32,
                        width: line_w,
                        r: color[0],
                        g: color[1],
                        b: color[2],
                        a: color[3],
                        _pad: 0.0,
                    },
                );
            }
            DrawInstruction::PlotShape {
                timestamp,
                value,
                color,
                size,
                ..
            } => {
                let Some(bar_idx) = timestamp_to_bar_index(*timestamp, bar_timestamps) else {
                    continue;
                };
                if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                    continue;
                }
                let x = bar_to_x(bar_idx + 0.5, viewport, pane_w).round();
                let y = price_to_y(*value, viewport, candle_h).round();
                let side = (*size as f64 * v_ratio).round().max(1.0) as f32;
                let half = side * 0.5;
                push_indicator_rect(
                    &mut fill_rects,
                    &mut ordered_commands,
                    order,
                    &mut sequence,
                    ColoredRect {
                        x: x as f32 - half,
                        y: y as f32 - half,
                        w: side,
                        h: side,
                        r: color[0],
                        g: color[1],
                        b: color[2],
                        a: color[3],
                    },
                );
            }
            DrawInstruction::FillBetween {
                upper_series_id,
                lower_series_id,
                color,
                ..
            } => {
                let Some(upper_points) = series_points_by_id.get(upper_series_id) else {
                    continue;
                };
                let Some(lower_points) = series_points_by_id.get(lower_series_id) else {
                    continue;
                };
                if upper_points.len() < 2 || lower_points.len() < 2 {
                    continue;
                }

                let mut lower_by_ts = HashMap::<u64, f64>::with_capacity(lower_points.len());
                for (ts, value) in lower_points {
                    lower_by_ts.insert(*ts, *value);
                }

                let mut projected = Vec::<(f64, f64, f64)>::new();
                for (ts, upper_value) in upper_points {
                    let Some(lower_value) = lower_by_ts.get(ts) else {
                        continue;
                    };
                    let Some(bar_idx) = timestamp_to_bar_index(*ts, bar_timestamps) else {
                        continue;
                    };
                    if bar_idx < viewport.start_bar - 2.0 || bar_idx > viewport.end_bar + 2.0 {
                        continue;
                    }
                    projected.push((
                        bar_to_x(bar_idx + 0.5, viewport, pane_w).round(),
                        price_to_y(*upper_value, viewport, candle_h).round(),
                        price_to_y(*lower_value, viewport, candle_h).round(),
                    ));
                }

                if projected.len() < 2 {
                    continue;
                }

                for i in 0..projected.len() {
                    let (x0, upper_y, lower_y) = projected[i];
                    let left = if i == 0 {
                        x0
                    } else {
                        ((projected[i - 1].0 + x0) / 2.0).round()
                    };
                    let right = if i + 1 == projected.len() {
                        x0
                    } else {
                        ((x0 + projected[i + 1].0) / 2.0).round()
                    };
                    let top = upper_y.min(lower_y);
                    let bottom = upper_y.max(lower_y);
                    push_indicator_rect(
                        &mut fill_rects,
                        &mut ordered_commands,
                        order,
                        &mut sequence,
                        ColoredRect {
                            x: left as f32,
                            y: top as f32,
                            w: (right - left).max(1.0) as f32,
                            h: (bottom - top).max(1.0) as f32,
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        },
                    );
                }
            }
            _ => {}
        }
    }

    ordered_commands.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.sequence.cmp(&right.sequence))
    });

    (line_segments, fill_rects, ordered_commands)
}

#[cfg(test)]
mod tests {
    use super::{
        generate_indicator_instruction_commands, generate_indicator_instruction_geometry,
        IndicatorGeometryPrimitive,
    };
    use crate::core::indicators::render::types::{DrawInstruction, LayerBand, RenderOrderKey};
    use crate::core::viewport::Viewport;

    fn sample_order() -> RenderOrderKey {
        RenderOrderKey {
            layer_band: LayerBand::IndicatorSeries,
            z: 0,
            declaration_order: 0,
            stable_id: 1,
        }
    }

    fn sample_viewport() -> Viewport {
        let mut viewport = Viewport::new(300, 200);
        viewport.set_range(0.0, 3.0);
        viewport.price_min = 0.0;
        viewport.price_max = 20.0;
        viewport
    }

    #[test]
    fn indicator_plot_candle_generates_rects() {
        let viewport = sample_viewport();
        let instructions = vec![DrawInstruction::PlotCandle {
            order: sample_order(),
            series_id: "s".to_string(),
            points: vec![(1, 10.0, 12.0, 9.0, 11.0), (2, 11.0, 13.0, 10.0, 10.5)],
            up_color: [0.0, 1.0, 0.0, 1.0],
            down_color: [1.0, 0.0, 0.0, 1.0],
        }];

        let (segments, rects) = generate_indicator_instruction_geometry(
            &instructions,
            &viewport,
            &[1, 2, 3],
            300.0,
            200.0,
            1.0,
            1.0,
        );

        assert!(segments.is_empty());
        assert!(rects.len() >= 4);
    }

    #[test]
    fn indicator_plot_bar_generates_rects() {
        let viewport = sample_viewport();
        let instructions = vec![DrawInstruction::PlotBar {
            order: sample_order(),
            series_id: "s".to_string(),
            points: vec![(1, 10.0, 12.0, 9.0, 11.0), (2, 11.0, 13.0, 10.0, 10.5)],
            up_color: [0.0, 1.0, 0.0, 1.0],
            down_color: [1.0, 0.0, 0.0, 1.0],
        }];

        let (segments, rects) = generate_indicator_instruction_geometry(
            &instructions,
            &viewport,
            &[1, 2, 3],
            300.0,
            200.0,
            1.0,
            1.0,
        );

        assert!(segments.is_empty());
        assert!(rects.len() >= 6);
    }

    #[test]
    fn indicator_draw_box_generates_rects() {
        let viewport = sample_viewport();
        let instructions = vec![DrawInstruction::DrawBox {
            order: sample_order(),
            id: 1,
            x1: 1,
            y1: 12.0,
            x2: 2,
            y2: 8.0,
            line_color: [1.0, 1.0, 0.0, 1.0],
            fill_color: [1.0, 1.0, 0.0, 0.15],
        }];

        let (_segments, rects) = generate_indicator_instruction_geometry(
            &instructions,
            &viewport,
            &[1, 2, 3],
            300.0,
            200.0,
            1.0,
            1.0,
        );

        assert!(rects.len() >= 5);
    }

    #[test]
    fn indicator_draw_polyline_generates_segments() {
        let viewport = sample_viewport();
        let instructions = vec![DrawInstruction::DrawPolyline {
            order: sample_order(),
            id: 2,
            points: vec![(1, 10.0), (2, 11.0), (3, 9.0)],
            color: [0.0, 1.0, 1.0, 1.0],
            width: 2.0,
        }];

        let (segments, rects) = generate_indicator_instruction_geometry(
            &instructions,
            &viewport,
            &[1, 2, 3],
            300.0,
            200.0,
            1.0,
            1.0,
        );

        assert_eq!(segments.len(), 2);
        assert!(rects.is_empty());
    }

    #[test]
    fn indicator_plot_area_generates_fill_rects() {
        let viewport = sample_viewport();
        let instructions = vec![DrawInstruction::PlotArea {
            order: sample_order(),
            series_id: "area".to_string(),
            points: vec![(1, 10.0), (2, 12.0), (3, 11.0)],
            top_color: [0.0, 1.0, 0.0, 0.4],
            bottom_color: [0.0, 1.0, 0.0, 0.05],
        }];
        let (_segments, rects) = generate_indicator_instruction_geometry(
            &instructions,
            &viewport,
            &[1, 2, 3],
            300.0,
            200.0,
            1.0,
            1.0,
        );
        assert!(!rects.is_empty());
    }

    #[test]
    fn indicator_fill_between_generates_fill_rects() {
        let viewport = sample_viewport();
        let instructions = vec![
            DrawInstruction::PlotLine {
                order: sample_order(),
                series_id: "upper".to_string(),
                points: vec![(1, 14.0), (2, 15.0), (3, 14.0)],
                color: [0.0, 1.0, 1.0, 1.0],
                width: 2.0,
            },
            DrawInstruction::PlotLine {
                order: sample_order(),
                series_id: "lower".to_string(),
                points: vec![(1, 10.0), (2, 9.0), (3, 10.0)],
                color: [1.0, 0.0, 1.0, 1.0],
                width: 2.0,
            },
            DrawInstruction::FillBetween {
                order: sample_order(),
                upper_series_id: "upper".to_string(),
                lower_series_id: "lower".to_string(),
                color: [0.2, 0.6, 0.9, 0.2],
            },
        ];

        let (_segments, rects) = generate_indicator_instruction_geometry(
            &instructions,
            &viewport,
            &[1, 2, 3],
            300.0,
            200.0,
            1.0,
            1.0,
        );
        assert!(!rects.is_empty());
    }

    #[test]
    fn indicator_commands_preserve_cross_primitive_ordering() {
        let viewport = sample_viewport();
        let mut line_order = sample_order();
        line_order.z = 2;
        let mut shape_order = sample_order();
        shape_order.z = 5;
        let instructions = vec![
            DrawInstruction::PlotLine {
                order: line_order,
                series_id: "line".to_string(),
                points: vec![(1, 10.0), (2, 11.0), (3, 12.0)],
                color: [0.0, 1.0, 1.0, 1.0],
                width: 2.0,
            },
            DrawInstruction::PlotShape {
                order: shape_order,
                shape: "square".to_string(),
                timestamp: 2,
                value: 11.5,
                color: [1.0, 0.0, 0.0, 1.0],
                size: 4.0,
            },
        ];

        let commands = generate_indicator_instruction_commands(
            &instructions,
            &viewport,
            &[1, 2, 3],
            300.0,
            200.0,
            1.0,
            1.0,
        );
        assert!(commands.len() >= 3);
        assert!(matches!(
            commands.first().map(|it| it.primitive),
            Some(IndicatorGeometryPrimitive::Line(_))
        ));
        assert!(matches!(
            commands.last().map(|it| it.primitive),
            Some(IndicatorGeometryPrimitive::Rect(_))
        ));
        let first_order = commands.first().map(|it| it.order.z).unwrap_or_default();
        let last_order = commands.last().map(|it| it.order.z).unwrap_or_default();
        assert!(first_order <= last_order);
    }
}
