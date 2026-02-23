//! GeometryGenerator — single source of truth for ALL visual math.
//!
//! Takes Viewport, data, style → produces pixel-perfect ColoredRects.
//! Both Canvas2D and WebGPU (fallback rect path) consume these identically.
//!
//! Public API is split per element type so each ChartRenderer phase can
//! request only the geometry it needs:
//! - `generate_grid_rects`   — background grid lines
//! - `generate_candle_rects` — wicks + borders + body fills
//! - `generate_volume_rects` — volume bars
//! - `generate`              — legacy all-in-one (background + grid + volume + candles)
//!
//! All candle sizing uses LWC-matching algorithms from series.rs.
//! Tick computation is in tick_marks.rs (shared with axis renderers).

use crate::core::renderer::baseline_utils::emit_split_segment_by_baseline;
use crate::core::renderer::draw_list::{ColoredRect, DrawList};
use crate::core::renderer::series::CandleSizing;
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::transforms::{bar_to_x, color4, price_to_y};
use crate::core::viewport::Viewport;

/// Generate the complete DrawList for one frame (legacy monolithic path).
/// Order: background → grid lines → volume → candles.
/// `pane_w` and `pane_h` are in physical pixels (chart area only, no axes).
pub fn generate(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
    y_ticks: &[TickMark],
    x_ticks: &[TickMark],
) -> DrawList {
    let mut dl = DrawList::new();

    // Background fill
    let (br, bg, bb, ba) = color4(&style.bg_color);
    dl.rects.push(ColoredRect {
        x: 0.0,
        y: 0.0,
        w: pane_w as f32,
        h: pane_h as f32,
        r: br,
        g: bg,
        b: bb,
        a: ba,
    });

    // Grid lines (as thin rects, 1 physical pixel wide)
    let grid = generate_grid_rects(style, y_ticks, x_ticks, pane_w, pane_h);
    dl.rects.extend_from_slice(&grid);

    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, h_ratio, v_ratio);

    // Volume occupies the bottom portion of pane (configured via viewport)
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    generate_volume_into(
        bars,
        viewport,
        style,
        pane_w,
        candle_h,
        vol_h,
        &sizing,
        &mut dl.rects,
    );
    generate_candles_into(
        bars,
        viewport,
        style,
        pane_w,
        candle_h,
        &sizing,
        &mut dl.rects,
    );

    dl
}

// ── Public per-element generators ────────────────────────────────────────────

/// Generate grid line rects (horizontal at price ticks, vertical at time ticks).
/// This is the SINGLE SOURCE OF TRUTH for grid line generation.
/// All renderers (Canvas2D, WebGPU, subpanes) should use this function.
///
/// CURRENTLY DISABLED - returns empty vector.
pub fn generate_grid_rects(
    _style: &ChartStyle,
    _y_ticks: &[TickMark],
    _x_ticks: &[TickMark],
    _pane_w: f64,
    _pane_h: f64,
) -> Vec<ColoredRect> {
    // Grid lines disabled
    Vec::new()
}

/// Generate candle rects (wicks, borders, body fills).
pub fn generate_candle_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, h_ratio, v_ratio);
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len() * 6);
    generate_candles_into(bars, viewport, style, pane_w, candle_h, &sizing, &mut rects);
    rects
}

/// Generate volume bar rects.
pub fn generate_volume_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, h_ratio, v_ratio);
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len());
    generate_volume_into(
        bars, viewport, style, pane_w, candle_h, vol_h, &sizing, &mut rects,
    );
    rects
}

// ── Coordinate helpers imported from transforms.rs ───────────────────────────
// bar_to_x, price_to_y, and color4 are now in crate::core::renderer::transforms

// ── Inner-border fill (matches LWC fillRectInnerBorder) ──────────────────────

fn push_inner_border(
    rects: &mut Vec<ColoredRect>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bw: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) {
    // top edge
    rects.push(ColoredRect {
        x: x + bw,
        y,
        w: w - bw * 2.0,
        h: bw,
        r,
        g,
        b,
        a,
    });
    // bottom edge
    rects.push(ColoredRect {
        x: x + bw,
        y: y + h - bw,
        w: w - bw * 2.0,
        h: bw,
        r,
        g,
        b,
        a,
    });
    // left edge
    rects.push(ColoredRect {
        x,
        y,
        w: bw,
        h,
        r,
        g,
        b,
        a,
    });
    // right edge
    rects.push(ColoredRect {
        x: x + w - bw,
        y,
        w: bw,
        h,
        r,
        g,
        b,
        a,
    });
}

// ── Candle generation (3-pass LWC order: wicks → borders → body fill) ────────

/// Project one visible candle to discrete physical-pixel geometry.
/// Values are in physical pixels, using LWC-style inclusive body/wick ends.
#[derive(Debug, Clone, Copy)]
pub struct ProjectedCandle {
    pub bar_left: f64,
    pub bar_width: f64,
    pub wick_left: f64,
    pub wick_width: f64,
    pub body_top: f64,
    pub body_bottom: f64,
    pub high_y: f64,
    pub low_y: f64,
    pub bull: bool,
}

/// Shared candle projection for all backends.
/// This centralizes the bar/wick overlap clamp and pixel rounding policy.
pub fn project_candles(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
) -> Vec<ProjectedCandle> {
    let start = (vp.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end {
        return Vec::new();
    }

    let half_bar = (sizing.bar_width * 0.5).floor();
    let wick_offset = (sizing.wick_width * 0.5).floor();
    let mut prev_wick_right: Option<f64> = None;
    let mut prev_bar_right: Option<f64> = None;

    let mut projected = Vec::with_capacity(end - start);
    for i in start..end {
        // SAFETY: i is bounded by start..end which are clamped to bars.len()
        let b = bars.get_unchecked(i);
        let bull = b.close >= b.open;

        let center_x = bar_to_x(i as f64 + 0.5, vp, chart_w).round();
        let body_top = price_to_y(b.open.max(b.close) as f64, vp, candle_h).round();
        let body_bottom = price_to_y(b.open.min(b.close) as f64, vp, candle_h).round();
        let high_y = price_to_y(b.high as f64, vp, candle_h).round();
        let low_y = price_to_y(b.low as f64, vp, candle_h).round();

        // Wick X edges with anti-overlap clamp.
        let mut wick_left = center_x - wick_offset;
        let wick_right = wick_left + sizing.wick_width - 1.0;
        if let Some(prev) = prev_wick_right {
            wick_left = wick_left.max(prev + 1.0).min(wick_right);
        }
        let wick_width = (wick_right - wick_left + 1.0).max(1.0);
        prev_wick_right = Some(wick_right);

        // Body X edges with anti-overlap clamp.
        let mut bar_left = center_x - half_bar;
        let bar_right = bar_left + sizing.bar_width - 1.0;
        if let Some(prev) = prev_bar_right {
            bar_left = bar_left.max(prev + 1.0).min(bar_right);
        }
        let bar_width = (bar_right - bar_left + 1.0).max(1.0);
        prev_bar_right = Some(bar_right);

        projected.push(ProjectedCandle {
            bar_left,
            bar_width,
            wick_left,
            wick_width,
            body_top,
            body_bottom,
            high_y,
            low_y,
            bull,
        });
    }

    projected
}

fn generate_candles_into(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let projected = project_candles(bars, vp, chart_w, candle_h, sizing);
    if projected.is_empty() {
        return;
    }

    // ── Pass 1: Wicks ────────────────────────────────────────────────────
    for c in &projected {
        let (wr, wg, wb, wa) = if c.bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        if c.body_top > c.high_y {
            rects.push(ColoredRect {
                x: c.wick_left as f32,
                y: c.high_y as f32,
                w: c.wick_width as f32,
                h: (c.body_top - c.high_y) as f32,
                r: wr,
                g: wg,
                b: wb,
                a: wa,
            });
        }
        // LWC parity: draw 1px lower wick when low == body_bottom + 1.
        if c.low_y > c.body_bottom {
            rects.push(ColoredRect {
                x: c.wick_left as f32,
                y: (c.body_bottom + 1.0) as f32,
                w: c.wick_width as f32,
                h: (c.low_y - c.body_bottom) as f32,
                r: wr,
                g: wg,
                b: wb,
                a: wa,
            });
        }
    }

    // ── Pass 2: Borders ──────────────────────────────────────────────────
    for c in &projected {
        let (br, bg, bb, ba) = if c.bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        let h = (c.body_bottom - c.body_top + 1.0).max(1.0);
        if sizing.bar_spacing * sizing.h_pixel_ratio > 2.0 * sizing.border_width {
            push_inner_border(
                rects,
                c.bar_left as f32,
                c.body_top as f32,
                c.bar_width as f32,
                h as f32,
                sizing.border_width as f32,
                br,
                bg,
                bb,
                ba,
            );
        } else {
            rects.push(ColoredRect {
                x: c.bar_left as f32,
                y: c.body_top as f32,
                w: c.bar_width as f32,
                h: h as f32,
                r: br,
                g: bg,
                b: bb,
                a: ba,
            });
        }
    }

    // ── Pass 3: Body fill ────────────────────────────────────────────────
    if sizing.draw_body {
        for c in &projected {
            let (cr, cg, cb, ca) = if c.bull {
                color4(&style.bullish_color)
            } else {
                color4(&style.bearish_color)
            };

            let right = c.bar_left + c.bar_width - 1.0;
            let bl = c.bar_left + sizing.border_width;
            let bt = c.body_top + sizing.border_width;
            let br_x = right - sizing.border_width;
            let bb_y = c.body_bottom - sizing.border_width;

            if bt > bb_y {
                continue;
            }

            rects.push(ColoredRect {
                x: bl as f32,
                y: bt as f32,
                w: (br_x - bl + 1.0) as f32,
                h: (bb_y - bt + 1.0) as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }
    }
}

// ── Volume generation ────────────────────────────────────────────────────────

fn generate_volume_into(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    vol_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let start = (vp.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end {
        return;
    }

    let mut max_vol = 0.0f32;
    for i in start..end {
        max_vol = max_vol.max(bars.volume(i));
    }
    if max_vol <= 0.0 {
        return;
    }

    let half_bar = (sizing.bar_width * 0.5).floor();

    for i in start..end {
        // SAFETY: i is bounded by start..end which are clamped to bars.len()
        let b = bars.get_unchecked(i);
        let (cr, cg, cb, ca) = if b.is_bullish() {
            color4(&style.bullish_volume_color)
        } else {
            color4(&style.bearish_volume_color)
        };

        let cx = bar_to_x(i as f64 + 0.5, vp, chart_w);
        let h = (b.volume as f64 / max_vol as f64) * vol_h;
        let top = candle_h + vol_h - h;

        let phys_x = cx.round();
        let left = phys_x - half_bar;

        rects.push(ColoredRect {
            x: left as f32,
            y: top.floor() as f32,
            w: sizing.bar_width as f32,
            h: h.ceil() as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OHLC Bars Generation (Traditional bar chart with ticks)
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate OHLC bar rects (vertical line + open/close ticks).
pub fn generate_ohlc_bar_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, h_ratio, v_ratio);
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len() * 3);
    generate_ohlc_bars_into(bars, viewport, style, pane_w, candle_h, &sizing, &mut rects);
    rects
}

fn generate_ohlc_bars_into(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let start = (vp.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end {
        return;
    }

    let wick_offset = (sizing.wick_width * 0.5).floor();
    let tick_width = (sizing.bar_width * 0.4).max(2.0).floor();

    for i in start..end {
        let b = bars.get_unchecked(i);
        let bull = b.close >= b.open;
        let (cr, cg, cb, ca) = if bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        let phys_x = bar_to_x(i as f64 + 0.5, vp, chart_w).round();
        let high_y = price_to_y(b.high as f64, vp, candle_h).round();
        let low_y = price_to_y(b.low as f64, vp, candle_h).round();
        let open_y = price_to_y(b.open as f64, vp, candle_h).round();
        let close_y = price_to_y(b.close as f64, vp, candle_h).round();

        // Vertical line (high to low)
        let left = phys_x - wick_offset;
        rects.push(ColoredRect {
            x: left as f32,
            y: high_y as f32,
            w: sizing.wick_width as f32,
            h: (low_y - high_y).max(1.0) as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });

        // Open tick (left side)
        rects.push(ColoredRect {
            x: (phys_x - tick_width) as f32,
            y: (open_y - sizing.wick_width * 0.5).round() as f32,
            w: tick_width as f32,
            h: sizing.wick_width as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });

        // Close tick (right side)
        rects.push(ColoredRect {
            x: phys_x as f32,
            y: (close_y - sizing.wick_width * 0.5).round() as f32,
            w: tick_width as f32,
            h: sizing.wick_width as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Line Chart Generation
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate line chart rects (line segments connecting close prices).
/// Line is approximated as thin rectangles for Canvas2D/rect rendering.
pub fn generate_line_chart_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    line_color: [f32; 4],
    line_width: f32,
    pane_w: f64,
    pane_h: f64,
) -> Vec<ColoredRect> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len());
    generate_line_into(
        bars, viewport, line_color, line_width, pane_w, candle_h, &mut rects,
    );
    rects
}

fn generate_line_into(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    color: [f32; 4],
    line_width: f32,
    chart_w: f64,
    candle_h: f64,
    rects: &mut Vec<ColoredRect>,
) {
    let start = (vp.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end || end - start < 2 {
        return;
    }

    let [cr, cg, cb, ca] = color;
    let half_width = (line_width * 0.5) as f64;

    // For each segment, generate multiple small horizontal rects to approximate the line
    // This creates a smooth diagonal line using rect primitives
    for i in start..(end - 1) {
        let b1 = bars.get_unchecked(i);
        let b2 = bars.get_unchecked(i + 1);

        let x1 = bar_to_x(i as f64 + 0.5, vp, chart_w);
        let x2 = bar_to_x((i + 1) as f64 + 0.5, vp, chart_w);
        let y1 = price_to_y(b1.close as f64, vp, candle_h);
        let y2 = price_to_y(b2.close as f64, vp, candle_h);

        let dx = x2 - x1;
        let dy = y2 - y1;
        let length = (dx * dx + dy * dy).sqrt();

        if length < 0.5 {
            continue;
        }

        // Subdivide the line segment into small steps for smoother rendering
        // Each step is a small horizontal rect at the interpolated y position
        let steps = ((dx.abs() / 2.0).max(1.0) as usize).min(50);
        let step_width = dx / steps as f64;

        for s in 0..steps {
            let t = s as f64 / steps as f64;
            let sx = x1 + dx * t;
            let sy = y1 + dy * t;

            rects.push(ColoredRect {
                x: sx as f32,
                y: (sy - half_width) as f32,
                w: (step_width.abs() + 1.0) as f32,
                h: line_width,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Area Chart Generation
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate area chart rects (filled area below the close line).
pub fn generate_area_chart_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    fill_color: [f32; 4],
    line_color: [f32; 4],
    line_width: f32,
    pane_w: f64,
    pane_h: f64,
) -> Vec<ColoredRect> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len() * 2);

    // Generate fill area first (underneath)
    generate_area_fill_into(bars, viewport, fill_color, pane_w, candle_h, &mut rects);

    // Then generate line on top
    generate_line_into(
        bars, viewport, line_color, line_width, pane_w, candle_h, &mut rects,
    );

    rects
}

fn generate_area_fill_into(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    color: [f32; 4],
    chart_w: f64,
    candle_h: f64,
    rects: &mut Vec<ColoredRect>,
) {
    let start = (vp.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end {
        return;
    }

    let [cr, cg, cb, ca] = color;

    // For smooth area fill, draw vertical columns for each pair of adjacent bars
    // interpolating the top edge to create a smooth fill below the line
    for i in start..end {
        let b = bars.get_unchecked(i);
        let x1 = bar_to_x(i as f64, vp, chart_w);
        let x2 = bar_to_x((i + 1) as f64, vp, chart_w);
        let y = price_to_y(b.close as f64, vp, candle_h);

        // Get next bar's close for interpolation (or use current if at end)
        let next_y = if i + 1 < bars.len() {
            let b2 = bars.get_unchecked(i + 1);
            price_to_y(b2.close as f64, vp, candle_h)
        } else {
            y
        };

        let col_width = x2 - x1;
        if col_width <= 0.0 {
            continue;
        }

        // Subdivide column for smooth diagonal top edge
        let steps = ((col_width / 3.0).max(1.0) as usize).min(20);
        let step_w = col_width / steps as f64;

        for s in 0..steps {
            let t = s as f64 / steps as f64;
            let sx = x1 + col_width * t;
            let sy = y + (next_y - y) * t; // interpolate y between current and next close
            let height = candle_h - sy;

            if height > 0.0 {
                rects.push(ColoredRect {
                    x: sx as f32,
                    y: sy as f32,
                    w: (step_w + 0.5) as f32, // slight overlap to avoid gaps
                    h: height as f32,
                    r: cr,
                    g: cg,
                    b: cb,
                    a: ca,
                });
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Line Segment Generation (for GPU line pipeline)
// ═══════════════════════════════════════════════════════════════════════════════

use crate::core::renderer::draw_list::{AreaSegment, LineSegment};

fn generate_main_close_points(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    pane_w: f64,
    candle_h: f64,
) -> Vec<(f64, f64)> {
    let start = (viewport.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(end - start);
    for i in start..end {
        let b = bars.get_unchecked(i);
        let x = bar_to_x(i as f64 + 0.5, viewport, pane_w).round();
        let y = price_to_y(b.close as f64, viewport, candle_h).round();
        points.push((x, y));
    }
    points
}

#[inline]
fn strip_bounds(points: &[(f64, f64)], i: usize) -> Option<(f64, f64)> {
    let x = points[i].0;
    let left = if i == 0 {
        x
    } else {
        ((points[i - 1].0 + x) * 0.5).round()
    };
    let right = if i + 1 == points.len() {
        x
    } else {
        ((x + points[i + 1].0) * 0.5).round()
    };
    if right <= left {
        None
    } else {
        Some((left, right))
    }
}

#[inline]
fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Generate line segments for the GPU line pipeline (smooth anti-aliased lines).
pub fn generate_line_segments(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    line_color: [f32; 4],
    line_width: f32,
    pane_w: f64,
    pane_h: f64,
) -> Vec<LineSegment> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let start = (viewport.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());

    if end <= start || end - start < 2 {
        return Vec::new();
    }

    let [r, g, b, a] = line_color;
    let mut segments = Vec::with_capacity(end - start);

    for i in start..(end - 1) {
        let b1 = bars.get_unchecked(i);
        let b2 = bars.get_unchecked(i + 1);

        let x1 = bar_to_x(i as f64 + 0.5, viewport, pane_w) as f32;
        let y1 = price_to_y(b1.close as f64, viewport, candle_h) as f32;
        let x2 = bar_to_x((i + 1) as f64 + 0.5, viewport, pane_w) as f32;
        let y2 = price_to_y(b2.close as f64, viewport, candle_h) as f32;

        segments.push(LineSegment {
            x1,
            y1,
            x2,
            y2,
            width: line_width,
            r,
            g,
            b,
            a,
            _pad: 0.0,
        });
    }

    segments
}

/// Generate gradient fill rects for the main area chart.
pub fn generate_main_area_fill_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    top_color: [f32; 4],
    bottom_color: [f32; 4],
    pane_w: f64,
    pane_h: f64,
) -> Vec<ColoredRect> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let base_y = candle_h.round();
    let points = generate_main_close_points(bars, viewport, pane_w, candle_h);
    if points.is_empty() {
        return Vec::new();
    }

    let num_bands = 8usize;
    let mut rects = Vec::with_capacity(points.len() * num_bands);
    for i in 0..points.len() {
        let (_, y0) = points[i];
        let (left, right) = match strip_bounds(&points, i) {
            Some(bounds) => bounds,
            None => continue,
        };
        let strip_w = right - left;
        if strip_w <= 0.0 {
            continue;
        }

        let fill_top = y0.min(base_y);
        let fill_bottom = y0.max(base_y);
        let fill_h = fill_bottom - fill_top;
        if fill_h <= 0.5 {
            continue;
        }

        let (from, to) = if y0 <= base_y {
            (top_color, bottom_color)
        } else {
            (bottom_color, top_color)
        };

        let band_h = fill_h / num_bands as f64;
        for band in 0..num_bands {
            let band_top = fill_top + band_h * band as f64;
            let band_bottom = if band + 1 == num_bands {
                fill_bottom
            } else {
                fill_top + band_h * (band + 1) as f64
            };
            let h = band_bottom - band_top;
            if h <= 0.0 {
                continue;
            }
            let mid = band_top + h * 0.5;
            let t = ((mid - fill_top) / fill_h).clamp(0.0, 1.0) as f32;
            let [r, g, b, a] = lerp_color(from, to, t);
            rects.push(ColoredRect {
                x: left as f32,
                y: band_top as f32,
                w: strip_w as f32,
                h: h as f32,
                r,
                g,
                b,
                a,
            });
        }
    }
    rects
}

/// Generate two-tone fill rects for the main baseline chart.
pub fn generate_main_baseline_fill_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    baseline_value: f32,
    top_fill_color: [f32; 4],
    bottom_fill_color: [f32; 4],
    pane_w: f64,
    pane_h: f64,
) -> Vec<ColoredRect> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let base_y = price_to_y(baseline_value as f64, viewport, candle_h).round();
    let points = generate_main_close_points(bars, viewport, pane_w, candle_h);
    if points.is_empty() {
        return Vec::new();
    }

    let mut rects = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let (_, y0) = points[i];
        let (left, right) = match strip_bounds(&points, i) {
            Some(bounds) => bounds,
            None => continue,
        };
        let strip_w = right - left;
        if strip_w <= 0.0 {
            continue;
        }

        if y0 < base_y {
            let h = base_y - y0;
            if h > 0.5 {
                rects.push(ColoredRect {
                    x: left as f32,
                    y: y0 as f32,
                    w: strip_w as f32,
                    h: h as f32,
                    r: top_fill_color[0],
                    g: top_fill_color[1],
                    b: top_fill_color[2],
                    a: top_fill_color[3],
                });
            }
        } else if y0 > base_y {
            let h = y0 - base_y;
            if h > 0.5 {
                rects.push(ColoredRect {
                    x: left as f32,
                    y: base_y as f32,
                    w: strip_w as f32,
                    h: h as f32,
                    r: bottom_fill_color[0],
                    g: bottom_fill_color[1],
                    b: bottom_fill_color[2],
                    a: bottom_fill_color[3],
                });
            }
        }
    }
    rects
}

/// Generate two-tone line segments for the main baseline chart.
pub fn generate_main_baseline_line_segments(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    baseline_value: f32,
    top_line_color: [f32; 4],
    bottom_line_color: [f32; 4],
    line_width: f32,
    pane_w: f64,
    pane_h: f64,
    v_ratio: f64,
) -> Vec<LineSegment> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let base_y = price_to_y(baseline_value as f64, viewport, candle_h).round();
    let points = generate_main_close_points(bars, viewport, pane_w, candle_h);
    if points.len() < 2 {
        return Vec::new();
    }

    let width = (line_width * v_ratio as f32).round().max(1.0);
    let correction = if (width as i32) % 2 == 1 { 0.5 } else { 0.0 };

    let mut segments = Vec::with_capacity(points.len() - 1);
    for i in 0..(points.len() - 1) {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];
        emit_split_segment_by_baseline(
            x1 + correction,
            y1 + correction,
            x2 + correction,
            y2 + correction,
            base_y + correction,
            top_line_color,
            bottom_line_color,
            |sx1, sy1, sx2, sy2, color| {
                segments.push(LineSegment {
                    x1: sx1 as f32,
                    y1: sy1 as f32,
                    x2: sx2 as f32,
                    y2: sy2 as f32,
                    width,
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

/// Generate area segments (trapezoids) for smooth area chart fills.
pub fn generate_area_segments(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    fill_color: [f32; 4],
    pane_w: f64,
    pane_h: f64,
) -> Vec<AreaSegment> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let start = (viewport.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());

    if end <= start || end - start < 2 {
        return Vec::new();
    }

    let [r, g, b, a] = fill_color;
    let bottom = candle_h as f32;
    let mut segments = Vec::with_capacity(end - start);

    for i in start..(end - 1) {
        let b1 = bars.get_unchecked(i);
        let b2 = bars.get_unchecked(i + 1);

        let x1 = bar_to_x(i as f64 + 0.5, viewport, pane_w) as f32;
        let y1 = price_to_y(b1.close as f64, viewport, candle_h) as f32;
        let x2 = bar_to_x((i + 1) as f64 + 0.5, viewport, pane_w) as f32;
        let y2 = price_to_y(b2.close as f64, viewport, candle_h) as f32;

        segments.push(AreaSegment {
            x1,
            y1,
            x2,
            y2,
            bottom,
            r,
            g,
            b,
            a,
            _pad: 0.0,
        });
    }

    segments
}

/// Generate area fill rects + line segments for the GPU (smooth area chart).
/// Returns (fill_rects, line_segments).
pub fn generate_area_for_gpu(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    fill_color: [f32; 4],
    line_color: [f32; 4],
    line_width: f32,
    pane_w: f64,
    pane_h: f64,
) -> (Vec<ColoredRect>, Vec<LineSegment>) {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let start = (viewport.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());

    if end <= start {
        return (Vec::new(), Vec::new());
    }

    let [fr, fg, fb, fa] = fill_color;
    let [lr, lg, lb, la] = line_color;

    // Generate area fill as vertical columns
    let mut rects = Vec::with_capacity(end - start);
    for i in start..end {
        let b = bars.get_unchecked(i);
        let x1 = bar_to_x(i as f64, viewport, pane_w);
        let x2 = bar_to_x((i + 1) as f64, viewport, pane_w);
        let y = price_to_y(b.close as f64, viewport, candle_h);
        let height = candle_h - y;

        if height > 0.0 && x2 > x1 {
            rects.push(ColoredRect {
                x: x1 as f32,
                y: y as f32,
                w: (x2 - x1).max(1.0) as f32,
                h: height as f32,
                r: fr,
                g: fg,
                b: fb,
                a: fa,
            });
        }
    }

    // Generate line segments for the top edge
    let mut segments = Vec::with_capacity(end - start);
    if end - start >= 2 {
        for i in start..(end - 1) {
            let b1 = bars.get_unchecked(i);
            let b2 = bars.get_unchecked(i + 1);

            let x1 = bar_to_x(i as f64 + 0.5, viewport, pane_w) as f32;
            let y1 = price_to_y(b1.close as f64, viewport, candle_h) as f32;
            let x2 = bar_to_x((i + 1) as f64 + 0.5, viewport, pane_w) as f32;
            let y2 = price_to_y(b2.close as f64, viewport, candle_h) as f32;

            segments.push(LineSegment {
                x1,
                y1,
                x2,
                y2,
                width: line_width,
                r: lr,
                g: lg,
                b: lb,
                a: la,
                _pad: 0.0,
            });
        }
    }

    (rects, segments)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Heikin-Ashi Generation
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate Heikin-Ashi candle rects.
/// Transforms OHLC data using Heikin-Ashi formula, then renders as candlesticks.
pub fn generate_heikin_ashi_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, h_ratio, v_ratio);
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len() * 6);
    generate_heikin_ashi_into(bars, viewport, style, pane_w, candle_h, &sizing, &mut rects);
    rects
}

fn generate_heikin_ashi_into(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let start = (vp.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end {
        return;
    }

    let half_bar = (sizing.bar_width * 0.5).floor();
    let wick_offset = (sizing.wick_width * 0.5).floor();

    // Compute Heikin-Ashi values
    // HA_Close = (O + H + L + C) / 4
    // HA_Open = (prev_HA_Open + prev_HA_Close) / 2
    // HA_High = max(H, HA_Open, HA_Close)
    // HA_Low = min(L, HA_Open, HA_Close)

    let mut prev_ha_open = 0.0f32;
    let mut prev_ha_close = 0.0f32;

    // Initialize with first bar
    if start < bars.len() {
        let first = bars.get_unchecked(start);
        prev_ha_open = (first.open + first.close) / 2.0;
        prev_ha_close = (first.open + first.high + first.low + first.close) / 4.0;
    }

    for i in start..end {
        let b = bars.get_unchecked(i);

        // Compute Heikin-Ashi values
        let ha_close = (b.open + b.high + b.low + b.close) / 4.0;
        let ha_open = (prev_ha_open + prev_ha_close) / 2.0;
        let ha_high = b.high.max(ha_open).max(ha_close);
        let ha_low = b.low.min(ha_open).min(ha_close);

        let bull = ha_close >= ha_open;
        let (wr, wg, wb, wa) = if bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };
        let (cr, cg, cb, ca) = if bull {
            color4(&style.bullish_color)
        } else {
            color4(&style.bearish_color)
        };

        let phys_x = bar_to_x(i as f64 + 0.5, vp, chart_w).round();
        let body_top = price_to_y(ha_open.max(ha_close) as f64, vp, candle_h).round();
        let body_bottom = price_to_y(ha_open.min(ha_close) as f64, vp, candle_h).round();
        let high_y = price_to_y(ha_high as f64, vp, candle_h).round();
        let low_y = price_to_y(ha_low as f64, vp, candle_h).round();

        // Wick (high to body top)
        if body_top > high_y {
            rects.push(ColoredRect {
                x: (phys_x - wick_offset) as f32,
                y: high_y as f32,
                w: sizing.wick_width as f32,
                h: (body_top - high_y) as f32,
                r: wr,
                g: wg,
                b: wb,
                a: wa,
            });
        }

        // Wick (body bottom to low)
        if low_y > body_bottom {
            rects.push(ColoredRect {
                x: (phys_x - wick_offset) as f32,
                y: (body_bottom + 1.0) as f32,
                w: sizing.wick_width as f32,
                h: (low_y - body_bottom) as f32,
                r: wr,
                g: wg,
                b: wb,
                a: wa,
            });
        }

        // Body
        let left = phys_x - half_bar;
        let w = sizing.bar_width;
        let h = (body_bottom - body_top + 1.0).max(1.0);

        rects.push(ColoredRect {
            x: left as f32,
            y: body_top as f32,
            w: w as f32,
            h: h as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });

        prev_ha_open = ha_open;
        prev_ha_close = ha_close;
    }
}
