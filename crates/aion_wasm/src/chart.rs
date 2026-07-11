//! The chart object exported to JS.

use wasm_bindgen::prelude::*;

use aion_core::model::plot_list::{PlotList, PlotValueIndex};
use aion_core::model::price_range::PriceRange;
use aion_core::scale::price_scale_core::{PriceScaleCore, PriceScaleCoreOptions};
use aion_core::scale::time_scale_core::{TimeScaleCore, TimeScaleOptions};
use aion_core::scale::time_tick_marks::{fill_weights_for_points, TimeTickMarks};
use aion_render::bars::{build_bars, BarItem, BarsParams};
use aion_render::candles::{build_candles, CandleItem, CandlesParams};
use aion_render::color::Color;
use aion_render::draw_list::{LineStyle, Prim};
use aion_render_wgpu::{prims_to_instances, QuadInstance, QuadRenderer};

// lightweight-charts default palette (RENDERING_SPEC.md §2.5, §7, §8, §15)
const UP_COLOR: Color = Color::rgb(0x26, 0xa6, 0x9a);
const DOWN_COLOR: Color = Color::rgb(0xef, 0x53, 0x50);
const GRID_COLOR: Color = Color::rgb(0xd6, 0xdc, 0xde);
const CROSSHAIR_COLOR: Color = Color::rgb(0x95, 0x98, 0xa1);

/// Time-axis label density inputs (RENDERING_SPEC.md §11): fontSize 12 default.
const FONT_SIZE: f64 = 12.0;
const TICK_MARK_MAX_CHARS: f64 = 8.0;

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
    renderer: QuadRenderer,
}

#[wasm_bindgen]
pub struct AionChart {
    gfx: Gfx,
    time_scale: TimeScaleCore,
    price_scale: PriceScaleCore,
    plot_list: PlotList,
    times: Vec<i64>,
    tick_marks: TimeTickMarks,
    series_kind: SeriesKind,
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

    let renderer = QuadRenderer::new(&device, config.format);

    let mut time_scale = TimeScaleCore::new(TimeScaleOptions::default());
    time_scale.set_width(css_width);

    let mut price_scale = PriceScaleCore::new(PriceScaleCoreOptions::default());
    price_scale.set_height(css_height);

    Ok(AionChart {
        gfx: Gfx { device, queue, surface, config, renderer },
        time_scale,
        price_scale,
        plot_list: PlotList::new(),
        times: Vec::new(),
        tick_marks: TimeTickMarks::new(),
        series_kind: SeriesKind::Candlestick,
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

        // tick-mark weights over the merged time points (single series: the series' times)
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

    pub fn resize(&mut self, css_width: f64, css_height: f64, dpr: f64) {
        self.css_width = css_width;
        self.css_height = css_height;
        self.dpr = dpr;

        let bitmap_w = (css_width * dpr).round().max(1.0) as u32;
        let bitmap_h = (css_height * dpr).round().max(1.0) as u32;
        self.gfx.config.width = bitmap_w;
        self.gfx.config.height = bitmap_h;
        self.gfx.surface.configure(&self.gfx.device, &self.gfx.config);

        self.time_scale.set_width(css_width);
        self.price_scale.set_height(css_height);
    }

    // --- gestures (called from JS event handlers) ---

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

        let mut prims: Vec<Prim> = Vec::new();

        let visible = self.visible_data_range();
        if let Some((from, to)) = visible {
            self.autoscale(from, to);
            self.build_grid(&mut prims, from, to, hpr, vpr);
            match self.series_kind {
                SeriesKind::Candlestick => self.build_candle_prims(&mut prims, from, to, hpr, vpr),
                SeriesKind::Bar => self.build_bar_prims(&mut prims, from, to, hpr, vpr),
            }
        }

        self.build_crosshair(&mut prims, hpr, vpr);

        let mut instances: Vec<QuadInstance> = Vec::new();
        prims_to_instances(&prims, &mut instances);

        let frame = self
            .gfx
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("get_current_texture failed: {e}")))?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.gfx.renderer.render(
            &self.gfx.device,
            &self.gfx.queue,
            &view,
            self.gfx.config.width,
            self.gfx.config.height,
            &instances,
            wgpu::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }, // layout.background default #FFFFFF
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

    /// Momentary autoscale over the visible range via the chunked min/max cache.
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

    /// Grid lines (RENDERING_SPEC.md §7, §11): horizontal at price marks, vertical at time
    /// tick marks selected by label-width density.
    fn build_grid(&mut self, prims: &mut Vec<Prim>, from: i64, to: i64, hpr: f64, vpr: f64) {
        let line_width = 1f64.max(hpr.floor()) as i32;
        let bitmap_w = self.gfx.config.width as i32;
        let bitmap_h = self.gfx.config.height as i32;

        // vertical: time tick marks
        // maxLabelWidth = (fontSize + 4) * 5 / 8 * maxChars (RENDERING_SPEC.md §11)
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
                y1: bitmap_h + line_width,
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
                x1: bitmap_w + line_width,
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

    /// Crosshair with bar snapping (RENDERING_SPEC.md §8): x snaps to the hovered bar's center.
    fn build_crosshair(&mut self, prims: &mut Vec<Prim>, hpr: f64, vpr: f64) {
        let Some((x_css, y_css)) = self.crosshair else { return };
        if self.plot_list.is_empty() || self.time_scale.is_empty() {
            return;
        }

        let mut index = self.time_scale.coordinate_to_index(x_css);
        if let Some(range) = self.time_scale.visible_strict_range() {
            index = index.clamp(range.left(), range.right());
        }
        index = index.clamp(0, self.plot_list.size() as i64 - 1);
        let snapped_x = self.time_scale.index_to_coordinate(index);

        let line_width = 1f64.max((1.0 * hpr).floor()) as i32;
        let bitmap_w = self.gfx.config.width as i32;
        let bitmap_h = self.gfx.config.height as i32;

        prims.push(Prim::VLine {
            x: (snapped_x * hpr).round() as i32,
            y0: 0,
            y1: bitmap_h,
            width: line_width,
            style: LineStyle::LargeDashed,
            color: CROSSHAIR_COLOR,
        });
        prims.push(Prim::HLine {
            y: (y_css * vpr).round() as i32,
            x0: 0,
            x1: bitmap_w,
            width: line_width,
            style: LineStyle::LargeDashed,
            color: CROSSHAIR_COLOR,
        });
    }
}
