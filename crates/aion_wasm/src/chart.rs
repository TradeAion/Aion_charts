//! The chart object exported to JS.
//!
//! Hybrid rendering, mirroring lightweight-charts' per-cell canvas layout:
//! - the **pane** (grid, series, crosshair lines) is drawn with WebGPU;
//! - the **axes** (borders, tick labels, crosshair axis labels) are drawn natively on a
//!   stacked Canvas2D overlay via web-sys, so axis text is the browser's own `fillText`.
//!
//! Both canvases are full chart size and share the same rect; the WebGPU pass is scissored to
//! the pane region (axis strips are left as the white clear color) and the 2D overlay is
//! transparent except over the axis strips. All layout/formatting logic stays in Rust; the
//! overlay context is just a drawing backend.
//!
//! ```text
//! +-------------------------------+--------+
//! |             pane              | price  |
//! |   (grid, series, crosshair)   |  axis  |   <- pane: WebGPU, axes: Canvas2D overlay
//! +-------------------------------+--------+
//! |           time axis           |  stub  |
//! +-------------------------------+--------+
//! ```

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::CanvasRenderingContext2d;

use aion_core::format::price_formatter::PriceFormatter;
use aion_core::format::time_formatter::{
    format_crosshair_time, format_tick_label, weight_to_tick_mark_type,
};
use aion_core::model::magnet::{magnet_snap, CrosshairMode};
use aion_core::model::plot_list::{PlotList, PlotValueIndex};
use aion_core::model::price_range::PriceRange;
use aion_core::scale::price_scale_core::{PriceScaleCore, PriceScaleCoreOptions};
use aion_core::scale::time_scale_core::{TimeScaleCore, TimeScaleOptions};
use aion_core::scale::time_tick_marks::{fill_weights_for_points, TimeTickMarks};
use aion_render::bars::{build_bars, BarItem, BarsParams};
use aion_render::candles::{build_candles, CandleItem, CandlesParams};
use aion_render::color::Color;
use aion_render::draw_list::{LineStyle, LineType, Prim};
use aion_render::line::{
    build_area_fill, build_disc, build_line_stroke, AreaMesh, LineParams, LinePoint, StrokeMesh,
};
use aion_render_wgpu::{
    prims_to_instances, render_frame, DrawGroup, LabelAtlas, MsaaTarget, QuadRenderer,
    TexQuadRenderer, TriRenderer, TriVertex, SAMPLE_COUNT,
};

// lightweight-charts default palette (RENDERING_SPEC.md §2.5, §7, §8, §15)
const UP_COLOR: Color = Color::rgb(0x26, 0xa6, 0x9a);
const DOWN_COLOR: Color = Color::rgb(0xef, 0x53, 0x50);
const GRID_COLOR: Color = Color::rgb(0xd6, 0xdc, 0xde);
const CROSSHAIR_COLOR: Color = Color::rgb(0x95, 0x98, 0xa1);

// Axis palette (as CSS color strings for the 2D overlay)
const BORDER_CSS: &str = "#2B2B43";
const LABEL_BG_CSS: &str = "#131722";
const TEXT_CSS: &str = "#191919";
const WHITE_CSS: &str = "#FFFFFF";

// Line/Area series defaults (line-series.ts / area-series.ts)
const LINE_COLOR: Color = Color::rgb(0x21, 0x96, 0xf3);
const AREA_LINE_COLOR: Color = Color::rgb(0x33, 0xd7, 0x78);
const AREA_TOP_COLOR: Color = Color::rgba(0x2e, 0xdc, 0x87, 102); // rgba(46,220,135,0.4)
const AREA_BOTTOM_COLOR: Color = Color::rgba(0x28, 0xdd, 0x64, 0); // rgba(40,221,100,0)
const DEFAULT_LINE_WIDTH: f64 = 3.0;

// Crosshair marker (line/area) — line-series.ts defaults.
const CROSSHAIR_MARKER_RADIUS: f64 = 4.0;
const CROSSHAIR_MARKER_BORDER_WIDTH: f64 = 2.0;
const MARKER_BORDER_COLOR: Color = Color::rgb(0xFF, 0xFF, 0xFF); // = chart background


/// LWC default font stack (`helpers/make-font.ts` / layout defaults).
const FONT_FAMILY: &str =
    "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif";

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
const TICK_MARK_MAX_CHARS: f64 = 8.0;

/// optimalHeight = ceil(border + tick + fontSize + padTop + padBottom + labelBottomOffset),
/// snapped even -> 28.
const TIME_AXIS_HEIGHT: f64 = 28.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SeriesKind {
    Candlestick,
    Bar,
    Line,
    Area,
}

struct Gfx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    quad_renderer: QuadRenderer,
    tri_renderer: TriRenderer,
    msaa: MsaaTarget,
    // Reserved for future in-pane text (legend, watermark, series markers). The atlas owns
    // the texture the tex renderer's bind group references, so it must stay alive.
    _atlas: LabelAtlas,
    tex_renderer: TexQuadRenderer,
}

#[wasm_bindgen]
pub struct AionChart {
    gfx: Gfx,
    axis_ctx: CanvasRenderingContext2d,
    time_scale: TimeScaleCore,
    price_scale: PriceScaleCore,
    price_formatter: PriceFormatter,
    plot_list: PlotList,
    times: Vec<i64>,
    tick_marks: TimeTickMarks,
    series_kind: SeriesKind,
    crosshair_mode: CrosshairMode,
    time_visible: bool,
    css_width: f64,
    css_height: f64,
    dpr: f64,
    crosshair: Option<(f64, f64)>,
    // last computed layout (css px)
    pane_w: f64,
    pane_h: f64,
    axis_w: f64,
}

/// Creates a chart bound to `pane_canvas` (WebGPU) and `overlay_canvas` (Canvas2D). Both must
/// be full chart size with bitmap size = css size * dpr, already set by the caller.
#[wasm_bindgen]
pub async fn create_chart(
    pane_canvas: web_sys::HtmlCanvasElement,
    overlay_canvas: web_sys::HtmlCanvasElement,
    css_width: f64,
    css_height: f64,
    dpr: f64,
) -> Result<AionChart, JsValue> {
    console_error_panic_hook::set_once();

    let axis_ctx = overlay_canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(pane_canvas))
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

    let quad_renderer = QuadRenderer::new(&device, config.format, SAMPLE_COUNT);
    let atlas = LabelAtlas::new(&device);
    let tex_renderer = TexQuadRenderer::new(&device, config.format, atlas.view(), SAMPLE_COUNT);
    let tri_renderer = TriRenderer::new(&device, config.format, SAMPLE_COUNT);
    let msaa = MsaaTarget::new(&device, config.format, bitmap_w, bitmap_h);

    let time_scale = TimeScaleCore::new(TimeScaleOptions::default());
    let price_scale = PriceScaleCore::new(PriceScaleCoreOptions::default());

    Ok(AionChart {
        gfx: Gfx {
            device,
            queue,
            surface,
            config,
            quad_renderer,
            tri_renderer,
            msaa,
            _atlas: atlas,
            tex_renderer,
        },
        axis_ctx,
        time_scale,
        price_scale,
        price_formatter: PriceFormatter::default(),
        plot_list: PlotList::new(),
        times: Vec::new(),
        tick_marks: TimeTickMarks::new(),
        series_kind: SeriesKind::Candlestick,
        crosshair_mode: CrosshairMode::Magnet, // LWC default
        time_visible: true,
        css_width,
        css_height,
        dpr,
        crosshair: None,
        pane_w: css_width,
        pane_h: css_height,
        axis_w: 0.0,
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

    /// 0 = candlestick, 1 = OHLC bars, 2 = line, 3 = area.
    pub fn set_series_type(&mut self, kind: u8) {
        self.series_kind = match kind {
            1 => SeriesKind::Bar,
            2 => SeriesKind::Line,
            3 => SeriesKind::Area,
            _ => SeriesKind::Candlestick,
        };
    }

    /// Show intraday time in tick labels and the crosshair label (LWC `timeScale.timeVisible`).
    pub fn set_time_visible(&mut self, visible: bool) {
        self.time_visible = visible;
    }

    /// 0 = normal, 1 = magnet (snap to close, LWC default), 2 = hidden, 3 = magnet OHLC.
    pub fn set_crosshair_mode(&mut self, mode: u8) {
        self.crosshair_mode = match mode {
            0 => CrosshairMode::Normal,
            2 => CrosshairMode::Hidden,
            3 => CrosshairMode::MagnetOhlc,
            _ => CrosshairMode::Magnet,
        };
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

    // --- gestures (pane-local css coordinates from the overlay's pointer events) ---

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

    /// Price-axis width in css px from the last render (0 until first render).
    pub fn price_axis_width(&self) -> f64 {
        self.axis_w
    }

    // --- rendering ---

    pub fn render(&mut self) -> Result<(), JsValue> {
        let hpr = self.dpr;
        let vpr = self.dpr;

        // ---- layout ----
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
        self.pane_w = pane_w;
        self.pane_h = pane_h;
        self.axis_w = axis_w;

        let pane_w_px = (pane_w * hpr).round() as u32;
        let pane_h_px = (pane_h * vpr).round() as u32;

        // Time tick marks: built once (needs &mut self), shared by the GPU grid and the 2D
        // axis labels.
        let pixels_per_character = (FONT_SIZE + 4.0) * 5.0 / 8.0;
        let max_label_width = pixels_per_character * TICK_MARK_MAX_CHARS;
        let spacing = self.time_scale.bar_spacing();
        let time_marks: Vec<(i64, u8)> = self
            .tick_marks
            .build(spacing, max_label_width)
            .iter()
            .map(|m| (m.index, m.weight))
            .collect();

        // ---- GPU pane group (scissored) ----
        let mut pane_group = DrawGroup {
            scissor: Some([0, 0, pane_w_px, pane_h_px]),
            ..Default::default()
        };
        let mut pane_prims: Vec<Prim> = Vec::new();

        let visible = self.visible_data_range();
        if let Some((from, to)) = visible {
            self.build_grid(&mut pane_prims, &time_marks, from, to, pane_w_px as i32, pane_h_px as i32, hpr, vpr);
            match self.series_kind {
                SeriesKind::Candlestick => self.build_candle_prims(&mut pane_prims, from, to, hpr, vpr),
                SeriesKind::Bar => self.build_bar_prims(&mut pane_prims, from, to, hpr, vpr),
                SeriesKind::Line | SeriesKind::Area => {
                    self.build_line_prims(&mut pane_group, from, to, pane_h, hpr, vpr)
                }
            }
            self.build_last_value_line(&mut pane_prims, pane_w_px as i32, vpr);
        }
        let mut crosshair_stroke: Vec<TriVertex> = Vec::new();
        self.build_crosshair_lines(&mut pane_prims, &mut crosshair_stroke, pane_w_px as i32, pane_h_px as i32, pane_w, pane_h, hpr, vpr);
        pane_group.stroke_tris.extend(crosshair_stroke);
        prims_to_instances(&pane_prims, &mut pane_group.quads);

        self.gfx.msaa.ensure(
            &self.gfx.device,
            self.gfx.config.format,
            self.gfx.config.width,
            self.gfx.config.height,
        );

        let frame = self
            .gfx
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("get_current_texture failed: {e}")))?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        render_frame(
            &self.gfx.device,
            &self.gfx.queue,
            self.gfx.msaa.view(),
            &view,
            self.gfx.config.width,
            self.gfx.config.height,
            wgpu::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }, // layout.background default #FFFFFF
            &self.gfx.quad_renderer,
            &self.gfx.tex_renderer,
            &self.gfx.tri_renderer,
            &[pane_group],
        );

        frame.present();

        // ---- Canvas2D axis overlay ----
        self.draw_axes_2d(visible, &time_marks)?;

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

    /// Text width in css px via the overlay 2D context (mirrors LWC's `TextWidthCache`).
    ///
    /// Sets the **device-px** font (`fontSize * dpr`) — the same font used for drawing — so the
    /// context is never left in a CSS-px state. Otherwise a `fill_text` after a `measure` would
    /// render at half size on HiDPI (dpr != 1) displays. Returns the width back in css px.
    fn measure(&self, text: &str) -> f64 {
        self.axis_ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * self.dpr));
        let device_w = self.axis_ctx.measure_text(text).map(|m| m.width()).unwrap_or(0.0);
        device_w / self.dpr
    }

    /// Port of `PriceAxisWidget.optimalWidth()` (RENDERING_SPEC.md §10).
    fn compute_price_axis_width(&self) -> f64 {
        let mut max_text_w = 0f64;
        for mark in self.price_scale.build_tick_marks(100, 0.0) {
            let label = self.price_formatter.format(mark.logical);
            max_text_w = max_text_w.max(self.measure(&label));
        }
        if let Some((_, y)) = self.crosshair {
            if !self.price_scale.is_empty() {
                let price = self.price_scale.coordinate_to_price(y, 0.0);
                let label = self.price_formatter.format(price);
                max_text_w = max_text_w.max(self.measure(&label));
            }
        }
        // last-value label participates in axis width (LWC includes back labels)
        if !self.plot_list.is_empty() {
            let last = self.plot_list.size() - 1;
            let close = self.plot_list.column(PlotValueIndex::Close)[last];
            max_text_w = max_text_w.max(self.measure(&self.price_formatter.format(close)));
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

    // ---- GPU pane builders ----

    #[allow(clippy::too_many_arguments)]
    fn build_grid(
        &mut self,
        prims: &mut Vec<Prim>,
        time_marks: &[(i64, u8)],
        from: i64,
        to: i64,
        pane_w_px: i32,
        pane_h_px: i32,
        hpr: f64,
        vpr: f64,
    ) {
        let line_width = 1f64.max(hpr.floor()) as i32;

        for &(index, _weight) in time_marks {
            if index < from || index > to {
                continue;
            }
            let x = (self.time_scale.index_to_coordinate(index) * hpr).round() as i32;
            prims.push(Prim::VLine {
                x,
                y0: -line_width,
                y1: pane_h_px + line_width,
                width: line_width,
                style: LineStyle::Solid,
                color: GRID_COLOR,
            });
        }

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

    fn build_line_prims(
        &mut self,
        group: &mut DrawGroup,
        from: i64,
        to: i64,
        pane_h: f64,
        hpr: f64,
        vpr: f64,
    ) {
        let close_col = self.plot_list.column(PlotValueIndex::Close);
        let mut points: Vec<LinePoint> = Vec::with_capacity((to - from + 1) as usize);
        for i in from..=to {
            let close = close_col[i as usize];
            points.push(LinePoint {
                x: self.time_scale.index_to_coordinate(i),
                y: self.price_scale.price_to_coordinate(close, close),
            });
        }

        let params = LineParams {
            horizontal_pixel_ratio: hpr,
            vertical_pixel_ratio: vpr,
            line_width: DEFAULT_LINE_WIDTH,
            line_type: LineType::Simple,
        };

        let line_color = match self.series_kind {
            SeriesKind::Area => AREA_LINE_COLOR,
            _ => LINE_COLOR,
        };

        if self.series_kind == SeriesKind::Area {
            let mut area = AreaMesh::default();
            build_area_fill(&points, pane_h, AREA_TOP_COLOR, AREA_BOTTOM_COLOR, &params, &mut area);
            group.fill_tris.extend(area.vertices.iter().map(mesh_vertex));
        }

        let mut stroke = StrokeMesh::default();
        build_line_stroke(&points, line_color, &params, &mut stroke);
        group.stroke_tris.extend(stroke.vertices.iter().map(mesh_vertex));
    }

    /// Dashed horizontal line at the last bar's close, spanning the pane (priceLineSource
    /// LastBar, priceLineVisible default true).
    fn build_last_value_line(&self, prims: &mut Vec<Prim>, pane_w_px: i32, vpr: f64) {
        if self.plot_list.is_empty() || self.price_scale.is_empty() {
            return;
        }
        let last = self.plot_list.size() - 1;
        let close = self.plot_list.column(PlotValueIndex::Close)[last];
        let y = self.price_scale.price_to_coordinate(close, close);
        let width = 1f64.max(vpr.floor()) as i32;
        prims.push(Prim::HLine {
            y: (y * vpr).round() as i32,
            x0: 0,
            x1: pane_w_px,
            width,
            style: LineStyle::Dashed,
            color: self.last_value_color(),
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn build_crosshair_lines(
        &mut self,
        prims: &mut Vec<Prim>,
        group_stroke: &mut Vec<TriVertex>,
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
        if x_css > pane_w || y_css > pane_h || self.crosshair_mode == CrosshairMode::Hidden {
            return;
        }

        let snapped_x = self.snapped_crosshair_x(x_css);
        let (_price, snap_y) = self.crosshair_snap(x_css, y_css);
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
            y: (snap_y * vpr).round() as i32,
            x0: 0,
            x1: pane_w_px,
            width: line_width,
            style: LineStyle::LargeDashed,
            color: CROSSHAIR_COLOR,
        });

        // crosshair marker on line/area: white halo disc + series-color disc at the data point
        if matches!(self.series_kind, SeriesKind::Line | SeriesKind::Area) {
            let idx = self.snapped_crosshair_index(x_css) as usize;
            let close = self.plot_list.column(PlotValueIndex::Close)[idx];
            let cx = (self.time_scale.index_to_coordinate(self.snapped_crosshair_index(x_css)) * hpr) as f32;
            let cy = (self.price_scale.price_to_coordinate(close, close) * vpr) as f32;
            let fill = if self.series_kind == SeriesKind::Area { AREA_LINE_COLOR } else { LINE_COLOR };
            let outer_r = ((CROSSHAIR_MARKER_RADIUS + CROSSHAIR_MARKER_BORDER_WIDTH) * vpr) as f32;
            let inner_r = (CROSSHAIR_MARKER_RADIUS * vpr) as f32;
            let mut disc = Vec::new();
            build_disc([cx, cy], outer_r, MARKER_BORDER_COLOR, &mut disc);
            build_disc([cx, cy], inner_r, fill, &mut disc);
            group_stroke.extend(disc.iter().map(mesh_vertex));
        }
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

    /// Magnet-snapped crosshair price + its pane y-coordinate (RENDERING_SPEC.md §8).
    /// In Normal mode the price follows the cursor; in Magnet it sticks to the hovered bar's
    /// close; in MagnetOHLC to the nearest of O/H/L/C.
    fn crosshair_snap(&self, x_css: f64, y_css: f64) -> (f64, f64) {
        let idx = self.snapped_crosshair_index(x_css) as usize;
        let close = self.plot_list.column(PlotValueIndex::Close)[idx];

        let price = match self.crosshair_mode {
            CrosshairMode::Normal => return (self.price_scale.coordinate_to_price(y_css, close), y_css),
            CrosshairMode::Hidden => return (self.price_scale.coordinate_to_price(y_css, close), y_css),
            CrosshairMode::Magnet => close,
            CrosshairMode::MagnetOhlc => {
                let open = self.plot_list.column(PlotValueIndex::Open)[idx];
                let high = self.plot_list.column(PlotValueIndex::High)[idx];
                let low = self.plot_list.column(PlotValueIndex::Low)[idx];
                let candidates = [
                    (open, self.price_scale.price_to_coordinate(open, open)),
                    (high, self.price_scale.price_to_coordinate(high, high)),
                    (low, self.price_scale.price_to_coordinate(low, low)),
                    (close, self.price_scale.price_to_coordinate(close, close)),
                ];
                magnet_snap(y_css, &candidates).unwrap_or(close)
            }
        };
        (price, self.price_scale.price_to_coordinate(price, price))
    }

    /// Color of the last-value price line/label: last bar's up/down for OHLC series, or the
    /// line color for line/area.
    fn last_value_color(&self) -> Color {
        match self.series_kind {
            SeriesKind::Line => LINE_COLOR,
            SeriesKind::Area => AREA_LINE_COLOR,
            _ => {
                let last = self.plot_list.size() - 1;
                let open = self.plot_list.column(PlotValueIndex::Open)[last];
                let close = self.plot_list.column(PlotValueIndex::Close)[last];
                if close >= open { UP_COLOR } else { DOWN_COLOR }
            }
        }
    }

    // ---- Canvas2D axis overlay (RENDERING_SPEC.md §10, §11) ----
    //
    // Drawn in device px (media coord * dpr, matching the GPU path) with the font scaled to
    // `fontSize * dpr`, so axis text is crisp and positioned identically to LWC's bitmap text.

    fn draw_axes_2d(
        &self,
        visible: Option<(i64, i64)>,
        time_marks: &[(i64, u8)],
    ) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        let dpr = self.dpr;
        let bitmap_w = self.gfx.config.width as f64;
        let bitmap_h = self.gfx.config.height as f64;
        let pane_w = self.pane_w;
        let pane_h = self.pane_h;

        ctx.clear_rect(0.0, 0.0, bitmap_w, bitmap_h);

        let font = format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr);
        let border_w = 1f64.max(dpr.floor());

        // ---- axis borders ----
        ctx.set_fill_style_str(BORDER_CSS);
        ctx.fill_rect((pane_w * dpr).round(), 0.0, border_w, (pane_h * dpr).round());
        ctx.fill_rect(0.0, (pane_h * dpr).round(), bitmap_w, border_w);

        // ---- price tick labels (right axis, left-aligned after the tick) ----
        ctx.set_font(&font);
        ctx.set_text_baseline("middle");
        ctx.set_text_align("left");
        ctx.set_fill_style_str(TEXT_CSS);
        let text_x = (pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER) * dpr;
        for mark in self.price_scale.build_tick_marks(100, 0.0) {
            let label = self.price_formatter.format(mark.logical);
            ctx.fill_text(&label, text_x, (mark.coord * dpr).round())?;
        }

        // ---- time tick labels (bottom axis, centered) ----
        if let Some((from, to)) = visible {
            ctx.set_text_align("center");
            ctx.set_fill_style_str(TEXT_CSS);
            let y_center =
                pane_h + AXIS_BORDER_SIZE + AXIS_TICK_LENGTH + TIME_PADDING_TOP + FONT_SIZE / 2.0;
            for &(index, weight) in time_marks {
                if index < from || index > to {
                    continue;
                }
                let ts = self.times[index as usize];
                let mark_type = weight_to_tick_mark_type(weight, self.time_visible, false);
                let label = format_tick_label(ts, mark_type);
                let x_center = self.time_scale.index_to_coordinate(index);
                ctx.fill_text(&label, (x_center * dpr).round(), (y_center * dpr).round())?;
            }
        }

        // ---- last-value label (series-colored box) ----
        self.draw_last_value_label_2d(pane_w, pane_h, dpr)?;

        // ---- crosshair axis labels ----
        self.draw_crosshair_labels_2d(pane_w, pane_h, dpr, &font)?;

        Ok(())
    }

    fn draw_last_value_label_2d(&self, pane_w: f64, pane_h: f64, dpr: f64) -> Result<(), JsValue> {
        if self.plot_list.is_empty() || self.price_scale.is_empty() {
            return Ok(());
        }
        let last = self.plot_list.size() - 1;
        let close = self.plot_list.column(PlotValueIndex::Close)[last];
        let y = self.price_scale.price_to_coordinate(close, close);
        if y < 0.0 || y > pane_h {
            return Ok(());
        }

        let ctx = &self.axis_ctx;
        ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr));
        let color = self.last_value_color();
        let label = self.price_formatter.format(close);
        let text_w = self.measure(&label);

        let box_h = ((FONT_SIZE + PRICE_LABEL_PADDING_TB * 2.0) * dpr).round();
        let box_w =
            ((AXIS_BORDER_SIZE + PRICE_PADDING_INNER + PRICE_PADDING_OUTER + AXIS_TICK_LENGTH + text_w)
                * dpr)
                .round();
        let box_x = (pane_w * dpr).round();
        let box_y = ((y * dpr).round() - box_h / 2.0).round();

        ctx.set_fill_style_str(&color.to_hex());
        ctx.fill_rect(box_x, box_y, box_w, box_h);

        ctx.set_text_align("left");
        ctx.set_text_baseline("middle");
        ctx.set_fill_style_str(&color.contrast_text().to_hex());
        let text_x = (pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER) * dpr;
        ctx.fill_text(&label, text_x, (y * dpr).round())?;
        Ok(())
    }

    fn draw_crosshair_labels_2d(
        &self,
        pane_w: f64,
        pane_h: f64,
        dpr: f64,
        font: &str,
    ) -> Result<(), JsValue> {
        let Some((x_css, y_css)) = self.crosshair else { return Ok(()) };
        if self.plot_list.is_empty()
            || self.time_scale.is_empty()
            || self.crosshair_mode == CrosshairMode::Hidden
        {
            return Ok(());
        }
        let ctx = &self.axis_ctx;
        ctx.set_font(font);
        ctx.set_text_baseline("middle");

        // price label on the right axis (magnet-snapped price + coordinate)
        if y_css <= pane_h && !self.price_scale.is_empty() {
            let (price, snap_y) = self.crosshair_snap(x_css, y_css);
            let label = self.price_formatter.format(price);
            let text_w = self.measure(&label);

            let box_h = ((FONT_SIZE + PRICE_LABEL_PADDING_TB * 2.0) * dpr).round();
            let box_w =
                ((AXIS_BORDER_SIZE + PRICE_PADDING_INNER + PRICE_PADDING_OUTER + AXIS_TICK_LENGTH + text_w)
                    * dpr)
                    .round();
            let box_x = (pane_w * dpr).round();
            let box_y = ((snap_y * dpr).round() - box_h / 2.0).round();

            ctx.set_fill_style_str(LABEL_BG_CSS);
            ctx.fill_rect(box_x, box_y, box_w, box_h);

            ctx.set_text_align("left");
            ctx.set_fill_style_str(WHITE_CSS);
            let text_x = (pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER) * dpr;
            ctx.fill_text(&label, text_x, (snap_y * dpr).round())?;
        }

        // time label on the bottom axis
        if x_css <= pane_w {
            let index = self.snapped_crosshair_index(x_css);
            let ts = self.times[index as usize];
            let label = format_crosshair_time(ts, self.time_visible, false);
            let text_w = self.measure(&label);

            let box_w = ((text_w + TIME_PADDING_HORZ * 2.0) * dpr).round();
            let box_h = ((FONT_SIZE + TIME_PADDING_TOP + TIME_PADDING_BOTTOM) * dpr).round();
            let snapped_x = self.snapped_crosshair_x(x_css);
            let bitmap_w = self.gfx.config.width as f64;
            let box_x = ((snapped_x * dpr).round() - box_w / 2.0).clamp(0.0, bitmap_w - box_w);
            let box_y = (pane_h * dpr).round() + 1f64.max(dpr.floor());

            ctx.set_fill_style_str(LABEL_BG_CSS);
            ctx.fill_rect(box_x, box_y, box_w, box_h);

            ctx.set_text_align("center");
            ctx.set_fill_style_str(WHITE_CSS);
            ctx.fill_text(&label, box_x + box_w / 2.0, box_y + box_h / 2.0)?;
        }

        Ok(())
    }
}

fn mesh_vertex(v: &aion_render::line::LineVertex) -> TriVertex {
    TriVertex { pos: [v.x, v.y], color: v.color }
}
