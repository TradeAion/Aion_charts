//! VolumeSeriesCanvas2D — standalone Canvas2D volume histogram renderer.
//!
//! Draws volume bars below the candle area. Uses the same bar width
//! from CandleSizing (optimalCandlestickWidth) so volume bars align
//! with candle bodies.

#![cfg(target_arch = "wasm32")]

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::ChartStyle;
use crate::core::renderer::series::{ChartLayout, CandleSizing, PaneSeriesRenderer};
use web_sys::CanvasRenderingContext2d;

pub struct VolumeSeriesCanvas2D;

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

impl VolumeSeriesCanvas2D {
    pub fn new() -> Self { Self }
}

impl PaneSeriesRenderer for VolumeSeriesCanvas2D {
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

        let visible = &bars[start..end];
        let max_vol = visible.iter().map(|b| b.volume).fold(0.0f32, f32::max);
        if max_vol <= 0.0 { return; }

        let sizing = CandleSizing::compute(layout, vp);
        let half_bar = (sizing.bar_width * 0.5).floor();

        for i in start..end {
            let b = &bars[i];
            let cx = bar_to_x(i as f64 + 0.5, vp, layout.chart_w);
            let clr = if b.is_bullish() { &style.bullish_volume_color } else { &style.bearish_volume_color };
            let h = (b.volume as f64 / max_vol as f64) * layout.vol_h;
            let top = layout.candle_h + layout.vol_h - h;

            let phys_x = cx.round();
            let left = phys_x - half_bar;

            ctx.set_fill_style_str(&rgba(clr));
            ctx.fill_rect(left, top.floor(), sizing.bar_width, h.ceil());
        }
    }
}
