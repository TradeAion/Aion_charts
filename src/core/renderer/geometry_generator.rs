//! GeometryGenerator — single source of truth for ALL visual math.
//!
//! Takes Viewport, data, style → produces pixel-perfect ColoredRects.
//! The Canvas2D renderer consumes these directly.
//!
//! Public API is split per element type so each ChartRenderer phase can
//! request only the geometry it needs:
//! - `generate_grid_rects`   — background grid lines
//! - `generate_candle_rects` — wicks + borders + body fills
//! - `generate_volume_rects` — volume bars
//! - `generate`              — legacy all-in-one (background + grid + volume + candles)
//!
//! All candle sizing uses reference-matching algorithms from series.rs.
//! Tick computation is in tick_marks.rs (shared with axis renderers).

use crate::core::renderer::draw_list::{ColoredRect, DrawList};
use crate::core::renderer::series::CandleSizing;
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::transforms::{bar_to_x, color4, price_to_y};
use crate::core::renderer::value_projection::TimeScaleIndex;
use crate::core::viewport::Viewport;

/// Keep wick thickness stable across high-DPI/mobile layouts.
/// Returned value is in physical pixels and must stay integer-aligned because
/// candle geometry is rendered as filled rects by every backend.
#[inline]
fn effective_wick_width(sizing: &CandleSizing) -> f64 {
    let ratio = sizing.h_pixel_ratio.max(1.0);
    let ratio_floor = ratio.floor().max(1.0);
    let min_visible_phys = if ratio - ratio_floor <= 0.05 {
        ratio_floor
    } else {
        ratio.ceil()
    };
    sizing
        .wick_width
        .round()
        .max(min_visible_phys)
        .min(sizing.bar_width.max(1.0))
        .max(1.0)
}

#[inline]
fn visible_main_bar_range(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    time_scale: &TimeScaleIndex,
) -> Option<(usize, usize)> {
    if bars.is_empty() {
        return None;
    }
    time_scale.visible_main_bar_range(vp.start_bar - 1.0, vp.end_bar + 1.0)
}

#[inline]
fn main_bar_center_x(
    bar_index: usize,
    vp: &Viewport,
    time_scale: &TimeScaleIndex,
    chart_w: f64,
) -> Option<f64> {
    time_scale
        .logical_index_for_main_bar(bar_index)
        .map(|logical_index| bar_to_x(logical_index + 0.5, vp, chart_w))
}

#[inline]
fn main_bar_center_x_phys(
    bar_index: usize,
    vp: &Viewport,
    time_scale: &TimeScaleIndex,
    chart_w: f64,
    h_pixel_ratio: f64,
) -> Option<f64> {
    let ratio = h_pixel_ratio.max(1.0);
    time_scale
        .logical_index_for_main_bar(bar_index)
        .map(|logical_index| {
            let frac = (logical_index + 0.5 - vp.start_bar) / (vp.end_bar - vp.start_bar);
            frac * chart_w - ratio
        })
}

#[inline]
fn main_bar_slot_x(
    bar_index: usize,
    vp: &Viewport,
    time_scale: &TimeScaleIndex,
    chart_w: f64,
) -> Option<f64> {
    time_scale
        .logical_index_for_main_bar(bar_index)
        .map(|logical_index| bar_to_x(logical_index, vp, chart_w))
}

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
    let time_scale = TimeScaleIndex::from_bars(bars);

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
        &time_scale,
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
        &time_scale,
        viewport,
        style,
        style.wick_bullish_color,
        style.wick_bearish_color,
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
/// All renderers (Canvas2D, subpanes) should use this function.
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
    time_scale: &TimeScaleIndex,
    viewport: &Viewport,
    style: &ChartStyle,
    bullish_border_color: [f32; 4],
    bearish_border_color: [f32; 4],
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> Vec<ColoredRect> {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, h_ratio, v_ratio);
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len() * 6);
    generate_candles_into(
        bars,
        time_scale,
        viewport,
        style,
        bullish_border_color,
        bearish_border_color,
        pane_w,
        candle_h,
        &sizing,
        &mut rects,
    );
    rects
}

/// Generate volume bar rects.
pub fn generate_volume_rects(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
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
        bars, time_scale, viewport, style, pane_w, candle_h, vol_h, &sizing, &mut rects,
    );
    rects
}

// ── Coordinate helpers imported from transforms.rs ───────────────────────────
// bar_to_x, price_to_y, and color4 are now in crate::core::renderer::transforms

// ── Inner-border fill (matches reference implementation fillRectInnerBorder) ──────────────────────

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

// ── Candle generation (3-pass reference implementation order: wicks → borders → body fill) ────────

/// Project one visible candle to discrete physical-pixel geometry.
/// Values are in physical pixels, using compatibility-style inclusive body/wick ends.
#[derive(Debug, Clone, Copy)]
pub struct ProjectedCandle {
    pub body_left: f64,
    pub body_width: f64,
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
/// This centralizes pixel rounding policy for bar/wick/body footprints.
pub fn project_candles(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    vp: &Viewport,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
) -> Vec<ProjectedCandle> {
    let Some((start, end)) = visible_main_bar_range(bars, vp, time_scale) else {
        return Vec::new();
    };
    if start >= end {
        return Vec::new();
    }

    let half_bar = (sizing.bar_width * 0.5).floor();

    let mut projected = Vec::with_capacity(end - start);
    for i in start..end {
        // SAFETY: i is bounded by start..end which are clamped to bars.len()
        let b = bars.get_unchecked(i);
        let bull = b.close >= b.open;

        let Some(center_x) =
            main_bar_center_x_phys(i, vp, time_scale, chart_w, sizing.h_pixel_ratio)
                .map(f64::round)
        else {
            continue;
        };
        let body_price_high = b.open.max(b.close);
        let body_price_low = b.open.min(b.close);
        let body_top = price_to_y(body_price_high as f64, vp, candle_h).round();
        let body_bottom = price_to_y(body_price_low as f64, vp, candle_h).round();
        let mut high_y = price_to_y(b.high as f64, vp, candle_h).round();
        let mut low_y = price_to_y(b.low as f64, vp, candle_h).round();

        // Rounding can collapse a real wick to the body edge at some scroll /
        // autoscale positions. Keep data-present wicks visible by at least one
        // physical pixel so they do not flicker in and out while panning.
        if b.high > body_price_high && high_y >= body_top {
            high_y = body_top - 1.0;
        }
        if b.low < body_price_low && low_y <= body_bottom {
            low_y = body_bottom + 1.0;
        }

        // Keep border/body footprints width-stable per frame. Clamping adjacent
        // bars to avoid overlap creates position-dependent apparent thickness,
        // which is exactly the blur/thickness jitter users notice when panning.
        let ideal_bar_left = center_x - half_bar;
        let bar_left = ideal_bar_left;
        let bar_width = sizing.bar_width.max(1.0);

        let body_left = ideal_bar_left;
        let body_width = sizing.bar_width.max(1.0);

        // Keep wick width/centering stable candle-to-candle. Clamping wick
        // width to overlap-reduced bar footprints makes adjacent candles look
        // uneven on some viewport sizes.
        let wick_width = effective_wick_width(sizing).max(1.0);
        let wick_left = center_x - (wick_width * 0.5).floor();

        projected.push(ProjectedCandle {
            body_left,
            body_width,
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
    time_scale: &TimeScaleIndex,
    vp: &Viewport,
    style: &ChartStyle,
    bullish_border_color: [f32; 4],
    bearish_border_color: [f32; 4],
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let projected = project_candles(bars, time_scale, vp, chart_w, candle_h, sizing);
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
        // reference parity: draw 1px lower wick when low == body_bottom + 1.
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
            color4(&bullish_border_color)
        } else {
            color4(&bearish_border_color)
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

            let right = c.body_left + c.body_width - 1.0;
            let bl = c.body_left + sizing.border_width;
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
    time_scale: &TimeScaleIndex,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    vol_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let Some((start, end)) = visible_main_bar_range(bars, vp, time_scale) else {
        return;
    };
    if start >= end {
        return;
    }

    let mut max_vol = 0.0f64;
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

        let Some(cx) = main_bar_center_x_phys(i, vp, time_scale, chart_w, sizing.h_pixel_ratio)
        else {
            continue;
        };
        let h = (b.volume / max_vol) * vol_h;
        let top = candle_h + vol_h - h;

        // Match candlestick X anchoring exactly so volume bars stay aligned
        // across backends (WebGPU + Canvas2D) and chart types.
        let slot_center = cx.round();
        let left = (slot_center - half_bar).round();
        let right = left + sizing.bar_width - 1.0;
        let width = (right - left + 1.0).max(1.0);

        rects.push(ColoredRect {
            x: left as f32,
            y: top.floor() as f32,
            w: width as f32,
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
    time_scale: &TimeScaleIndex,
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
    generate_ohlc_bars_into(
        bars, time_scale, viewport, style, pane_w, candle_h, &sizing, &mut rects,
    );
    rects
}

fn generate_ohlc_bars_into(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let Some((start, end)) = visible_main_bar_range(bars, vp, time_scale) else {
        return;
    };
    if start >= end {
        return;
    }

    let wick_w = effective_wick_width(sizing);
    let wick_offset = (wick_w * 0.5).floor();
    let tick_width = (sizing.bar_width * 0.4).max(2.0).floor();

    for i in start..end {
        let b = bars.get_unchecked(i);
        let bull = b.close >= b.open;
        let (cr, cg, cb, ca) = if bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        let Some(phys_x) = main_bar_center_x_phys(i, vp, time_scale, chart_w, sizing.h_pixel_ratio)
            .map(f64::round)
        else {
            continue;
        };
        let high_y = price_to_y(b.high as f64, vp, candle_h).round();
        let low_y = price_to_y(b.low as f64, vp, candle_h).round();
        let open_y = price_to_y(b.open as f64, vp, candle_h).round();
        let close_y = price_to_y(b.close as f64, vp, candle_h).round();

        // Vertical line (high to low)
        let left = phys_x - wick_offset;
        rects.push(ColoredRect {
            x: left as f32,
            y: high_y as f32,
            w: wick_w as f32,
            h: (low_y - high_y).max(1.0) as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });

        // Open tick (left side)
        rects.push(ColoredRect {
            x: (phys_x - tick_width) as f32,
            y: (open_y - wick_w * 0.5).round() as f32,
            w: tick_width as f32,
            h: wick_w as f32,
            r: cr,
            g: cg,
            b: cb,
            a: ca,
        });

        // Close tick (right side)
        rects.push(ColoredRect {
            x: phys_x as f32,
            y: (close_y - wick_w * 0.5).round() as f32,
            w: tick_width as f32,
            h: wick_w as f32,
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
    let time_scale = TimeScaleIndex::from_bars(bars);
    generate_line_into(
        bars,
        &time_scale,
        viewport,
        line_color,
        line_width,
        pane_w,
        candle_h,
        &mut rects,
    );
    rects
}

fn generate_line_into(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    vp: &Viewport,
    color: [f32; 4],
    line_width: f32,
    chart_w: f64,
    candle_h: f64,
    rects: &mut Vec<ColoredRect>,
) {
    let Some((start, end)) = visible_main_bar_range(bars, vp, time_scale) else {
        return;
    };
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

        let Some(x1) = main_bar_center_x(i, vp, time_scale, chart_w) else {
            continue;
        };
        let Some(x2) = main_bar_center_x(i + 1, vp, time_scale, chart_w) else {
            continue;
        };
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
    time_scale: &TimeScaleIndex,
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
    generate_area_fill_into(
        bars, time_scale, viewport, fill_color, pane_w, candle_h, &mut rects,
    );

    // Then generate line on top
    generate_line_into(
        bars, time_scale, viewport, line_color, line_width, pane_w, candle_h, &mut rects,
    );

    rects
}

fn generate_area_fill_into(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    vp: &Viewport,
    color: [f32; 4],
    chart_w: f64,
    candle_h: f64,
    rects: &mut Vec<ColoredRect>,
) {
    let Some((start, end)) = visible_main_bar_range(bars, vp, time_scale) else {
        return;
    };
    if start >= end {
        return;
    }

    let [cr, cg, cb, ca] = color;

    // For smooth area fill, draw vertical columns for each pair of adjacent bars
    // interpolating the top edge to create a smooth fill below the line
    for i in start..end {
        let b = bars.get_unchecked(i);
        let Some(x1) = main_bar_slot_x(i, vp, time_scale, chart_w) else {
            continue;
        };
        let x2 = if i + 1 < bars.len() {
            match main_bar_slot_x(i + 1, vp, time_scale, chart_w) {
                Some(value) => value,
                None => continue,
            }
        } else {
            x1 + (chart_w / (vp.end_bar - vp.start_bar)).max(0.0)
        };
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
// Line Segment Generation (for Canvas2D line rendering)
// ═══════════════════════════════════════════════════════════════════════════════

use crate::core::renderer::draw_list::{AreaSegment, LineSegment};

/// Generate main-series close points for area rendering with monotonic X.
/// This collapses duplicate X pixels (zoomed-out bars mapping to same column)
/// so area fills cannot self-overlap and produce dark patches.
fn generate_main_area_points(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    viewport: &Viewport,
    pane_w: f64,
    candle_h: f64,
) -> Vec<(f32, f32)> {
    let Some((start, end)) = visible_main_bar_range(bars, viewport, time_scale) else {
        return Vec::new();
    };
    if start >= end {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(end - start);
    let mut last_x_bucket: Option<i32> = None;
    for i in start..end {
        let b = bars.get_unchecked(i);
        let Some(x) = main_bar_center_x(i, viewport, time_scale, pane_w).map(|x| x as f32) else {
            continue;
        };
        let y = price_to_y(b.close as f64, viewport, candle_h) as f32;

        if !x.is_finite() || !y.is_finite() {
            continue;
        }

        // Deduplicate by pixel column, but keep subpixel coordinates for smooth strokes.
        let x_bucket = x.round() as i32;
        match last_x_bucket {
            None => {
                points.push((x, y));
                last_x_bucket = Some(x_bucket);
            }
            Some(prev_bucket) if x_bucket > prev_bucket => {
                points.push((x, y));
                last_x_bucket = Some(x_bucket);
            }
            Some(prev_bucket) if x_bucket == prev_bucket => {
                if let Some((last_x, last_y)) = points.last_mut() {
                    *last_x = x;
                    *last_y = y;
                }
            }
            Some(_) => continue,
        }
    }

    points
}

/// Generate line segments for the Canvas2D line rendering (smooth anti-aliased lines).
pub fn generate_line_segments(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    viewport: &Viewport,
    line_color: [f32; 4],
    line_width: f32,
    pane_w: f64,
    pane_h: f64,
) -> Vec<LineSegment> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let Some((start, end)) = visible_main_bar_range(bars, viewport, time_scale) else {
        return Vec::new();
    };

    if end <= start || end - start < 2 {
        return Vec::new();
    }

    let [r, g, b, a] = line_color;
    let mut segments = Vec::with_capacity(end - start);

    for i in start..(end - 1) {
        let b1 = bars.get_unchecked(i);
        let b2 = bars.get_unchecked(i + 1);

        let Some(x1) = main_bar_center_x(i, viewport, time_scale, pane_w).map(|x| x as f32) else {
            continue;
        };
        let y1 = price_to_y(b1.close as f64, viewport, candle_h) as f32;
        let Some(x2) = main_bar_center_x(i + 1, viewport, time_scale, pane_w).map(|x| x as f32)
        else {
            continue;
        };
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
            reserved: 0.0,
        });
    }

    segments
}

/// Generate area segments (trapezoids) for smooth area chart fills.
pub fn generate_area_segments(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    viewport: &Viewport,
    top_color: [f32; 4],
    bottom_color: [f32; 4],
    pane_w: f64,
    pane_h: f64,
) -> Vec<AreaSegment> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let points = generate_main_area_points(bars, time_scale, viewport, pane_w, candle_h);
    if points.len() < 2 {
        return Vec::new();
    }

    let mut gradient_top = points[0].1;
    for (_, y) in &points {
        gradient_top = gradient_top.min(*y);
    }
    let bottom = candle_h as f32;
    let mut segments = Vec::with_capacity(points.len() - 1);

    for i in 0..(points.len() - 1) {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];
        if x2 <= x1 {
            continue;
        }
        segments.push(AreaSegment {
            x1,
            y1,
            x2,
            y2,
            bottom,
            top_r: top_color[0],
            top_g: top_color[1],
            top_b: top_color[2],
            top_a: top_color[3],
            bottom_r: bottom_color[0],
            bottom_g: bottom_color[1],
            bottom_b: bottom_color[2],
            bottom_a: bottom_color[3],
            gradient_top,
            reserved1: 0.0,
            reserved2: 0.0,
        });
    }

    segments
}

/// Generate the area top line from the same monotonic points as area fill.
/// This keeps the line perfectly connected to the fill across zoom levels.
pub fn generate_main_area_line_segments(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    viewport: &Viewport,
    line_color: [f32; 4],
    line_width: f32,
    pane_w: f64,
    pane_h: f64,
) -> Vec<LineSegment> {
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let points = generate_main_area_points(bars, time_scale, viewport, pane_w, candle_h);
    if points.len() < 2 {
        return Vec::new();
    }

    let [r, g, b, a] = line_color;
    let mut segments = Vec::with_capacity(points.len() - 1);
    for i in 0..(points.len() - 1) {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];
        if x2 <= x1 {
            continue;
        }
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
            reserved: 0.0,
        });
    }
    segments
}

// ═══════════════════════════════════════════════════════════════════════════════
// Heikin-Ashi Generation
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate Heikin-Ashi candle rects.
/// Transforms OHLC data using Heikin-Ashi formula, then renders as candlesticks.
pub fn generate_heikin_ashi_rects(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
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
    generate_heikin_ashi_into(
        bars, time_scale, viewport, style, pane_w, candle_h, &sizing, &mut rects,
    );
    rects
}

fn generate_heikin_ashi_into(
    bars: &crate::core::data::BarArray,
    time_scale: &TimeScaleIndex,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let Some((start, end)) = visible_main_bar_range(bars, vp, time_scale) else {
        return;
    };
    if start >= end {
        return;
    }

    let half_bar = (sizing.bar_width * 0.5).floor();
    let wick_w = effective_wick_width(sizing);
    let wick_offset = (wick_w * 0.5).floor();

    // Compute Heikin-Ashi values
    // HA_Close = (O + H + L + C) / 4
    // HA_Open = (prev_HA_Open + prev_HA_Close) / 2
    // HA_High = max(H, HA_Open, HA_Close)
    // HA_Low = min(L, HA_Open, HA_Close)

    let mut prev_ha_open = 0.0f64;
    let mut prev_ha_close = 0.0f64;

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

        let Some(phys_x) = main_bar_center_x_phys(i, vp, time_scale, chart_w, sizing.h_pixel_ratio)
            .map(f64::round)
        else {
            continue;
        };
        let ha_body_high = ha_open.max(ha_close);
        let ha_body_low = ha_open.min(ha_close);
        let body_top = price_to_y(ha_body_high, vp, candle_h).round();
        let body_bottom = price_to_y(ha_body_low, vp, candle_h).round();
        let mut high_y = price_to_y(ha_high, vp, candle_h).round();
        let mut low_y = price_to_y(ha_low, vp, candle_h).round();
        if ha_high > ha_body_high && high_y >= body_top {
            high_y = body_top - 1.0;
        }
        if ha_low < ha_body_low && low_y <= body_bottom {
            low_y = body_bottom + 1.0;
        }

        // Wick (high to body top)
        if body_top > high_y {
            rects.push(ColoredRect {
                x: (phys_x - wick_offset) as f32,
                y: high_y as f32,
                w: wick_w as f32,
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
                w: wick_w as f32,
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

#[cfg(test)]
mod tests {
    use super::{effective_wick_width, project_candles};
    use crate::core::data::{Bar, BarArray};
    use crate::core::renderer::series::CandleSizing;
    use crate::core::renderer::value_projection::TimeScaleIndex;
    use crate::core::viewport::Viewport;

    fn sample_bars() -> BarArray {
        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 10.0,
                high: 11.0,
                low: 9.0,
                close: 10.5,
                volume: 100.0,
            },
            Bar {
                timestamp: 2,
                open: 10.5,
                high: 11.5,
                low: 10.0,
                close: 11.0,
                volume: 120.0,
            },
            Bar {
                timestamp: 3,
                open: 11.0,
                high: 11.4,
                low: 10.4,
                close: 10.8,
                volume: 90.0,
            },
            Bar {
                timestamp: 4,
                open: 10.8,
                high: 11.2,
                low: 10.2,
                close: 10.6,
                volume: 80.0,
            },
        ])
        .expect("sample bars");
        bars
    }

    #[test]
    fn wick_and_bar_widths_stay_constant_in_dense_views() {
        let bars = sample_bars();
        let time_scale = TimeScaleIndex::from_bars(&bars);
        let mut viewport = Viewport::new(8, 200);
        viewport.volume_height_ratio = 0.0;
        viewport.set_range(0.0, 3.1);
        viewport.price_min = 9.0;
        viewport.price_max = 12.0;

        let sizing = CandleSizing::compute_from_pane(8.0, &viewport, 1.0, 1.0);
        let projected = project_candles(&bars, &time_scale, &viewport, 8.0, 200.0, &sizing);

        assert_eq!(sizing.bar_width, 3.0);
        assert_eq!(projected.len(), 4);

        assert!(
            projected
                .iter()
                .all(|c| (c.bar_width - sizing.bar_width).abs() < f64::EPSILON),
            "bar border footprint should remain width-stable candle-to-candle"
        );
        assert!(
            projected
                .iter()
                .all(|c| (c.body_width - sizing.bar_width).abs() < f64::EPSILON),
            "body footprint should remain width-stable candle-to-candle"
        );
        assert!(
            projected
                .iter()
                .all(|c| (c.wick_width - effective_wick_width(&sizing)).abs() < f64::EPSILON),
            "wick width should remain width-stable candle-to-candle"
        );
    }

    #[test]
    fn dense_mobile_projection_keeps_uniform_widths() {
        let bars = sample_bars();
        let time_scale = TimeScaleIndex::from_bars(&bars);
        let mut viewport = Viewport::new(30, 200);
        viewport.volume_height_ratio = 0.0;
        viewport.set_range(0.0, 4.0);
        viewport.price_min = 9.0;
        viewport.price_max = 12.0;

        let sizing = CandleSizing::compute_from_pane(30.0, &viewport, 3.0, 3.0);
        let projected = project_candles(&bars, &time_scale, &viewport, 30.0, 200.0, &sizing);

        assert!(
            projected
                .iter()
                .all(|c| (c.bar_width - sizing.bar_width).abs() < f64::EPSILON),
            "dense mobile spacing should keep a uniform bar footprint width"
        );
        assert!(
            projected
                .iter()
                .all(|c| (c.body_width - sizing.bar_width).abs() < f64::EPSILON),
            "dense mobile spacing should keep a uniform body footprint width"
        );
    }

    #[test]
    fn candle_x_projection_applies_css_bias_in_physical_pixels() {
        let bars = sample_bars();
        let time_scale = TimeScaleIndex::from_bars(&bars);
        let mut viewport = Viewport::new(1000, 200);
        viewport.volume_height_ratio = 0.0;
        viewport.set_range(0.0, 100.0);
        viewport.price_min = 9.0;
        viewport.price_max = 12.0;

        let sizing = CandleSizing {
            bar_width: 3.0,
            wick_width: 1.0,
            border_width: 1.0,
            draw_body: true,
            bar_spacing: 5.0,
            h_pixel_ratio: 2.0,
            v_pixel_ratio: 2.0,
        };
        let projected = project_candles(&bars, &time_scale, &viewport, 1000.0, 200.0, &sizing);

        assert_eq!(projected.len(), 4);
        assert_eq!(
            projected[0].body_left, 2.0,
            "logical x=-1 CSS bias must be scaled to -2 physical px at 2x DPR"
        );
    }

    #[test]
    fn wick_width_stays_stable_when_exact_pixel_ratio_drifts() {
        let sizing = CandleSizing {
            bar_width: 12.0,
            wick_width: 3.0,
            border_width: 3.0,
            draw_body: true,
            bar_spacing: 5.0,
            h_pixel_ratio: 3.01,
            v_pixel_ratio: 3.01,
        };

        assert_eq!(effective_wick_width(&sizing), 3.0);
    }

    #[test]
    fn wick_width_is_at_least_one_css_pixel_on_fractional_dpr() {
        let sizing = CandleSizing {
            bar_width: 12.0,
            wick_width: 1.0,
            border_width: 1.0,
            draw_body: true,
            bar_spacing: 5.0,
            h_pixel_ratio: 1.5,
            v_pixel_ratio: 1.5,
        };

        assert_eq!(effective_wick_width(&sizing), 2.0);
    }

    #[test]
    fn wick_width_never_exceeds_series_bar_width() {
        let sizing = CandleSizing {
            bar_width: 2.0,
            wick_width: 3.0,
            border_width: 1.0,
            draw_body: false,
            bar_spacing: 1.0,
            h_pixel_ratio: 3.0,
            v_pixel_ratio: 3.0,
        };

        assert_eq!(effective_wick_width(&sizing), 2.0);
    }

    #[test]
    fn real_wicks_survive_subpixel_price_projection() {
        let mut bars = BarArray::new();
        bars.set(vec![Bar {
            timestamp: 1,
            open: 500.0,
            high: 501.1,
            low: 499.9,
            close: 501.0,
            volume: 100.0,
        }])
        .expect("bar");
        let time_scale = TimeScaleIndex::from_bars(&bars);
        let mut viewport = Viewport::new(100, 100);
        viewport.volume_height_ratio = 0.0;
        viewport.set_range(0.0, 1.0);
        viewport.price_min = 0.0;
        viewport.price_max = 1000.0;

        let sizing = CandleSizing::compute_from_pane(100.0, &viewport, 1.0, 1.0);
        let projected = project_candles(&bars, &time_scale, &viewport, 100.0, 100.0, &sizing);

        assert_eq!(projected.len(), 1);
        let candle = projected[0];
        assert!(
            candle.high_y < candle.body_top,
            "upper wick should remain visible after rounding"
        );
        assert!(
            candle.low_y > candle.body_bottom,
            "lower wick should remain visible after rounding"
        );
    }
}
