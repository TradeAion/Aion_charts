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

use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::series::CandleSizing;
use crate::core::renderer::draw_list::{DrawList, ColoredRect};

/// Generate the complete DrawList for one frame (legacy monolithic path).
/// Order: background → grid lines → volume → candles.
/// `pane_w` and `pane_h` are in physical pixels (chart area only, no axes).
pub fn generate(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    pane_w: f64,
    pane_h: f64,
    dpr: f64,
    y_ticks: &[TickMark],
    x_ticks: &[TickMark],
) -> DrawList {
    let mut dl = DrawList::new();

    // Background fill
    let (br, bg, bb, ba) = color4(&style.bg_color);
    dl.rects.push(ColoredRect {
        x: 0.0, y: 0.0, w: pane_w as f32, h: pane_h as f32,
        r: br, g: bg, b: bb, a: ba,
    });

    // Grid lines (as thin rects, 1 physical pixel wide)
    let grid = generate_grid_rects(style, y_ticks, x_ticks, pane_w, pane_h);
    dl.rects.extend_from_slice(&grid);

    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, dpr);

    // Volume occupies the bottom portion of pane (configured via viewport)
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    generate_volume_into(bars, viewport, style, pane_w, candle_h, vol_h, &sizing, &mut dl.rects);
    generate_candles_into(bars, viewport, style, pane_w, candle_h, &sizing, &mut dl.rects);

    dl
}

// ── Public per-element generators ────────────────────────────────────────────

/// Generate grid line rects (horizontal at price ticks, vertical at time ticks).
pub fn generate_grid_rects(
    style: &ChartStyle,
    y_ticks: &[TickMark],
    x_ticks: &[TickMark],
    pane_w: f64,
    pane_h: f64,
) -> Vec<ColoredRect> {
    let mut rects = Vec::with_capacity(y_ticks.len() + x_ticks.len());
    let (gr, gg, gb, ga) = color4(&style.grid_color);

    // Horizontal grid lines (at price ticks) — major ticks only
    for t in y_ticks {
        if !t.major { continue; }
        let y = snap(t.pixel);
        if y > 0.0 && y < pane_h {
            rects.push(ColoredRect {
                x: 0.0, y: y as f32, w: pane_w as f32, h: 1.0,
                r: gr, g: gg, b: gb, a: ga,
            });
        }
    }

    // Vertical grid lines (at time ticks) — major ticks only
    for t in x_ticks {
        if !t.major { continue; }
        let x = snap(t.pixel);
        if x > 0.0 && x < pane_w {
            rects.push(ColoredRect {
                x: x as f32, y: 0.0, w: 1.0, h: pane_h as f32,
                r: gr, g: gg, b: gb, a: ga,
            });
        }
    }

    rects
}

/// Generate candle rects (wicks, borders, body fills).
pub fn generate_candle_rects(
    bars: &crate::core::data::BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    pane_w: f64,
    pane_h: f64,
    dpr: f64,
) -> Vec<ColoredRect> {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, dpr);
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
    dpr: f64,
) -> Vec<ColoredRect> {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, dpr);
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;
    let mut rects = Vec::with_capacity(bars.len());
    generate_volume_into(bars, viewport, style, pane_w, candle_h, vol_h, &sizing, &mut rects);
    rects
}

// ── Coordinate helpers (physical pixel space) ────────────────────────────────

#[inline]
fn bar_to_x(bar_idx: f64, vp: &Viewport, chart_w: f64) -> f64 {
    (bar_idx - vp.start_bar) / (vp.end_bar - vp.start_bar) * chart_w
}

#[inline]
fn price_to_y(price: f64, vp: &Viewport, candle_h: f64) -> f64 {
    let frac = (price - vp.price_min) / (vp.price_max - vp.price_min);
    candle_h * (1.0 - frac)
}

fn color4(c: &[f32; 4]) -> (f32, f32, f32, f32) {
    (c[0], c[1], c[2], c[3])
}

fn snap(v: f64) -> f64 { v.floor() + 0.5 }

// ── Inner-border fill (matches LWC fillRectInnerBorder) ──────────────────────

fn push_inner_border(
    rects: &mut Vec<ColoredRect>,
    x: f32, y: f32, w: f32, h: f32, bw: f32,
    r: f32, g: f32, b: f32, a: f32,
) {
    // top edge
    rects.push(ColoredRect { x: x + bw, y, w: w - bw * 2.0, h: bw, r, g, b, a });
    // bottom edge
    rects.push(ColoredRect { x: x + bw, y: y + h - bw, w: w - bw * 2.0, h: bw, r, g, b, a });
    // left edge
    rects.push(ColoredRect { x, y, w: bw, h, r, g, b, a });
    // right edge
    rects.push(ColoredRect { x: x + w - bw, y, w: bw, h, r, g, b, a });
}

// ── Candle generation (3-pass LWC order: wicks → borders → body fill) ────────

fn generate_candles_into(
    bars: &crate::core::data::BarArray,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let start = (vp.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end { return; }

    let half_bar = (sizing.bar_width * 0.5).floor();
    let wick_offset = (sizing.wick_width * 0.5).floor();

    // ── Pass 1: Wicks ────────────────────────────────────────────────────
    let mut prev_edge: Option<f64> = None;
    for i in start..end {
        let b = bars.get(i);
        let bull = b.close >= b.open;
        let (wr, wg, wb, wa) = if bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        let phys_x = bar_to_x(i as f64 + 0.5, vp, chart_w).round();
        let body_top = price_to_y(b.open.max(b.close) as f64, vp, candle_h).round();
        let body_bottom = price_to_y(b.open.min(b.close) as f64, vp, candle_h).round();
        let high_y = price_to_y(b.high as f64, vp, candle_h).round();
        let low_y = price_to_y(b.low as f64, vp, candle_h).round();

        let mut left = phys_x - wick_offset;
        let right = left + sizing.wick_width - 1.0;
        if let Some(pe) = prev_edge {
            left = left.max(pe + 1.0).min(right);
        }
        let width = right - left + 1.0;

        if body_top > high_y {
            rects.push(ColoredRect {
                x: left as f32, y: high_y as f32,
                w: width as f32, h: (body_top - high_y) as f32,
                r: wr, g: wg, b: wb, a: wa,
            });
        }
        if low_y > body_bottom + 1.0 {
            rects.push(ColoredRect {
                x: left as f32, y: (body_bottom + 1.0) as f32,
                w: width as f32, h: (low_y - body_bottom) as f32,
                r: wr, g: wg, b: wb, a: wa,
            });
        }

        prev_edge = Some(right);
    }

    // ── Pass 2: Borders ──────────────────────────────────────────────────
    prev_edge = None;
    for i in start..end {
        let b = bars.get(i);
        let bull = b.close >= b.open;
        let (br, bg, bb, ba) = if bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        let phys_x = bar_to_x(i as f64 + 0.5, vp, chart_w).round();
        let mut left = phys_x - half_bar;
        let right = left + sizing.bar_width - 1.0;
        let top = price_to_y(b.open.max(b.close) as f64, vp, candle_h).round();
        let bottom = price_to_y(b.open.min(b.close) as f64, vp, candle_h).round();

        if let Some(pe) = prev_edge {
            left = left.max(pe + 1.0).min(right);
        }

        let w = right - left + 1.0;
        let h = (bottom - top + 1.0).max(1.0);

        if sizing.bar_spacing * sizing.dpr > 2.0 * sizing.border_width {
            push_inner_border(
                rects,
                left as f32, top as f32, w as f32, h as f32,
                sizing.border_width as f32,
                br, bg, bb, ba,
            );
        } else {
            rects.push(ColoredRect {
                x: left as f32, y: top as f32, w: w as f32, h: h as f32,
                r: br, g: bg, b: bb, a: ba,
            });
        }

        prev_edge = Some(right);
    }

    // ── Pass 3: Body fill ────────────────────────────────────────────────
    if sizing.draw_body {
        for i in start..end {
            let b = bars.get(i);
            let bull = b.close >= b.open;
            let (cr, cg, cb, ca) = if bull {
                color4(&style.bullish_color)
            } else {
                color4(&style.bearish_color)
            };

            let phys_x = bar_to_x(i as f64 + 0.5, vp, chart_w).round();
            let left = phys_x - half_bar;
            let right = left + sizing.bar_width - 1.0;
            let top = price_to_y(b.open.max(b.close) as f64, vp, candle_h).round();
            let bottom = price_to_y(b.open.min(b.close) as f64, vp, candle_h).round();

            let bl = left + sizing.border_width;
            let bt = top + sizing.border_width;
            let br_x = right - sizing.border_width;
            let bb_y = bottom - sizing.border_width;

            if bt > bb_y { continue; }

            rects.push(ColoredRect {
                x: bl as f32, y: bt as f32,
                w: (br_x - bl + 1.0) as f32, h: (bb_y - bt + 1.0) as f32,
                r: cr, g: cg, b: cb, a: ca,
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
    let start = (vp.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end { return; }

    let mut max_vol = 0.0f32;
    for i in start..end {
        max_vol = max_vol.max(bars.volumes.value(i));
    }
    if max_vol <= 0.0 { return; }

    let half_bar = (sizing.bar_width * 0.5).floor();

    for i in start..end {
        let b = bars.get(i);
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
            x: left as f32, y: top.floor() as f32,
            w: sizing.bar_width as f32, h: h.ceil() as f32,
            r: cr, g: cg, b: cb, a: ca,
        });
    }
}
