//! Line geometry generator — produces ColoredRect segments for line series.
//!
//! Uses the LWC "walk-line" approach: for each pair of consecutive data points,
//! draw a horizontal rect at the source Y, then a vertical connector to the
//! next Y level. This produces crisp, pixel-perfect staircase rendering
//! identical to LWC's line series.
//!
//! An alternative "diagonal" mode could be added later with anti-aliased lines.

use crate::core::viewport::Viewport;
use crate::core::renderer::draw_list::ColoredRect;
use crate::core::series::{Series, SeriesType};

// ── Coordinate helpers (same as geometry_generator.rs) ───────────────────────

#[inline]
fn bar_to_x(bar_idx: f64, vp: &Viewport, chart_w: f64) -> f64 {
    (bar_idx - vp.start_bar) / (vp.end_bar - vp.start_bar) * chart_w
}

#[inline]
fn price_to_y(price: f64, vp: &Viewport, candle_h: f64) -> f64 {
    let frac = (price - vp.price_min) / (vp.price_max - vp.price_min);
    candle_h * (1.0 - frac)
}

/// Map a timestamp to a fractional bar index by binary-searching the bar
/// timestamps array. Returns None if the timestamp is outside the range.
fn timestamp_to_bar_index(
    ts: u64,
    bar_timestamps: &[u64],
) -> Option<f64> {
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

/// Generate ColoredRect segments for a single line series.
///
/// Uses the "connected horizontal segments" approach: for each data point,
/// draw a thin horizontal rect from the previous X to the current X at the
/// current Y, producing a simple stepped/connected line. For smoother
/// appearance, we draw a diagonal approximation using many small rects.
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

    let data = &series.line_data;
    if data.is_empty() {
        return Vec::new();
    }

    let opts = &series.line_options;
    let color = opts.color;
    let (cr, cg, cb, ca) = (color[0], color[1], color[2], color[3]);

    // Line width in physical pixels
    let line_w = (opts.line_width * v_ratio).round().max(1.0);
    let half_w = (line_w * 0.5).floor();

    // Volume area calculation (same as candlestick rendering)
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    // Pre-compute (bar_index, physical_pixel) pairs for visible points
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
                r: cr, g: cg, b: cb, a: ca,
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
                r: cr, g: cg, b: cb, a: ca,
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
                r: cr, g: cg, b: cb, a: ca,
            });
        }
    }

    rects
}

/// Generate line rects for ALL visible line series.
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
        if s.series_type() == SeriesType::Line && s.line_options.visible {
            let rects = generate_line_rects(s, viewport, bar_timestamps, pane_w, pane_h, h_ratio, v_ratio);
            all_rects.extend(rects);
        }
    }
    all_rects
}
