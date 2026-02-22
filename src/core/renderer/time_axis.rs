//! TimeAxisRenderer — dedicated renderer for the time (X) axis widget.
//!
//! Mirrors LWC's TimeAxisWidget:
//! - Base canvas: background, border, tick marks, tick labels (normal + bold)
//! - Top canvas: crosshair time label (rounded rect + text)
//!
//! Each canvas is sized to the time axis container only.

#![cfg(target_arch = "wasm32")]

use crate::core::formatters::format_crosshair_time;
use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::text_cache::TextWidthCache;
use crate::core::renderer::traits::{ChartStyle, CrosshairState, TickMark};
use crate::core::viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct TimeAxisRenderer {
    base_canvas: HtmlCanvasElement,
    base_ctx: CanvasRenderingContext2d,
    top_canvas: HtmlCanvasElement,
    top_ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
    /// Shared text width cache for crosshair time label.
    text_cache: TextWidthCache,
}

impl TimeAxisRenderer {
    pub fn new(
        base_canvas: HtmlCanvasElement,
        top_canvas: HtmlCanvasElement,
        dpr: f64,
    ) -> Result<Self, String> {
        let base_ctx = get_2d_ctx(&base_canvas, "time-axis base")?;
        let top_ctx = get_2d_ctx(&top_canvas, "time-axis top")?;
        let pw = base_canvas.width();
        let ph = base_canvas.height();
        Ok(Self {
            base_canvas,
            base_ctx,
            top_canvas,
            top_ctx,
            pw,
            ph,
            dpr,
            text_cache: TextWidthCache::new(50),
        })
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        self.base_canvas.set_width(pw.max(1));
        self.base_canvas.set_height(ph.max(1));
        self.top_canvas.set_width(pw.max(1));
        self.top_canvas.set_height(ph.max(1));
        self.base_ctx.set_image_smoothing_enabled(false);
        self.top_ctx.set_image_smoothing_enabled(false);
    }

    /// Render the base layer: background, border, tick marks, tick labels.
    /// `pane_w` is the pane width in physical pixels (only draw ticks within this range).
    pub fn render_base(&mut self, style: &ChartStyle, ticks: &[TickMark], pane_w: f64) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;

        // Clear + background
        self.base_ctx.clear_rect(0.0, 0.0, w, h);
        self.base_ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.base_ctx.fill_rect(0.0, 0.0, w, h);

        // Border line at top edge (LWC: time axis border is at its top)
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_border_color));
        self.base_ctx
            .fill_rect(0.0, 0.0, pane_w.min(w), border_size);

        // Tick marks (small vertical bars below the border)
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let tick_width = (1.0 * dpr).floor().max(1.0);
        let tick_offset = (dpr * 0.5).floor();

        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_border_color));
        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_w {
                continue;
            }
            let x = t.pixel.round();
            self.base_ctx
                .fill_rect(x - tick_offset, 0.0, tick_width, tick_length);
        }

        // Tick labels — draw in media (CSS) coordinate space for sharp text.
        self.base_ctx.save();
        let _ = self.base_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        let css_font_normal = format!("{}px {}", style.font_size, style.font_family);
        let css_font_bold = format!("bold {}px {}", style.font_size, style.font_family);
        self.base_ctx
            .set_fill_style_str(&rgba(&style.axis_text_color));
        self.base_ctx.set_text_align("center");
        self.base_ctx.set_text_baseline("alphabetic");

        let padding_top_css = style.time_axis_padding_top();
        let fs_css = style.font_size as f64;
        let text_y_css = (border_size + tick_length) / dpr + padding_top_css + fs_css / 2.0;

        for t in ticks {
            if t.pixel < 0.0 || t.pixel > pane_w {
                continue;
            }
            let font_key = if t.major {
                &css_font_bold
            } else {
                &css_font_normal
            };
            if t.major {
                self.base_ctx.set_font(&css_font_bold);
            } else {
                self.base_ctx.set_font(&css_font_normal);
            }
            let x_css = t.pixel / dpr;
            let m = self
                .text_cache
                .measure_full(&self.base_ctx, &t.label, font_key);
            let _ = self
                .base_ctx
                .fill_text(&t.label, x_css, text_y_css + m.y_mid_correction);
        }
        self.base_ctx.restore();
    }

    /// Render the top layer: crosshair time label.
    /// `crosshair_x` is in CSS px relative to the pane.
    pub fn render_top(
        &mut self,
        crosshair: &CrosshairState,
        bars: &crate::core::data::BarArray,
        vp: &Viewport,
        style: &ChartStyle,
        pane_css_w: f64,
    ) {
        let w = self.pw as f64;
        let h = self.ph as f64;
        let dpr = self.dpr;

        self.top_ctx.clear_rect(0.0, 0.0, w, h);

        if !crosshair.active {
            return;
        }

        let mx = crosshair.x * dpr; // physical X in pane space
        let pane_w = pane_css_w * dpr;
        if mx < 0.0 || mx > pane_w {
            return;
        }

        // Bar index at crosshair X — use floor() not round() (matches interaction.rs fix)
        let bar_idx = vp.bar_index_at_pixel(mx, pane_w, bars.len());
        let bar_i = bar_idx.unwrap_or(0);
        let bar_lbl = if bar_idx.is_some() && bars.timestamps.value(bar_i) > 0 {
            format_crosshair_time(bars.timestamps.value(bar_i))
        } else {
            format!("{}", bar_i)
        };

        let font = style.axis_font(dpr);
        self.top_ctx.set_font(&font);

        let text_w = self
            .text_cache
            .measure(&self.top_ctx, &bar_lbl, &font)
            .round();
        let h_margin = style.time_axis_padding_horizontal() * dpr;
        let label_w = text_w + 2.0 * h_margin;
        let label_half = label_w / 2.0;

        // Center on crosshair X, clamp to bounds
        let mut coord = mx;
        let mut lx1 = (coord - label_half).floor();
        if lx1 < 0.0 {
            coord += -lx1;
            lx1 = (coord - label_half).floor();
        } else if lx1 + label_w > pane_w {
            coord -= (lx1 + label_w) - pane_w;
            lx1 = (coord - label_half).floor();
        }
        let lx2 = lx1 + label_w;

        // Label height = borderSize + tickLength + paddingTop + fontSize + paddingBottom (all in physical px)
        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();
        let tick_length = (style.axis_tick_length as f64 * dpr).round();
        let padding_top = style.time_axis_padding_top() * dpr;
        let padding_bottom = style.time_axis_padding_bottom() * dpr;
        let fs = style.font_size as f64 * dpr;
        let label_h = (border_size + tick_length + padding_top + fs + padding_bottom).ceil();

        let by1 = 0.0;
        let by2 = label_h.min(h);
        let radius = (2.0 * dpr).round();

        // Rounded rect: top corners square, bottom corners rounded
        self.top_ctx
            .set_fill_style_str(&rgba(&style.crosshair_label_bg));
        self.top_ctx.begin_path();
        self.top_ctx.move_to(lx1, by1);
        self.top_ctx.line_to(lx2, by1);
        self.top_ctx.line_to(lx2, by2 - radius);
        let _ = self.top_ctx.arc_to(lx2, by2, lx2 - radius, by2, radius);
        self.top_ctx.line_to(lx1 + radius, by2);
        let _ = self.top_ctx.arc_to(lx1, by2, lx1, by2 - radius, radius);
        self.top_ctx.line_to(lx1, by1);
        self.top_ctx.close_path();
        self.top_ctx.fill();

        // Time tick mark
        let tick_w_px = (1.0 * dpr).floor().max(1.0);
        let tick_off_x = (dpr * 0.5).floor();
        let tick_x = coord.round();
        self.top_ctx
            .set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx
            .fill_rect(tick_x - tick_off_x, 0.0, tick_w_px, tick_length);

        // Time text — draw in media (CSS) coordinate space for sharp text.
        self.top_ctx.save();
        let _ = self.top_ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        let css_font = format!("{}px {}", style.font_size, style.font_family);
        self.top_ctx.set_font(&css_font);
        self.top_ctx
            .set_fill_style_str(&rgba(&style.crosshair_label_text));
        self.top_ctx.set_text_align("left");
        self.top_ctx.set_text_baseline("alphabetic");
        let text_x_css = (lx1 + h_margin) / dpr;
        let text_y_css = (border_size + tick_length + padding_top + fs / 2.0) / dpr;
        // yMidCorrection already computed during width measurement above
        let m = self
            .text_cache
            .measure_full(&self.top_ctx, &bar_lbl, &css_font);
        let _ = self
            .top_ctx
            .fill_text(&bar_lbl, text_x_css, text_y_css + m.y_mid_correction);
        self.top_ctx.restore();
    }

    /// Render a scrollbar indicator showing the visible range within the total data.
    ///
    /// The scrollbar appears as a thin track at the very top of the time axis,
    /// with a highlighted thumb showing the currently visible portion.
    ///
    /// - `viewport`: Current viewport with start_bar, end_bar
    /// - `total_bars`: Total number of bars in the dataset
    /// - `pane_w`: Width of the pane area in physical pixels
    pub fn render_scrollbar(
        &self,
        style: &ChartStyle,
        viewport: &Viewport,
        total_bars: usize,
        pane_w: f64,
    ) {
        if total_bars == 0 {
            return;
        }

        let dpr = self.dpr;
        let w = pane_w.min(self.pw as f64);

        // Scrollbar dimensions
        let track_height = (3.0 * dpr).round().max(2.0);
        let track_y = 0.0; // At the very top, overlapping the border slightly

        // Calculate thumb position and size
        let total = total_bars as f64;
        // Add some margin for scrolling past the end
        let scrollable_range = total + 50.0; // Same margin as keyboard scroll

        let visible_start = viewport.start_bar.max(0.0);
        let visible_end = viewport.end_bar;
        let visible_bars = visible_end - visible_start;

        // Thumb position as fraction of total scrollable range
        let thumb_start_frac = visible_start / scrollable_range;
        let thumb_width_frac = (visible_bars / scrollable_range).min(1.0);

        let thumb_x = (thumb_start_frac * w).round();
        let thumb_w = (thumb_width_frac * w).round().max(20.0 * dpr); // Minimum thumb width

        // Draw track background (semi-transparent)
        let track_color = [
            style.grid_color[0],
            style.grid_color[1],
            style.grid_color[2],
            0.3,
        ];
        self.base_ctx.set_fill_style_str(&rgba(&track_color));
        self.base_ctx.fill_rect(0.0, track_y, w, track_height);

        // Draw thumb (highlighted, more opaque)
        let thumb_color = [
            style.axis_text_color[0],
            style.axis_text_color[1],
            style.axis_text_color[2],
            0.5,
        ];
        self.base_ctx.set_fill_style_str(&rgba(&thumb_color));

        // Round the thumb corners
        let radius = (track_height / 2.0).min(3.0 * dpr);
        self.base_ctx.begin_path();
        let _ = self
            .base_ctx
            .round_rect_with_f64(thumb_x, track_y, thumb_w, track_height, radius);
        self.base_ctx.fill();
    }
}

fn get_2d_ctx(canvas: &HtmlCanvasElement, label: &str) -> Result<CanvasRenderingContext2d, String> {
    let ctx = canvas
        .get_context("2d")
        .map_err(|e| format!("{} get_context('2d') failed: {:?}", label, e))?
        .ok_or(format!("{} get_context('2d') returned None", label))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|_| format!("{} context is not CanvasRenderingContext2d", label))?;
    ctx.set_image_smoothing_enabled(false);
    Ok(ctx)
}
