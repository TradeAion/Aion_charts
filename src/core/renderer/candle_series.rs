//! CandleSeriesCanvas2D — standalone Canvas2D candle renderer.
//!
//! Draws OHLC candlesticks matching LWC's PaneRendererCandlesticks exactly:
//!   1. Wicks (thin vertical lines, floor(dpr) px wide)
//!   2. Borders (inner-border fill around the body rect)
//!   3. Body fill (color fill inset by border_width)
//!
//! Uses optimalCandlestickWidth for bar width, parity-matched to wick width.

#![cfg(target_arch = "wasm32")]

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::ChartStyle;
use crate::core::renderer::series::{ChartLayout, CandleSizing, PaneSeriesRenderer};
use web_sys::CanvasRenderingContext2d;

pub struct CandleSeriesCanvas2D;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn rgba(c: &[f32; 4]) -> String {
    format!(
        "rgba({},{},{},{})",
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        c[3]
    )
}

#[inline]
fn bar_to_x(bar_idx: f64, vp: &Viewport, chart_w: f64) -> f64 {
    (bar_idx - vp.start_bar) / (vp.end_bar - vp.start_bar) * chart_w
}

#[inline]
fn price_to_y(price: f64, vp: &Viewport, candle_h: f64) -> f64 {
    let frac = (price - vp.price_min) / (vp.price_max - vp.price_min);
    candle_h * (1.0 - frac)
}

/// Draw inner border rect matching LWC's `fillRectInnerBorder`.
fn fill_rect_inner_border(
    ctx: &CanvasRenderingContext2d,
    x: f64, y: f64, w: f64, h: f64, bw: f64,
) {
    // top edge
    ctx.fill_rect(x + bw, y, w - bw * 2.0, bw);
    // bottom edge
    ctx.fill_rect(x + bw, y + h - bw, w - bw * 2.0, bw);
    // left edge
    ctx.fill_rect(x, y, bw, h);
    // right edge
    ctx.fill_rect(x + w - bw, y, bw, h);
}

impl CandleSeriesCanvas2D {
    pub fn new() -> Self { Self }
}

impl PaneSeriesRenderer for CandleSeriesCanvas2D {
    fn draw(
        &self,
        ctx: &CanvasRenderingContext2d,
        bars: &[Bar],
        vp: &Viewport,
        style: &ChartStyle,
        layout: &ChartLayout,
    ) {
        let start = (vp.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
        let end = ((vp.end_bar.ceil() as usize) + 1).min(bars.len());
        if start >= end { return; }

        let sizing = CandleSizing::compute(layout, vp);
        let dpr = layout.dpr;
        let half_bar = (sizing.bar_width * 0.5).floor();
        let wick_offset = (sizing.wick_width * 0.5).floor();

        // ── Pass 1: Wicks ────────────────────────────────────────────────
        let mut prev_edge: Option<f64> = None;
        for i in start..end {
            let b = &bars[i];
            let bull = b.close >= b.open;
            let wick_c = if bull { &style.wick_bullish_color } else { &style.wick_bearish_color };
            ctx.set_fill_style_str(&rgba(wick_c));

            let phys_x = (bar_to_x(i as f64 + 0.5, vp, layout.chart_w)).round();

            let top = (price_to_y(b.open.min(b.close) as f64, vp, layout.candle_h)).round();
            let bottom = (price_to_y(b.open.max(b.close) as f64, vp, layout.candle_h)).round();
            let high = (price_to_y(b.high as f64, vp, layout.candle_h)).round();
            let low = (price_to_y(b.low as f64, vp, layout.candle_h)).round();

            let mut left = phys_x - wick_offset;
            let right = left + sizing.wick_width - 1.0;
            if let Some(pe) = prev_edge {
                left = left.max(pe + 1.0).min(right);
            }
            let width = right - left + 1.0;

            // Upper wick: from high to body top
            if top > high {
                ctx.fill_rect(left, high, width, top - high);
            }
            // Lower wick: from body bottom to low
            if low > bottom + 1.0 {
                ctx.fill_rect(left, bottom + 1.0, width, low - bottom);
            }

            prev_edge = Some(right);
        }

        // ── Pass 2: Borders ──────────────────────────────────────────────
        prev_edge = None;
        for i in start..end {
            let b = &bars[i];
            let bull = b.close >= b.open;
            // LWC uses barBorderColor; we use wick color for border
            let border_c = if bull { &style.wick_bullish_color } else { &style.wick_bearish_color };
            ctx.set_fill_style_str(&rgba(border_c));

            let phys_x = (bar_to_x(i as f64 + 0.5, vp, layout.chart_w)).round();

            let mut left = phys_x - half_bar;
            let right = left + sizing.bar_width - 1.0;

            let top = (price_to_y(b.open.min(b.close) as f64, vp, layout.candle_h)).round();
            let bottom = (price_to_y(b.open.max(b.close) as f64, vp, layout.candle_h)).round();

            if let Some(pe) = prev_edge {
                left = left.max(pe + 1.0).min(right);
            }

            let w = right - left + 1.0;
            let h = (bottom - top + 1.0).max(1.0);

            if sizing.bar_spacing * dpr > 2.0 * sizing.border_width {
                fill_rect_inner_border(ctx, left, top, w, h, sizing.border_width);
            } else {
                ctx.fill_rect(left, top, w, h);
            }

            prev_edge = Some(right);
        }

        // ── Pass 3: Body fill ────────────────────────────────────────────
        if sizing.draw_body {
            for i in start..end {
                let b = &bars[i];
                let bull = b.close >= b.open;
                let body_c = if bull { &style.bullish_color } else { &style.bearish_color };
                ctx.set_fill_style_str(&rgba(body_c));

                let phys_x = (bar_to_x(i as f64 + 0.5, vp, layout.chart_w)).round();
                let left = phys_x - half_bar;
                let right = left + sizing.bar_width - 1.0;
                let top = (price_to_y(b.open.min(b.close) as f64, vp, layout.candle_h)).round();
                let bottom = (price_to_y(b.open.max(b.close) as f64, vp, layout.candle_h)).round();

                // Inset by border
                let bl = left + sizing.border_width;
                let bt = top + sizing.border_width;
                let br = right - sizing.border_width;
                let bb = bottom - sizing.border_width;

                if bt > bb { continue; }

                ctx.fill_rect(bl, bt, br - bl + 1.0, bb - bt + 1.0);
            }
        }
    }
}
