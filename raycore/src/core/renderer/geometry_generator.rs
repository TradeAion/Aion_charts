//! GeometryGenerator — single source of truth for ALL visual math.
//!
//! Takes Viewport, data, style → produces a DrawList of pixel-perfect
//! ColoredRects. Both Canvas2D and WebGPU renderers consume this
//! identically, guaranteeing visual consistency.
//!
//! All candle sizing uses LWC-matching algorithms from series.rs.

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::series::{ChartLayout, CandleSizing};
use crate::core::renderer::draw_list::{DrawList, ColoredRect};
use crate::core::formatters::{format_price, format_timestamp, nice_step};

/// Generate the complete DrawList for one frame.
/// Order: background → grid lines → volume → candles.
/// Returns (DrawList, y_ticks, x_ticks) — ticks are needed by the overlay for axis labels.
pub fn generate(
    bars: &[Bar],
    viewport: &Viewport,
    style: &ChartStyle,
    layout: &ChartLayout,
) -> (DrawList, Vec<TickMark>, Vec<TickMark>) {
    let mut dl = DrawList::new();

    // 1. Background fill — makes the canvas opaque, no alpha compositing needed
    let bg = &style.bg_color;
    dl.rects.push(ColoredRect {
        x: 0.0, y: 0.0,
        w: layout.total_w as f32, h: layout.total_h as f32,
        r: bg[0], g: bg[1], b: bg[2], a: bg[3],
    });

    // 2. Grid lines (as thin rects)
    let y_ticks = compute_y_ticks(viewport, layout);
    let x_ticks = compute_x_ticks(viewport, bars, layout);
    generate_grid_lines(viewport, style, layout, &y_ticks, &x_ticks, &mut dl);

    // 3. Data
    let sizing = CandleSizing::compute(layout, viewport);
    generate_volume(bars, viewport, style, layout, &sizing, &mut dl);
    generate_candles(bars, viewport, style, layout, &sizing, &mut dl);

    (dl, y_ticks, x_ticks)
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

// ── Inner-border fill (matches LWC fillRectInnerBorder) ──────────────────────

fn push_inner_border(
    dl: &mut DrawList,
    x: f32, y: f32, w: f32, h: f32, bw: f32,
    r: f32, g: f32, b: f32, a: f32,
) {
    // top edge
    dl.rects.push(ColoredRect { x: x + bw, y, w: w - bw * 2.0, h: bw, r, g, b, a });
    // bottom edge
    dl.rects.push(ColoredRect { x: x + bw, y: y + h - bw, w: w - bw * 2.0, h: bw, r, g, b, a });
    // left edge
    dl.rects.push(ColoredRect { x, y, w: bw, h, r, g, b, a });
    // right edge
    dl.rects.push(ColoredRect { x: x + w - bw, y, w: bw, h, r, g, b, a });
}

// ── Candle generation (3-pass LWC order: wicks → borders → body fill) ────────

fn generate_candles(
    bars: &[Bar],
    vp: &Viewport,
    style: &ChartStyle,
    layout: &ChartLayout,
    sizing: &CandleSizing,
    dl: &mut DrawList,
) {
    let start = (vp.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end { return; }

    let dpr = layout.dpr;
    let half_bar = (sizing.bar_width * 0.5).floor();
    let wick_offset = (sizing.wick_width * 0.5).floor();

    // ── Pass 1: Wicks ────────────────────────────────────────────────────
    let mut prev_edge: Option<f64> = None;
    for i in start..end {
        let b = &bars[i];
        let bull = b.close >= b.open;
        let (wr, wg, wb, wa) = if bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        let phys_x = bar_to_x(i as f64 + 0.5, vp, layout.chart_w).round();

        // max price → smaller Y (higher on screen) = visual top
        let body_top = price_to_y(b.open.max(b.close) as f64, vp, layout.candle_h).round();
        // min price → larger Y (lower on screen) = visual bottom
        let body_bottom = price_to_y(b.open.min(b.close) as f64, vp, layout.candle_h).round();
        let high_y = price_to_y(b.high as f64, vp, layout.candle_h).round();
        let low_y = price_to_y(b.low as f64, vp, layout.candle_h).round();

        let mut left = phys_x - wick_offset;
        let right = left + sizing.wick_width - 1.0;
        if let Some(pe) = prev_edge {
            left = left.max(pe + 1.0).min(right);
        }
        let width = right - left + 1.0;

        // Upper wick: from high to body top
        if body_top > high_y {
            dl.rects.push(ColoredRect {
                x: left as f32, y: high_y as f32,
                w: width as f32, h: (body_top - high_y) as f32,
                r: wr, g: wg, b: wb, a: wa,
            });
        }
        // Lower wick: from body bottom to low
        if low_y > body_bottom + 1.0 {
            dl.rects.push(ColoredRect {
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
        let b = &bars[i];
        let bull = b.close >= b.open;
        let (br, bg, bb, ba) = if bull {
            color4(&style.wick_bullish_color)
        } else {
            color4(&style.wick_bearish_color)
        };

        let phys_x = bar_to_x(i as f64 + 0.5, vp, layout.chart_w).round();

        let mut left = phys_x - half_bar;
        let right = left + sizing.bar_width - 1.0;

        let top = price_to_y(b.open.max(b.close) as f64, vp, layout.candle_h).round();
        let bottom = price_to_y(b.open.min(b.close) as f64, vp, layout.candle_h).round();

        if let Some(pe) = prev_edge {
            left = left.max(pe + 1.0).min(right);
        }

        let w = right - left + 1.0;
        let h = (bottom - top + 1.0).max(1.0);

        if sizing.bar_spacing * dpr > 2.0 * sizing.border_width {
            push_inner_border(
                dl,
                left as f32, top as f32, w as f32, h as f32,
                sizing.border_width as f32,
                br, bg, bb, ba,
            );
        } else {
            dl.rects.push(ColoredRect {
                x: left as f32, y: top as f32, w: w as f32, h: h as f32,
                r: br, g: bg, b: bb, a: ba,
            });
        }

        prev_edge = Some(right);
    }

    // ── Pass 3: Body fill ────────────────────────────────────────────────
    if sizing.draw_body {
        for i in start..end {
            let b = &bars[i];
            let bull = b.close >= b.open;
            let (cr, cg, cb, ca) = if bull {
                color4(&style.bullish_color)
            } else {
                color4(&style.bearish_color)
            };

            let phys_x = bar_to_x(i as f64 + 0.5, vp, layout.chart_w).round();
            let left = phys_x - half_bar;
            let right = left + sizing.bar_width - 1.0;
            let top = price_to_y(b.open.max(b.close) as f64, vp, layout.candle_h).round();
            let bottom = price_to_y(b.open.min(b.close) as f64, vp, layout.candle_h).round();

            let bl = left + sizing.border_width;
            let bt = top + sizing.border_width;
            let br_x = right - sizing.border_width;
            let bb_y = bottom - sizing.border_width;

            if bt > bb_y { continue; }

            dl.rects.push(ColoredRect {
                x: bl as f32, y: bt as f32,
                w: (br_x - bl + 1.0) as f32, h: (bb_y - bt + 1.0) as f32,
                r: cr, g: cg, b: cb, a: ca,
            });
        }
    }
}

// ── Volume generation ────────────────────────────────────────────────────────

fn generate_volume(
    bars: &[Bar],
    vp: &Viewport,
    style: &ChartStyle,
    layout: &ChartLayout,
    sizing: &CandleSizing,
    dl: &mut DrawList,
) {
    let start = (vp.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
    let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
    if start >= end { return; }

    let visible = &bars[start..end];
    let max_vol = visible.iter().map(|b| b.volume).fold(0.0f32, f32::max);
    if max_vol <= 0.0 { return; }

    let half_bar = (sizing.bar_width * 0.5).floor();

    for i in start..end {
        let b = &bars[i];
        let (cr, cg, cb, ca) = if b.is_bullish() {
            color4(&style.bullish_volume_color)
        } else {
            color4(&style.bearish_volume_color)
        };

        let cx = bar_to_x(i as f64 + 0.5, vp, layout.chart_w);
        let h = (b.volume as f64 / max_vol as f64) * layout.vol_h;
        let top = layout.candle_h + layout.vol_h - h;

        let phys_x = cx.round();
        let left = phys_x - half_bar;

        dl.rects.push(ColoredRect {
            x: left as f32, y: top.floor() as f32,
            w: sizing.bar_width as f32, h: h.ceil() as f32,
            r: cr, g: cg, b: cb, a: ca,
        });
    }
}

// ── Grid tick computation ────────────────────────────────────────────────────

fn compute_y_ticks(vp: &Viewport, layout: &ChartLayout) -> Vec<TickMark> {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 { return vec![]; }
    let step = nice_step(range / (layout.candle_h / (40.0 * layout.dpr)).max(3.0).min(15.0));
    let first = (vp.price_min / step).ceil() * step;
    let mut out = Vec::new();
    let mut v = first;
    while v <= vp.price_max {
        let px = price_to_y(v, vp, layout.candle_h);
        out.push(TickMark {
            value: v,
            pixel: px,
            label: format_price(v, step),
            major: true,
        });
        v += step;
    }
    out
}

fn compute_x_ticks(vp: &Viewport, bars: &[Bar], layout: &ChartLayout) -> Vec<TickMark> {
    let count = vp.end_bar - vp.start_bar;
    if count <= 0.0 { return vec![]; }
    let step = nice_step(count / (layout.chart_w / (100.0 * layout.dpr)).max(2.0)).max(1.0);
    let first = (vp.start_bar / step).ceil() * step;
    let mut out = Vec::new();
    let mut v = first;
    while v <= vp.end_bar {
        let px = bar_to_x(v, vp, layout.chart_w);
        let bar_i = v as usize;
        let label = if bar_i < bars.len() && bars[bar_i].timestamp > 0 {
            format_timestamp(bars[bar_i].timestamp)
        } else {
            format!("{}", v as i64)
        };
        out.push(TickMark { value: v, pixel: px, label, major: true });
        v += step;
    }
    out
}

// ── Grid line generation (thin rects) ────────────────────────────────────────

/// Pixel snap for crisp 1px lines (matches LWC `snap`).
#[inline]
fn snap(v: f64) -> f64 { v.floor() }

fn generate_grid_lines(
    vp: &Viewport,
    style: &ChartStyle,
    layout: &ChartLayout,
    y_ticks: &[TickMark],
    x_ticks: &[TickMark],
    dl: &mut DrawList,
) {
    let gc = &style.grid_color;
    let total_h = layout.candle_h + layout.vol_h;

    // Horizontal grid lines (at price ticks) — 1px tall rects
    for t in y_ticks {
        if !t.major { continue; }
        let y = snap(price_to_y(t.value, vp, layout.candle_h));
        if y > 0.0 && y < total_h {
            dl.rects.push(ColoredRect {
                x: 0.0, y: y as f32,
                w: layout.chart_w as f32, h: 1.0,
                r: gc[0], g: gc[1], b: gc[2], a: gc[3],
            });
        }
    }

    // Vertical grid lines (at time ticks) — 1px wide rects
    for t in x_ticks {
        if !t.major { continue; }
        let x = snap(bar_to_x(t.value, vp, layout.chart_w));
        if x > 0.0 && x < layout.chart_w {
            dl.rects.push(ColoredRect {
                x: x as f32, y: 0.0,
                w: 1.0, h: total_h as f32,
                r: gc[0], g: gc[1], b: gc[2], a: gc[3],
            });
        }
    }
}
