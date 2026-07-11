//! The chart object exported to JS.
//!
//! Layout (single pane, right price axis, bottom time axis — multi-pane comes with the
//! pane abstraction):
//!
//! ```text
//! +-------------------------------+--------+
//! |             pane              | price  |
//! |   (grid, series, crosshair)   |  axis  |
//! +-------------------------------+--------+
//! |           time axis           |  stub  |
//! +-------------------------------+--------+
//! ```

use wasm_bindgen::prelude::*;

use aion_core::format::price_formatter::PriceFormatter;
use aion_core::format::time_formatter::{
    format_crosshair_time, format_tick_label, weight_to_tick_mark_type,
};
use aion_core::model::plot_list::{PlotList, PlotValueIndex};
use aion_core::model::price_range::PriceRange;
use aion_core::scale::price_scale_core::{PriceScaleCore, PriceScaleCoreOptions};
use aion_core::scale::time_scale_core::{TimeScaleCore, TimeScaleOptions};
use aion_core::scale::time_tick_marks::{fill_weights_for_points, TimeTickMarks};
use aion_render::bars::{build_bars, BarItem, BarsParams};
use aion_render::candles::{build_candles, CandleItem, CandlesParams};
use aion_render::color::Color;
use aion_render::draw_list::{LineStyle, Prim};
use aion_render_wgpu::{
    prims_to_instances, render_frame, AtlasSlot, DrawGroup, LabelAtlas, QuadInstance,
    QuadRenderer, TexQuadInstance, TexQuadRenderer,
};

use crate::text::TextPainter;

// lightweight-charts default palette (RENDERING_SPEC.md §2.5, §7, §8, §15)
const UP_COLOR: Color = Color::rgb(0x26, 0xa6, 0x9a);
const DOWN_COLOR: Color = Color::rgb(0xef, 0x53, 0x50);
const GRID_COLOR: Color = Color::rgb(0xd6, 0xdc, 0xde);
const CROSSHAIR_COLOR: Color = Color::rgb(0x95, 0x98, 0xa1);
const BORDER_COLOR: Color = Color::rgb(0x2b, 0x2b, 0x43);
const LABEL_BG_COLOR: Color = Color::rgb(0x13, 0x17, 0x22);

const TEXT_COLOR: [f32; 4] = [0x19 as f32 / 255.0, 0x19 as f32 / 255.0, 0x19 as f32 / 255.0, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Axis metrics (RENDERING_SPEC.md §10, §11), font size 12.
const FONT_SIZE: f64 = 12.0;
const AXIS_BORDER_SIZE: f64 = 1.0;
const AXIS_TICK_LENGTH: f64 = 5.0;
const PRICE_PADDING_INNER: f64 = 5.0; // fontSize/12 * tickLength
const PRICE_PADDING_OUTER: f64 = 5.0;
const PRICE_LABEL_OFFSET: f64 = 5.0; // Constants.LabelOffset
const PRICE_DEFAULT_TEXT_WIDTH: f64 = 34.0; // Constants.DefaultOptimalWidth
const PRICE_LABEL_PADDING_TB: f64 = 2.5; // 2.5/12 * fontSize
const TIME_PADDING_TOP: f64 = 3.0; // 3/12 * fontSize
const TIME_PADDING_BOTTOM: f64 = 3.0;
const TIME_PADDING_HORZ: f64 = 9.0; // 9/12 * fontSize
const TIME_LABEL_BOTTOM_OFFSET: f64 = 4.0; // 4/12 * fontSize
const TICK_MARK_MAX_CHARS: f64 = 8.0;

/// optimalHeight = ceil(border + tick + fontSize + padTop + padBottom + labelBottomOffset),
/// snapped even -> 28.
const TIME_AXIS_HEIGHT: f64 = 28.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SeriesKind {
    Candlestick,
    Bar,
}

struct Gfx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    quad_renderer: QuadRenderer,
    tex_renderer: TexQuadRenderer,
}

#[wasm_bindgen]
pub struct AionChart {
    gfx: Gfx,
    atlas: LabelAtlas,
    text: TextPainter,
    time_scale: TimeScaleCore,
    price_scale: PriceScaleCore,
    price_formatter: PriceFormatter,
    plot_list: PlotList,
    times: Vec<i64>,
    tick_marks: TimeTickMarks,
    series_kind: SeriesKind,
    time_visible: bool,
    css_width: f64,
    css_height: f64,
    dpr: f64,
    crosshair: Option<(f64, f64)>,
}

/// Creates a chart bound to `canvas`. The canvas' bitmap size (`canvas.width/height`) must
/// already be set to css size * dpr by the caller.
#[wasm_bindgen]
pub async fn create_chart(
    canvas: web_sys::HtmlCanvasElement,
    css_width: f64,
    css_height: f64,
    dpr: f64,
) -> Result<AionChart, JsValue> {
    console_error_panic_hook::set_once();

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|e| JsValue::from_str(&format!("create_surface failed: {e}")))?;

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|e| JsValue::from_str(&format!("request_adapter failed: {e}")))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|e| JsValue::from_str(&format!("request_device failed: {e}")))?;

    let bitmap_w = (css_width * dpr).round().max(1.0) as u32;
    let bitmap_h = (css_height * dpr).round().max(1.0) as u32;

    let config = surface
        .get_default_config(&adapter, bitmap_w, bitmap_h)
        .ok_or_else(|| JsValue::from_str("surface not supported by adapter"))?;
    surface.configure(&device, &config);

    let quad_renderer = QuadRenderer::new(&device, config.format);
    let atlas = LabelAtlas::new(&device);
    let tex_renderer = TexQuadRenderer::new(&device, config.format, atlas.view());
    let text = TextPainter::new()?;

    let mut time_scale = TimeScaleCore::new(TimeScaleOptions::default());
    time_scale.set_width(css_width);

    let mut price_scale = PriceScaleCore::new(PriceScaleCoreOptions::default());
    price_scale.set_height(css_height);

    Ok(AionChart {
        gfx: Gfx { device, queue, surface, config, quad_renderer, tex_renderer },
        atlas,
        text,
        time_scale,
        price_scale,
        price_formatter: PriceFormatter::default(),
        plot_list: PlotList::new(),
        times: Vec::new(),
        tick_marks: TimeTickMarks::new(),
        series_kind: SeriesKind::Candlestick,
        time_visible: true,
        css_width,
        css_height,
        dpr,
        crosshair: None,
    })
}

#[wasm_bindgen]
impl AionChart {
    /// `times` are UTC timestamps in seconds, ascending. All arrays must be equal length.
    pub fn set_data(&mut self, times: &[f64], open: &[f64], high: &[f64], low: &[f64], close: &[f64]) {
        let n = times.len();
        assert!(
            n == open.len() && n == high.len() && n == low.len() && n == close.len(),
            "time/OHLC arrays must have equal length"
        );

        self.times = times.iter().map(|&t| t as i64).collect();

        let mut weights = vec![0u8; n];
        fill_weights_for_points(&self.times, &mut weights, 0);
        self.tick_marks.set_weights(&weights);

        self.plot_list.set_data(
            (0..n as i64).collect(),
            open.to_vec(),
            high.to_vec(),
            low.to_vec(),
            close.to_vec(),
        );

        self.time_scale.set_points_len(n);
        self.time_scale.set_base_index(if n == 0 { None } else { Some(n as i64 - 1) });
    }

    /// Streaming update: replaces the last bar or appends a new one (time must be >= last).
    pub fn update_bar(&mut self, time: f64, open: f64, high: f64, low: f64, close: f64) {
        let time = time as i64;
        let is_new = self.times.last().is_none_or(|&last| time > last);

        if is_new {
            self.times.push(time);
            let n = self.times.len();
            let mut weights = vec![0u8; n];
            fill_weights_for_points(&self.times, &mut weights, 0);
            self.tick_marks.set_weights(&weights);
            self.plot_list.upsert_last(n as i64 - 1, [open, high, low, close]);
            self.time_scale.set_points_len(n);
            self.time_scale.set_base_index(Some(n as i64 - 1));
        } else {
            let last_index = self.times.len() as i64 - 1;
            self.plot_list.upsert_last(last_index, [open, high, low, close]);
        }
    }

    /// 0 = candlestick, 1 = OHLC bars.
    pub fn set_series_type(&mut self, kind: u8) {
        self.series_kind = if kind == 1 { SeriesKind::Bar } else { SeriesKind::Candlestick };
    }

    /// Show intraday time in tick labels and the crosshair label (LWC `timeScale.timeVisible`).
    pub fn set_time_visible(&mut self, visible: bool) {
        self.time_visible = visible;
    }

    pub fn resize(&mut self, css_width: f64, css_height: f64, dpr: f64) {
        self.css_width = css_width;
        self.css_height = css_height;
        self.dpr = dpr;

        let bitmap_w = (css_width * dpr).round().max(1.0) as u32;
        let bitmap_h = (css_height * dpr).round().max(1.0) as u32;
        self.gfx.config.width = bitmap_w;
        self.gfx.config.height = bitmap_h;
        self.gfx.surface.configure(&self.gfx.device, &self.gfx.config);
    }

    // --- gestures (called from JS event handlers with pane-local css coordinates) ---

    pub fn zoom(&mut self, x_css: f64, scale: f64) {
        let x = x_css.max(1.0).min(self.time_scale.width());
        self.time_scale.zoom(x, scale);
    }

    pub fn scroll_start(&mut self, x_css: f64) {
        self.time_scale.start_scroll(x_css);
    }

    pub fn scroll_move(&mut self, x_css: f64) {
        self.time_scale.scroll_to(x_css);
    }

    pub fn scroll_end(&mut self) {
        self.time_scale.end_scroll();
    }

    pub fn fit_content(&mut self) {
        self.time_scale.fit_content();
    }

    pub fn set_crosshair(&mut self, x_css: f64, y_css: f64) {
        self.crosshair = Some((x_css, y_css));
    }

    pub fn clear_crosshair(&mut self) {
        self.crosshair = None;
    }

    pub fn bar_spacing(&self) -> f64 {
        self.time_scale.bar_spacing()
    }

    // --- rendering ---

    pub fn render(&mut self) -> Result<(), JsValue> {
        let hpr = self.dpr;
        let vpr = self.dpr;

        // ---- layout: time axis fixed height; price axis width negotiated (its labels
        // depend only on the price range, so one refinement pass converges) ----
        let pane_h = (self.css_height - TIME_AXIS_HEIGHT).max(1.0);
        self.price_scale.set_height(pane_h);

        let mut axis_w = self.compute_price_axis_width();
        for _ in 0..2 {
            let pane_w = (self.css_width - axis_w).max(1.0);
            self.time_scale.set_width(pane_w);
            if let Some((from, to)) = self.visible_data_range() {
                self.autoscale(from, to);
            }
            let new_w = self.compute_price_axis_width();
            if new_w == axis_w {
                break;
            }
            axis_w = new_w;
        }
        let pane_w = (self.css_width - axis_w).max(1.0);

        let pane_w_px = (pane_w * hpr).round() as u32;
        let pane_h_px = (pane_h * vpr).round() as u32;

        // ---- pane group (scissored) ----
        let mut pane_group = DrawGroup {
            scissor: Some([0, 0, pane_w_px, pane_h_px]),
            ..Default::default()
        };
        let mut pane_prims: Vec<Prim> = Vec::new();

        let visible = self.visible_data_range();
        if let Some((from, to)) = visible {
            self.build_grid(&mut pane_prims, from, to, pane_w_px as i32, pane_h_px as i32, hpr, vpr);
            match self.series_kind {
                SeriesKind::Candlestick => self.build_candle_prims(&mut pane_prims, from, to, hpr, vpr),
                SeriesKind::Bar => self.build_bar_prims(&mut pane_prims, from, to, hpr, vpr),
            }
        }
        self.build_crosshair_lines(&mut pane_prims, pane_w_px as i32, pane_h_px as i32, pane_w, pane_h, hpr, vpr);
        prims_to_instances(&pane_prims, &mut pane_group.quads);

        // ---- axis group (unscissored) ----
        let mut axis_group = DrawGroup::default();
        self.build_axes(&mut axis_group, pane_w, pane_h, pane_w_px, pane_h_px, hpr, vpr);
        if let Some((from, to)) = visible {
            self.build_time_axis_labels(&mut axis_group, from, to, pane_h, hpr, vpr);
        }
        self.build_crosshair_labels(&mut axis_group, pane_w, pane_h, hpr, vpr);

        // ---- submit ----
        let frame = self
            .gfx
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("get_current_texture failed: {e}")))?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        render_frame(
            &self.gfx.device,
            &self.gfx.queue,
            &view,
            self.gfx.config.width,
            self.gfx.config.height,
            wgpu::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }, // layout.background default #FFFFFF
            &self.gfx.quad_renderer,
            &self.gfx.tex_renderer,
            &[pane_group, axis_group],
        );

        frame.present();
        Ok(())
    }

    fn visible_data_range(&self) -> Option<(i64, i64)> {
        if self.plot_list.is_empty() || self.time_scale.is_empty() {
            return None;
        }
        let range = self.time_scale.visible_strict_range()?;
        let from = range.left().max(0);
        let to = range.right().min(self.plot_list.size() as i64 - 1);
        if from > to {
            return None;
        }
        Some((from, to))
    }

    fn autoscale(&mut self, from: i64, to: i64) {
        if let Some(mm) = self.plot_list.min_max_on_range_cached(
            from,
            to,
            &[PlotValueIndex::Low, PlotValueIndex::High],
        ) {
            self.price_scale
                .apply_autoscale_range(Some(PriceRange::new(mm.min, mm.max)), 0.01);
        }
    }

    /// Port of `PriceAxisWidget.optimalWidth()` (RENDERING_SPEC.md §10).
    fn compute_price_axis_width(&mut self) -> f64 {
        let mut max_text_w = 0f64;
        for mark in self.price_scale.build_tick_marks(100, 0.0) {
            let label = self.price_formatter.format(mark.logical);
            max_text_w = max_text_w.max(self.text.measure(&label));
        }
        if let Some((_, y)) = self.crosshair {
            if !self.price_scale.is_empty() {
                let price = self.price_scale.coordinate_to_price(y, 0.0);
                let label = self.price_formatter.format(price);
                max_text_w = max_text_w.max(self.text.measure(&label));
            }
        }
        let text_w = if max_text_w > 0.0 { max_text_w } else { PRICE_DEFAULT_TEXT_WIDTH };
        let w = (AXIS_BORDER_SIZE
            + AXIS_TICK_LENGTH
            + PRICE_PADDING_INNER
            + PRICE_PADDING_OUTER
            + PRICE_LABEL_OFFSET
            + text_w)
            .ceil();
        w + (w as i64 % 2) as f64 // suggestPriceScaleWidth: make even
    }

    /// Grid + axis borders and price labels.
    #[allow(clippy::too_many_arguments)]
    fn build_grid(
        &mut self,
        prims: &mut Vec<Prim>,
        from: i64,
        to: i64,
        pane_w_px: i32,
        pane_h_px: i32,
        hpr: f64,
        vpr: f64,
    ) {
        let line_width = 1f64.max(hpr.floor()) as i32;

        // vertical: time tick marks (maxLabelWidth density, RENDERING_SPEC.md §11)
        let pixels_per_character = (FONT_SIZE + 4.0) * 5.0 / 8.0;
        let max_label_width = pixels_per_character * TICK_MARK_MAX_CHARS;
        let spacing = self.time_scale.bar_spacing();
        for mark in self.tick_marks.build(spacing, max_label_width).to_vec() {
            if mark.index < from || mark.index > to {
                continue;
            }
            let x = (self.time_scale.index_to_coordinate(mark.index) * hpr).round() as i32;
            prims.push(Prim::VLine {
                x,
                y0: -line_width,
                y1: pane_h_px + line_width,
                width: line_width,
                style: LineStyle::Solid,
                color: GRID_COLOR,
            });
        }

        // horizontal: price marks
        for mark in self.price_scale.build_tick_marks(100, 0.0) {
            let y = (mark.coord * vpr).round() as i32;
            prims.push(Prim::HLine {
                y,
                x0: -line_width,
                x1: pane_w_px + line_width,
                width: line_width,
                style: LineStyle::Solid,
                color: GRID_COLOR,
            });
        }
    }

    fn build_candle_prims(&mut self, prims: &mut Vec<Prim>, from: i64, to: i64, hpr: f64, vpr: f64) {
        let count = (to - from + 1) as usize;
        let mut items: Vec<CandleItem> = Vec::with_capacity(count);

        let open_col = self.plot_list.column(PlotValueIndex::Open);
        let high_col = self.plot_list.column(PlotValueIndex::High);
        let low_col = self.plot_list.column(PlotValueIndex::Low);
        let close_col = self.plot_list.column(PlotValueIndex::Close);

        for i in from..=to {
            let idx = i as usize;
            let (open, high, low, close) = (open_col[idx], high_col[idx], low_col[idx], close_col[idx]);

            let color = if close >= open { UP_COLOR } else { DOWN_COLOR };
            let x = self.time_scale.index_to_coordinate(i);

            items.push(CandleItem {
                x,
                open_y: self.price_scale.price_to_coordinate(open, close),
                high_y: self.price_scale.price_to_coordinate(high, close),
                low_y: self.price_scale.price_to_coordinate(low, close),
                close_y: self.price_scale.price_to_coordinate(close, close),
                body_color: color,
                border_color: color,
                wick_color: color,
            });
        }

        build_candles(
            &items,
            &CandlesParams {
                bar_spacing: self.time_scale.bar_spacing(),
                horizontal_pixel_ratio: hpr,
                vertical_pixel_ratio: vpr,
                wick_visible: true,
                border_visible: true,
            },
            prims,
        );
    }

    fn build_bar_prims(&mut self, prims: &mut Vec<Prim>, from: i64, to: i64, hpr: f64, vpr: f64) {
        let count = (to - from + 1) as usize;
        let mut items: Vec<BarItem> = Vec::with_capacity(count);

        let open_col = self.plot_list.column(PlotValueIndex::Open);
        let high_col = self.plot_list.column(PlotValueIndex::High);
        let low_col = self.plot_list.column(PlotValueIndex::Low);
        let close_col = self.plot_list.column(PlotValueIndex::Close);

        for i in from..=to {
            let idx = i as usize;
            let (open, high, low, close) = (open_col[idx], high_col[idx], low_col[idx], close_col[idx]);

            items.push(BarItem {
                x: self.time_scale.index_to_coordinate(i),
                open_y: self.price_scale.price_to_coordinate(open, close),
                high_y: self.price_scale.price_to_coordinate(high, close),
                low_y: self.price_scale.price_to_coordinate(low, close),
                close_y: self.price_scale.price_to_coordinate(close, close),
                color: if close >= open { UP_COLOR } else { DOWN_COLOR },
            });
        }

        build_bars(
            &items,
            &BarsParams {
                bar_spacing: self.time_scale.bar_spacing(),
                horizontal_pixel_ratio: hpr,
                vertical_pixel_ratio: vpr,
                open_visible: true,
                thin_bars: true,
            },
            prims,
        );
    }

    /// Crosshair lines inside the pane (labels are drawn by `build_crosshair_labels`).
    #[allow(clippy::too_many_arguments)]
    fn build_crosshair_lines(
        &mut self,
        prims: &mut Vec<Prim>,
        pane_w_px: i32,
        pane_h_px: i32,
        pane_w: f64,
        pane_h: f64,
        hpr: f64,
        vpr: f64,
    ) {
        let Some((x_css, y_css)) = self.crosshair else { return };
        if self.plot_list.is_empty() || self.time_scale.is_empty() {
            return;
        }
        if x_css > pane_w || y_css > pane_h {
            return;
        }

        let snapped_x = self.snapped_crosshair_x(x_css);
        let line_width = 1f64.max(hpr.floor()) as i32;

        prims.push(Prim::VLine {
            x: (snapped_x * hpr).round() as i32,
            y0: 0,
            y1: pane_h_px,
            width: line_width,
            style: LineStyle::LargeDashed,
            color: CROSSHAIR_COLOR,
        });
        prims.push(Prim::HLine {
            y: (y_css * vpr).round() as i32,
            x0: 0,
            x1: pane_w_px,
            width: line_width,
            style: LineStyle::LargeDashed,
            color: CROSSHAIR_COLOR,
        });
    }

    fn snapped_crosshair_index(&self, x_css: f64) -> i64 {
        let mut index = self.time_scale.coordinate_to_index(x_css);
        if let Some(range) = self.time_scale.visible_strict_range() {
            index = index.clamp(range.left(), range.right());
        }
        index.clamp(0, self.plot_list.size() as i64 - 1)
    }

    fn snapped_crosshair_x(&self, x_css: f64) -> f64 {
        self.time_scale.index_to_coordinate(self.snapped_crosshair_index(x_css))
    }

    /// Axis borders + price tick labels.
    #[allow(clippy::too_many_arguments)]
    fn build_axes(
        &mut self,
        group: &mut DrawGroup,
        pane_w: f64,
        _pane_h: f64,
        pane_w_px: u32,
        pane_h_px: u32,
        hpr: f64,
        vpr: f64,
    ) {
        let full_w_px = self.gfx.config.width as f32;
        let border_h = 1f64.max((AXIS_BORDER_SIZE * hpr).floor()) as f32;

        // price axis border (left edge of the right axis)
        group.quads.push(QuadInstance {
            rect: [pane_w_px as f32, 0.0, border_h, pane_h_px as f32],
            color: color_rgba(BORDER_COLOR),
        });
        // time axis border (top edge)
        group.quads.push(QuadInstance {
            rect: [0.0, pane_h_px as f32, full_w_px, 1f64.max((AXIS_BORDER_SIZE * vpr).floor()) as f32],
            color: color_rgba(BORDER_COLOR),
        });

        // price tick labels: left-aligned at tickLength + paddingInner from the axis edge
        let text_x = pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER;
        let marks = self.price_scale.build_tick_marks(100, 0.0);
        for mark in marks {
            let label = self.price_formatter.format(mark.logical);
            self.push_label_left(group, &label, text_x, mark.coord, TEXT_COLOR, hpr, vpr);
        }
    }

    fn build_time_axis_labels(
        &mut self,
        group: &mut DrawGroup,
        from: i64,
        to: i64,
        pane_h: f64,
        hpr: f64,
        vpr: f64,
    ) {
        let pixels_per_character = (FONT_SIZE + 4.0) * 5.0 / 8.0;
        let max_label_width = pixels_per_character * TICK_MARK_MAX_CHARS;
        let spacing = self.time_scale.bar_spacing();

        // label vertical center: border + tick + paddingTop + fontSize/2
        let y_center =
            pane_h + AXIS_BORDER_SIZE + AXIS_TICK_LENGTH + TIME_PADDING_TOP + FONT_SIZE / 2.0;

        for mark in self.tick_marks.build(spacing, max_label_width).to_vec() {
            if mark.index < from || mark.index > to {
                continue;
            }
            let ts = self.times[mark.index as usize];
            let mark_type = weight_to_tick_mark_type(mark.weight, self.time_visible, false);
            let label = format_tick_label(ts, mark_type);
            let x_center = self.time_scale.index_to_coordinate(mark.index);
            self.push_label_centered(group, &label, x_center, y_center, TEXT_COLOR, hpr, vpr);
        }
    }

    /// Crosshair axis labels (RENDERING_SPEC.md §10): dark rounded box + white text.
    /// (Rounded corners r=2 land with the SDF pipeline; plain rects for now.)
    fn build_crosshair_labels(
        &mut self,
        group: &mut DrawGroup,
        pane_w: f64,
        pane_h: f64,
        hpr: f64,
        vpr: f64,
    ) {
        let Some((x_css, y_css)) = self.crosshair else { return };
        if self.plot_list.is_empty() || self.time_scale.is_empty() {
            return;
        }

        // --- price label on the right axis ---
        if y_css <= pane_h && !self.price_scale.is_empty() {
            let price = self.price_scale.coordinate_to_price(y_css, 0.0);
            let label = self.price_formatter.format(price);
            let text_w = self.text.measure(&label);

            // geometry per PriceAxisViewRenderer
            let total_h_media = FONT_SIZE + PRICE_LABEL_PADDING_TB * 2.0;
            let tick_h = 1f64.max(vpr.floor());
            let mut total_h = (total_h_media * vpr).round();
            if (total_h as i64) % 2 != (tick_h as i64) % 2 {
                total_h += 1.0;
            }
            let total_w = ((AXIS_BORDER_SIZE
                + PRICE_PADDING_INNER
                + PRICE_PADDING_OUTER
                + AXIS_TICK_LENGTH
                + text_w)
                * hpr)
                .round();

            let y_mid = (y_css * vpr).round() - (vpr * 0.5).floor();
            let y_top = (y_mid + tick_h / 2.0 - total_h / 2.0).floor();

            group.quads.push(QuadInstance {
                rect: [(pane_w * hpr).round() as f32, y_top as f32, total_w as f32, total_h as f32],
                color: color_rgba(LABEL_BG_COLOR),
            });

            let text_x = pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER;
            self.push_label_left(group, &label, text_x, y_css, WHITE, hpr, vpr);
        }

        // --- time label on the bottom axis ---
        if x_css <= pane_w {
            let index = self.snapped_crosshair_index(x_css);
            let ts = self.times[index as usize];
            let label = format_crosshair_time(ts, self.time_visible, false);
            let text_w = self.text.measure(&label);

            let box_w = ((text_w + TIME_PADDING_HORZ * 2.0) * hpr).round();
            let box_h = ((FONT_SIZE + TIME_PADDING_TOP + TIME_PADDING_BOTTOM) * vpr).round();
            let snapped_x = self.snapped_crosshair_x(x_css);
            let full_w = self.gfx.config.width as f64;
            let box_x = ((snapped_x * hpr).round() - box_w / 2.0).clamp(0.0, full_w - box_w);
            let box_y = (pane_h * vpr).round() + 1f64.max((AXIS_BORDER_SIZE * vpr).floor());

            group.quads.push(QuadInstance {
                rect: [box_x as f32, box_y as f32, box_w as f32, box_h as f32],
                color: color_rgba(LABEL_BG_COLOR),
            });

            let y_center = (box_y + box_h / 2.0) / vpr;
            let x_center = (box_x + box_w / 2.0) / hpr;
            self.push_label_centered(group, &label, x_center, y_center, WHITE, hpr, vpr);
        }
    }

    // --- text helpers ---

    fn label_slot(&mut self, text: &str, dpr: f64) -> AtlasSlot {
        let key = format!("{dpr:.3}|{text}");
        if let Some(slot) = self.atlas.get(&key) {
            return slot;
        }
        let (pixels, w, h) = self
            .text
            .rasterize(text, dpr)
            .unwrap_or_else(|_| (vec![0, 0, 0, 0], 1, 1));
        self.atlas.insert(&self.gfx.queue, key, w, h, &pixels)
    }

    /// Left-aligned at media x, vertically centered on media y.
    #[allow(clippy::too_many_arguments)]
    fn push_label_left(
        &mut self,
        group: &mut DrawGroup,
        text: &str,
        x_media: f64,
        y_center_media: f64,
        color: [f32; 4],
        hpr: f64,
        vpr: f64,
    ) {
        let slot = self.label_slot(text, hpr);
        let x = (x_media * hpr).round() - TextPainter::pad_px(hpr).round();
        let y = (y_center_media * vpr).round() - (slot.h / 2) as f64;
        group.tex_quads.push(TexQuadInstance {
            rect: [x as f32, y as f32, slot.w as f32, slot.h as f32],
            uv: slot.uv(),
            color,
        });
        let _ = vpr;
    }

    /// Horizontally centered on media x, vertically centered on media y.
    #[allow(clippy::too_many_arguments)]
    fn push_label_centered(
        &mut self,
        group: &mut DrawGroup,
        text: &str,
        x_center_media: f64,
        y_center_media: f64,
        color: [f32; 4],
        hpr: f64,
        vpr: f64,
    ) {
        let slot = self.label_slot(text, hpr);
        let x = (x_center_media * hpr).round() - (slot.w / 2) as f64;
        let y = (y_center_media * vpr).round() - (slot.h / 2) as f64;
        group.tex_quads.push(TexQuadInstance {
            rect: [x as f32, y as f32, slot.w as f32, slot.h as f32],
            uv: slot.uv(),
            color,
        });
    }
}

fn color_rgba(c: Color) -> [f32; 4] {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, c.a() as f32 / 255.0]
}
