//! OverlayRenderer — pane top canvas: crosshair, watermark, legend, drawings.
//!
//! Sits on the pane's top canvas (z-index:2).
//! No longer draws axes or crosshair labels — those are on their own widget canvases.
//!
//! Also renders dashed line series via Canvas2D strokePath (non-Solid LineStyle).

#![cfg(target_arch = "wasm32")]

use crate::core::data::BarArray;
use crate::core::drawings::types::DrawingGeometry;
use crate::core::formatters::{format_price, format_volume};
use crate::core::markers::{MarkerManager, MarkerPosition, MarkerShape};
use crate::core::price_line::PriceLineManager;
use crate::core::renderer::canvas_dash::{clear_canvas_line_dash, set_canvas_line_dash};
use crate::core::renderer::line_generator;
use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::text_cache::TextWidthCache;
use crate::core::renderer::traits::{ChartStyle, CrosshairState};
use crate::core::renderer::transforms::bar_to_x;
use crate::core::renderer::value_projection::{
    price_to_pane_y_phys, timestamp_to_bar_index_in_bars,
};
use crate::core::series::{LineStyle, SeriesCollection, SeriesType};
use crate::core::viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct OverlayRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    /// Reference to the base chart canvas for rendering base-layer drawings.
    base_canvas: Option<HtmlCanvasElement>,
    base_ctx: Option<CanvasRenderingContext2d>,
    pw: u32,
    ph: u32,
    dpr: f64,
    /// Shared text width cache for legend measurements.
    text_cache: TextWidthCache,
}

impl OverlayRenderer {
    pub fn new(canvas: HtmlCanvasElement, dpr: f64) -> Result<Self, String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("overlay get_context('2d') failed: {:?}", e))?
            .ok_or("overlay get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "overlay context is not CanvasRenderingContext2d")?;

        ctx.set_image_smoothing_enabled(false);
        let pw = canvas.width();
        let ph = canvas.height();
        Ok(Self {
            canvas,
            ctx,
            base_canvas: None,
            base_ctx: None,
            pw,
            ph,
            dpr,
            text_cache: TextWidthCache::new(50),
        })
    }

    /// Set the base chart canvas for rendering base-layer drawings.
    /// Call this after construction to enable drawing on the chart canvas.
    pub fn set_base_canvas(&mut self, canvas: HtmlCanvasElement) -> Result<(), String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("base canvas get_context('2d') failed: {:?}", e))?
            .ok_or("base canvas get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "base canvas context is not CanvasRenderingContext2d")?;
        ctx.set_image_smoothing_enabled(false);
        self.base_canvas = Some(canvas);
        self.base_ctx = Some(ctx);
        Ok(())
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        let pw = pw.max(1);
        let ph = ph.max(1);
        if self.pw == pw && self.ph == ph && (self.dpr - dpr).abs() < 1e-6 {
            return;
        }

        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        if self.canvas.width() != pw {
            self.canvas.set_width(pw);
        }
        if self.canvas.height() != ph {
            self.canvas.set_height(ph);
        }
        self.ctx.set_image_smoothing_enabled(false);
    }

    /// Render crosshair lines + active drawings on the pane's top canvas.
    pub fn render(&mut self, crosshair: &CrosshairState, style: &ChartStyle) {
        self.render_with_drawings(crosshair, style, &[], None);
    }

    /// Render crosshair + top-layer drawing geometry on the overlay canvas.
    pub fn render_with_drawings(
        &mut self,
        crosshair: &CrosshairState,
        style: &ChartStyle,
        top_drawings: &[DrawingGeometry],
        bars: Option<&BarArray>,
    ) {
        let pw = self.pw as f64;
        let ph = self.ph as f64;
        self.ctx.clear_rect(0.0, 0.0, pw, ph);

        // Watermark: below everything
        self.render_watermark(style);

        // Legend: OHLCV values in top-left corner
        if let Some(bars) = bars {
            self.render_legend(crosshair, style, bars);
        }

        // Draw active/hovered drawings BELOW crosshair
        for geom in top_drawings {
            self.draw_geometry(geom);
        }

        self.draw_crosshair(crosshair, style, pw, ph);
    }

    /// Draw a DrawingGeometry on the overlay canvas.
    fn draw_geometry(&self, geom: &DrawingGeometry) {
        Self::draw_geometry_on(&self.ctx, geom);
    }

    /// Draw a DrawingGeometry (lines, rects, text, anchor circles) on any 2D context.
    fn draw_geometry_on(ctx: &CanvasRenderingContext2d, geom: &DrawingGeometry) {
        // Filled rects
        for r in &geom.rects {
            if r.w <= 0.0 || r.h <= 0.0 {
                continue;
            }
            ctx.set_fill_style_str(&rgba(&[r.r, r.g, r.b, r.a]));
            ctx.fill_rect(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        }

        // Lines
        for l in &geom.lines {
            ctx.set_stroke_style_str(&rgba(&[l.r, l.g, l.b, l.a]));
            ctx.set_line_width(l.width as f64);
            ctx.set_line_cap("round");
            ctx.set_line_join("round");

            if l.dash > 0.0 && l.gap > 0.0 {
                let _ = ctx.set_line_dash(&js_sys::Array::of2(
                    &JsValue::from(l.dash as f64),
                    &JsValue::from(l.gap as f64),
                ));
            } else {
                let _ = ctx.set_line_dash(&js_sys::Array::new());
            }

            // LWC strokeInPixel: add 0.5px offset for odd-width lines
            // to snap to pixel center and prevent blurry sub-pixel rendering
            let correction = if (l.width as i32) % 2 == 1 { 0.5 } else { 0.0 };

            ctx.begin_path();
            ctx.move_to(l.x0 as f64 + correction, l.y0 as f64 + correction);
            ctx.line_to(l.x1 as f64 + correction, l.y1 as f64 + correction);
            ctx.stroke();
        }
        clear_canvas_line_dash(ctx);

        // Text labels (in physical pixel coords)
        for t in &geom.texts {
            let font = format!(
                "{}px {}",
                t.font_size,
                crate::core::renderer::theme::FONT_FAMILY
            );
            ctx.set_font(&font);
            ctx.set_fill_style_str(&rgba(&[t.r, t.g, t.b, t.a]));
            ctx.set_text_align("center");
            ctx.set_text_baseline("middle");
            let _ = ctx.fill_text(&t.text, t.x as f64, t.y as f64);
        }

        // Anchor circles
        for a in &geom.anchors {
            // Fill
            ctx.set_fill_style_str(&rgba(&a.fill));
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.fill();
            // Border
            ctx.set_stroke_style_str(&rgba(&a.border));
            ctx.set_line_width(a.border_width);
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.stroke();
        }
    }

    /// Render base-layer drawings on the chart (pane base) canvas.
    /// These sit above candles but below the crosshair/top canvas.
    /// Does NOT clear the canvas — call after engine.render() which already drew candles.
    pub fn render_base_drawings(&self, drawings: &[DrawingGeometry]) {
        let ctx = match &self.base_ctx {
            Some(c) => c,
            None => return, // base canvas not set up
        };
        for geom in drawings {
            Self::draw_geometry_on(ctx, geom);
        }
    }

    /// Render a watermark centered on the pane.
    /// Drawn on the top canvas, below drawings and crosshair.
    /// LWC pattern: text-watermark pane-renderer with auto-zoom shrink.
    pub fn render_watermark(&mut self, style: &ChartStyle) {
        let text = &style.watermark_text;
        if text.is_empty() {
            return;
        }

        let pw = self.pw as f64;
        let ph = self.ph as f64;
        let dpr = self.dpr;

        // Compute font size in physical px
        let fs = style.font_size_watermark as f64 * dpr;
        let font = format!("bold {}px {}", fs.round(), style.font_family);
        self.ctx.set_font(&font);
        self.ctx.set_fill_style_str(&rgba(&style.watermark_color));
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");

        // Measure text, auto-shrink if wider than pane
        let text_w = self.text_cache.measure(&self.ctx, text, &font);
        let zoom = if text_w > pw && text_w > 0.0 {
            pw / text_w
        } else {
            1.0
        };

        self.ctx.save();
        // Translate to center of pane, then scale for zoom
        self.ctx.translate(pw / 2.0, ph / 2.0).unwrap_or(());
        self.ctx.scale(zoom, zoom).unwrap_or(());
        let _ = self.ctx.fill_text(text, 0.0, 0.0);
        self.ctx.restore();
    }

    /// Render OHLCV legend in the top-left corner of the pane.
    /// Shows values for the bar at the crosshair position, or the last bar if no crosshair.
    fn render_legend(&mut self, crosshair: &CrosshairState, style: &ChartStyle, bars: &BarArray) {
        if bars.len() == 0 {
            return;
        }

        let dpr = self.dpr;

        // Pick the bar to display: hovered bar or last bar
        let bar_i = if crosshair.active {
            crosshair
                .bar_index
                .unwrap_or(bars.len() - 1)
                .min(bars.len() - 1)
        } else {
            bars.len() - 1
        };

        let o = bars.open(bar_i) as f64;
        let h = bars.high(bar_i) as f64;
        let l = bars.low(bar_i) as f64;
        let c = bars.close(bar_i) as f64;
        let v = bars.volume(bar_i) as f64;

        let is_bullish = c >= o;

        // Determine price step for formatting
        let step = if h > l && (h - l) > 0.0 {
            (h - l) / 4.0
        } else {
            0.01
        };

        // Format values
        let o_str = format_price(o, step);
        let h_str = format_price(h, step);
        let l_str = format_price(l, step);
        let c_str = format_price(c, step);
        let v_str = format_volume(v);

        // Draw in CSS coordinate space for sharp text
        self.ctx.save();
        let _ = self.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        let fs = style.font_size as f64;
        let font = format!("{}px {}", fs, style.font_family);
        self.ctx.set_font(&font);
        self.ctx.set_text_baseline("top");
        self.ctx.set_text_align("left");

        let pad = 6.0; // CSS px padding from pane edge
        let gap = 4.0; // CSS px gap between label and value
        let mut x = pad;
        let y = pad;

        let label_color = &rgba(&style.axis_text_color);
        let value_color = if is_bullish {
            rgba(&style.bullish_color)
        } else {
            rgba(&style.bearish_color)
        };

        // Helper: draw "Label: Value" pair, advancing x
        let labels = [
            ("O", &o_str),
            ("H", &h_str),
            ("L", &l_str),
            ("C", &c_str),
            ("V", &v_str),
        ];
        for (label, value) in &labels {
            // Label
            self.ctx.set_fill_style_str(label_color);
            let _ = self.ctx.fill_text(label, x, y);
            let lw = self.text_cache.measure(&self.ctx, label, &font);
            x += lw + 2.0;

            // Value (colored)
            self.ctx.set_fill_style_str(&value_color);
            let _ = self.ctx.fill_text(value, x, y);
            let vw = self.text_cache.measure(&self.ctx, value, &font);
            x += vw + gap;
        }

        self.ctx.restore();
    }

    /// Render dashed (non-Solid) line series via Canvas2D strokePath.
    ///
    /// For Canvas2D backend: draws on the base chart canvas (same z-level as solid lines).
    /// For WebGPU backend: draws on the overlay canvas (above the GPU surface).
    ///
    /// Uses `setLineDash()` with the LWC dash table, then `beginPath/moveTo/lineTo/stroke`.
    pub fn render_dashed_series(
        &self,
        series: &SeriesCollection,
        viewport: &Viewport,
        bar_timestamps: &[u64],
        pane_w: f64,
        pane_h: f64,
        v_ratio: f64,
        on_overlay: bool,
    ) {
        // Pick target canvas context
        let ctx = if on_overlay {
            &self.ctx
        } else {
            match &self.base_ctx {
                Some(c) => c,
                None => return,
            }
        };

        for s in series.iter() {
            if s.series_type() != SeriesType::Line || !s.line_options.visible {
                continue;
            }
            if !s.line_options.line_style.is_dashed() {
                continue;
            }

            let points = line_generator::generate_line_series_points(
                s,
                viewport,
                bar_timestamps,
                pane_w,
                pane_h,
            );
            if points.len() < 2 {
                continue;
            }

            Self::stroke_line_series(
                ctx,
                &points,
                &s.line_options.color,
                s.line_options.line_width,
                v_ratio,
                &s.line_options.line_style,
            );
        }
    }

    /// Draw a line series as a Canvas2D stroked path with an optional dash pattern.
    ///
    /// `points`: pixel-space (x, y) pairs (already in physical px).
    /// `color`: [R, G, B, A] in 0.0–1.0.
    /// `css_width`: line width in CSS px.
    /// `v_ratio`: vertical pixel ratio (CSS -> physical).
    /// `style`: the LineStyle (determines dash pattern).
    fn stroke_line_series(
        ctx: &CanvasRenderingContext2d,
        points: &[(f64, f64)],
        color: &[f32; 4],
        css_width: f64,
        v_ratio: f64,
        style: &LineStyle,
    ) {
        let line_w = (css_width * v_ratio).round().max(1.0);
        set_canvas_line_dash(ctx, *style, line_w);

        ctx.set_stroke_style_str(&rgba(color));
        ctx.set_line_width(line_w);
        ctx.set_line_join("round");
        ctx.set_line_cap("butt");

        // LWC strokeInPixel: 0.5px offset for odd-width lines
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        // Build a single connected path through all points
        ctx.begin_path();
        let (x0, y0) = points[0];
        ctx.move_to(x0.round() + correction, y0.round() + correction);

        for &(x, y) in &points[1..] {
            ctx.line_to(x.round() + correction, y.round() + correction);
        }

        ctx.stroke();

        // Reset dash
        clear_canvas_line_dash(ctx);
    }

    /// Render horizontal live last-price lines for all visible series.
    ///
    /// Each line starts at the currently printing point and extends to the
    /// price axis edge, so it stays visually connected to the live print.
    /// Style is controlled by `style.last_price_line` (LWC-like options).
    pub fn render_last_price_lines(
        &self,
        series: &SeriesCollection,
        bars: &crate::core::data::BarArray,
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
        _time_ms: f64,
    ) {
        if !style.last_price_line.visible {
            return;
        }

        let dpr = self.dpr;
        let pane_pw = pane_css_w * dpr;
        let pane_ph = pane_css_h * dpr;
        let candle_h = pane_ph * viewport.candle_height_frac();

        let line_w = (style.last_price_line.width * dpr).floor().max(1.0);
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        self.ctx.set_line_width(line_w);
        self.ctx.set_line_cap("butt");
        set_canvas_line_dash(&self.ctx, style.last_price_line.style, line_w);

        // Main series (candles / line / area / bars / baseline)
        if bars.len() > 0 {
            if let Some(last) = bars.get(bars.len() - 1) {
                let x_anchor = bar_to_x((bars.len() - 1) as f64 + 0.5, viewport, pane_pw);
                let y_phys = price_to_pane_y_phys(last.close as f64, viewport, pane_ph);
                if x_anchor >= 0.0 && x_anchor < pane_pw && y_phys >= 0.0 && y_phys <= candle_h {
                    let y = y_phys.round() + correction;
                    self.ctx
                        .set_stroke_style_str(&rgba(&if last.close >= last.open {
                            style.bullish_color
                        } else {
                            style.bearish_color
                        }));
                    self.ctx.begin_path();
                    self.ctx.move_to(x_anchor.max(0.0), y);
                    self.ctx.line_to(pane_pw + line_w + 1.0, y);
                    self.ctx.stroke();
                }
            }
        }

        // Overlay series
        for s in series.iter() {
            if !s.is_visible() {
                continue;
            }

            let (last_price, last_ts, color) = match s.series_type() {
                SeriesType::Line | SeriesType::Area | SeriesType::Baseline => {
                    if s.line_data.is_empty() {
                        continue;
                    }
                    (
                        s.line_data.values[s.line_data.len() - 1] as f64,
                        s.line_data.last_timestamp(),
                        s.series_color(),
                    )
                }
                SeriesType::Histogram => {
                    if s.histogram_data.is_empty() {
                        continue;
                    }
                    (
                        s.histogram_data.values[s.histogram_data.len() - 1] as f64,
                        s.histogram_data.last_timestamp(),
                        s.series_color(),
                    )
                }
                SeriesType::Bar => {
                    if s.bar_data.is_empty() {
                        continue;
                    }
                    (
                        s.bar_data.close[s.bar_data.len() - 1] as f64,
                        s.bar_data.last_timestamp(),
                        s.series_color(),
                    )
                }
                SeriesType::Candlestick => continue,
            };

            let ts = match last_ts {
                Some(v) => v,
                None => continue,
            };
            let bar_idx = match timestamp_to_bar_index_in_bars(ts, bars) {
                Some(v) => v,
                None => continue,
            };
            let x_anchor = bar_to_x(bar_idx + 0.5, viewport, pane_pw);
            let y_phys = price_to_pane_y_phys(last_price, viewport, pane_ph);
            if x_anchor < 0.0 || x_anchor >= pane_pw || y_phys < 0.0 || y_phys > candle_h {
                continue;
            }

            let y = y_phys.round() + correction;
            self.ctx.set_stroke_style_str(&rgba(&color));
            self.ctx.begin_path();
            self.ctx.move_to(x_anchor.max(0.0), y);
            self.ctx.line_to(pane_pw + line_w + 1.0, y);
            self.ctx.stroke();
        }

        // Reset dash
        clear_canvas_line_dash(&self.ctx);
    }

    /// Render custom price lines.
    ///
    /// Each line is a horizontal line at a specified price, spanning the full pane width.
    /// Supports all LineStyle dash patterns.
    pub fn render_price_lines(
        &self,
        price_lines: &PriceLineManager,
        viewport: &Viewport,
        _style: &ChartStyle,
        _pane_css_w: f64,
        _pane_css_h: f64,
    ) {
        if price_lines.is_empty() {
            return;
        }

        let dpr = self.dpr;
        let pane_pw = self.pw as f64;
        let pane_ph = self.ph as f64;

        for line in price_lines.iter() {
            if !line.is_visible() {
                continue;
            }

            let opts = &line.options;
            let y_phys = price_to_pane_y_phys(opts.price, viewport, pane_ph);

            if y_phys < 0.0 || y_phys > pane_ph {
                continue;
            }

            let line_w = (opts.line_width * dpr).round().max(1.0);
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

            // Set dash pattern based on line style
            set_canvas_line_dash(&self.ctx, opts.line_style, line_w);

            // Highlight if hovered
            let color = if line.hovered {
                // Brighten color slightly when hovered
                [
                    (opts.color[0] * 1.2).min(1.0),
                    (opts.color[1] * 1.2).min(1.0),
                    (opts.color[2] * 1.2).min(1.0),
                    opts.color[3],
                ]
            } else {
                opts.color
            };

            self.ctx.set_stroke_style_str(&rgba(&color));
            self.ctx.set_line_width(line_w);
            self.ctx.set_line_cap("butt");

            let y = y_phys.round() + correction;
            self.ctx.begin_path();
            self.ctx.move_to(0.0, y);
            self.ctx.line_to(pane_pw, y);
            self.ctx.stroke();
        }

        // Reset dash
        clear_canvas_line_dash(&self.ctx);
    }

    fn draw_crosshair(&self, ch: &CrosshairState, style: &ChartStyle, pane_w: f64, pane_h: f64) {
        if !ch.active {
            return;
        }

        let dpr = self.dpr;
        let mx = ch.x * dpr;
        let my = ch.y * dpr;

        let vert_in_bounds = mx >= 0.0 && mx <= pane_w;
        let horz_in_bounds = my >= 0.0 && my <= pane_h;
        if !vert_in_bounds && !horz_in_bounds {
            return;
        }

        self.ctx.set_line_cap("butt");

        if style.crosshair_horz_line.visible && horz_in_bounds {
            let line_w = (style.crosshair_horz_line.width * dpr).floor().max(1.0);
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
            self.ctx
                .set_stroke_style_str(&rgba(&style.crosshair_horz_line.color));
            self.ctx.set_line_width(line_w);
            set_canvas_line_dash(&self.ctx, style.crosshair_horz_line.style, line_w);

            let hy = my.round() + correction;
            let span = line_w + 1.0;
            self.ctx.begin_path();
            self.ctx.move_to(-span, hy);
            self.ctx.line_to(pane_w + span, hy);
            self.ctx.stroke();
        }

        if style.crosshair_vert_line.visible && vert_in_bounds {
            let line_w = (style.crosshair_vert_line.width * dpr).floor().max(1.0);
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
            self.ctx
                .set_stroke_style_str(&rgba(&style.crosshair_vert_line.color));
            self.ctx.set_line_width(line_w);
            set_canvas_line_dash(&self.ctx, style.crosshair_vert_line.style, line_w);

            let vx = mx.round() + correction;
            let span = line_w + 1.0;
            self.ctx.begin_path();
            self.ctx.move_to(vx, -span);
            self.ctx.line_to(vx, pane_h + span);
            self.ctx.stroke();
        }

        clear_canvas_line_dash(&self.ctx);
    }

    /// Render crosshair marker circles at the intersection with line/area/baseline series.
    ///
    /// LWC pattern: two-pass rendering — border ring first, then fill dot.
    /// The marker appears at the crosshair X position, Y determined by interpolating
    /// the series data at that bar index.
    pub fn render_crosshair_markers(
        &self,
        crosshair: &CrosshairState,
        series: &SeriesCollection,
        bars: &crate::core::data::BarArray,
        _bar_timestamps: &[u64],
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
    ) {
        if !crosshair.active {
            return;
        }

        let dpr = self.dpr;
        let _pane_pw = pane_css_w * dpr;
        let pane_ph = pane_css_h * dpr;

        // Get bar index at crosshair X
        let bar_idx = match crosshair.bar_index {
            Some(i) => i,
            None => return,
        };

        // Marker styling (LWC defaults)
        let radius = 4.0 * dpr;
        let border_width = 1.0 * dpr;

        // For each visible line/area/baseline series, find the value at bar_idx
        for s in series.iter() {
            if !s.is_visible() {
                continue;
            }
            match s.series_type() {
                SeriesType::Line | SeriesType::Area | SeriesType::Baseline => {}
                _ => continue,
            }

            // Find the data point at or near bar_idx by matching timestamp
            let target_ts = if bar_idx < bars.len() {
                bars.timestamp(bar_idx)
            } else {
                continue;
            };

            // Linear search for matching timestamp in series data
            let data = &s.line_data;
            if data.is_empty() {
                continue;
            }

            let value = {
                let mut found: Option<f64> = None;
                for i in 0..data.len() {
                    if data.timestamps[i] == target_ts {
                        found = Some(data.values[i] as f64);
                        break;
                    }
                }
                match found {
                    Some(v) => v,
                    None => continue,
                }
            };

            let color = s.series_color();
            let y_css = viewport.price_to_css_y(value, pane_css_h);
            let y_phys = y_css * dpr;

            if y_phys < 0.0 || y_phys > pane_ph {
                continue;
            }

            let x_phys = crosshair.x * dpr;

            // Two-pass rendering: border ring then fill
            // Pass 1: border ring (white or contrasting color)
            self.ctx.begin_path();
            let _ = self.ctx.arc(
                x_phys,
                y_phys,
                radius + border_width,
                0.0,
                std::f64::consts::TAU,
            );
            self.ctx.set_fill_style_str(&rgba(&style.bg_color));
            self.ctx.fill();

            // Pass 2: fill dot (series color)
            self.ctx.begin_path();
            let _ = self
                .ctx
                .arc(x_phys, y_phys, radius, 0.0, std::f64::consts::TAU);
            self.ctx.set_fill_style_str(&rgba(&color));
            self.ctx.fill();
        }
    }

    /// Render series markers (arrows, circles, squares) positioned at bar indices.
    ///
    /// LWC's setMarkers() API:
    /// - Shapes: arrowUp, arrowDown, circle, square
    /// - Position: aboveBar, belowBar, atPrice
    /// - Two-pass rendering for circles (border ring then fill)
    pub fn render_markers(
        &self,
        markers: &MarkerManager,
        bars: &BarArray,
        viewport: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
        pane_css_h: f64,
    ) {
        let dpr = self.dpr;
        let pane_ph = pane_css_h * dpr;

        // Calculate visible bar range
        let bar_count = bars.len();
        if bar_count == 0 {
            return;
        }

        let start_idx = (viewport.start_bar.floor() as usize).min(bar_count.saturating_sub(1));
        let end_idx = (viewport.end_bar.ceil() as usize).min(bar_count.saturating_sub(1));

        // Calculate bar spacing
        let visible_bars = (viewport.end_bar - viewport.start_bar).max(1.0);
        let bar_spacing_css = pane_css_w / visible_bars;

        // Collect all visible markers for two-pass rendering
        struct MarkerDraw {
            x_phys: f64,
            y_phys: f64,
            shape: MarkerShape,
            color: [f32; 4],
            size: f64,
            text: String,
            text_color: [f32; 4],
        }

        let mut to_draw: Vec<MarkerDraw> = Vec::new();

        for (_series_id, series_markers) in markers.iter() {
            let visible = series_markers.in_range(start_idx, end_idx);

            for marker in visible {
                // Calculate X position from bar index
                let bar_offset = marker.bar_index as f64 - viewport.start_bar;
                let x_css = bar_offset * bar_spacing_css + bar_spacing_css * 0.5;
                let x_phys = x_css * dpr;

                if x_phys < 0.0 || x_phys > pane_css_w * dpr {
                    continue;
                }

                // Calculate Y position based on marker position
                let y_price: f64 = match marker.position {
                    MarkerPosition::AboveBar => {
                        // Above the bar's high
                        bars.high(marker.bar_index) as f64
                    }
                    MarkerPosition::BelowBar => {
                        // Below the bar's low
                        bars.low(marker.bar_index) as f64
                    }
                    MarkerPosition::AtPrice => marker.price,
                };

                let y_css = viewport.price_to_css_y(y_price, pane_css_h);

                // Add offset for above/below positioning
                let y_offset = match marker.position {
                    MarkerPosition::AboveBar => -(marker.size + 4.0), // above the high
                    MarkerPosition::BelowBar => marker.size + 4.0,    // below the low
                    MarkerPosition::AtPrice => 0.0,
                };

                let y_phys = (y_css + y_offset) * dpr;

                if y_phys < 0.0 || y_phys > pane_ph {
                    continue;
                }

                to_draw.push(MarkerDraw {
                    x_phys,
                    y_phys,
                    shape: marker.shape,
                    color: marker.color,
                    size: marker.size * dpr,
                    text: marker.text.clone(),
                    text_color: marker.text_color,
                });
            }
        }

        // Two-pass rendering for circles: first all border rings, then all fills
        // This ensures fills are always on top of all borders

        // Pass 1: Draw border rings for circles
        let border_width = 2.0 * dpr;
        for m in &to_draw {
            if m.shape == MarkerShape::Circle {
                self.ctx.begin_path();
                let _ = self.ctx.arc(
                    m.x_phys,
                    m.y_phys,
                    m.size + border_width,
                    0.0,
                    std::f64::consts::TAU,
                );
                self.ctx.set_fill_style_str(&rgba(&style.bg_color));
                self.ctx.fill();
            }
        }

        // Pass 2: Draw all shapes
        for m in &to_draw {
            let color_str = rgba(&m.color);
            self.ctx.set_fill_style_str(&color_str);
            self.ctx.set_stroke_style_str(&color_str);
            self.ctx.set_line_width(2.0 * dpr);

            match m.shape {
                MarkerShape::Circle => {
                    self.ctx.begin_path();
                    let _ = self
                        .ctx
                        .arc(m.x_phys, m.y_phys, m.size, 0.0, std::f64::consts::TAU);
                    self.ctx.fill();
                }
                MarkerShape::Square => {
                    let half = m.size;
                    self.ctx
                        .fill_rect(m.x_phys - half, m.y_phys - half, half * 2.0, half * 2.0);
                }
                MarkerShape::ArrowUp => {
                    // Triangle pointing up
                    let h = m.size * 1.5;
                    let w = m.size;
                    self.ctx.begin_path();
                    self.ctx.move_to(m.x_phys, m.y_phys - h); // top
                    self.ctx.line_to(m.x_phys - w, m.y_phys + h * 0.5); // bottom left
                    self.ctx.line_to(m.x_phys + w, m.y_phys + h * 0.5); // bottom right
                    self.ctx.close_path();
                    self.ctx.fill();
                }
                MarkerShape::ArrowDown => {
                    // Triangle pointing down
                    let h = m.size * 1.5;
                    let w = m.size;
                    self.ctx.begin_path();
                    self.ctx.move_to(m.x_phys, m.y_phys + h); // bottom
                    self.ctx.line_to(m.x_phys - w, m.y_phys - h * 0.5); // top left
                    self.ctx.line_to(m.x_phys + w, m.y_phys - h * 0.5); // top right
                    self.ctx.close_path();
                    self.ctx.fill();
                }
            }

            // Draw text label if present
            if !m.text.is_empty() {
                let font_size = style.font_size as f64 * dpr;
                let font = format!("{}px {}", font_size, style.font_family);
                self.ctx.set_font(&font);
                self.ctx.set_text_align("center");
                self.ctx.set_text_baseline("middle");
                self.ctx.set_fill_style_str(&rgba(&m.text_color));

                // Text positioned below/above the shape
                let text_y = match m.shape {
                    MarkerShape::ArrowUp => m.y_phys + m.size * 2.0 + font_size,
                    MarkerShape::ArrowDown => m.y_phys - m.size * 2.0 - font_size * 0.5,
                    _ => m.y_phys + m.size + font_size,
                };

                let _ = self.ctx.fill_text(&m.text, m.x_phys, text_y);
            }
        }
    }
}
