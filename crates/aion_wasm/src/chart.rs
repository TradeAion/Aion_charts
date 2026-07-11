//! The chart object exported to JS.

use wasm_bindgen::prelude::*;

use aion_core::model::price_range::PriceRange;
use aion_core::scale::price_scale_core::{PriceScaleCore, PriceScaleCoreOptions};
use aion_core::scale::time_scale_core::{TimeScaleCore, TimeScaleOptions};
use aion_render::candles::{build_candles, CandleItem, CandlesParams};
use aion_render::color::Color;
use aion_render::draw_list::{LineStyle, Prim};
use aion_render_wgpu::{prims_to_instances, QuadInstance, QuadRenderer};

// lightweight-charts default palette (RENDERING_SPEC.md §2.5, §7, §8, §15)
const UP_COLOR: Color = Color::rgb(0x26, 0xa6, 0x9a);
const DOWN_COLOR: Color = Color::rgb(0xef, 0x53, 0x50);
const GRID_COLOR: Color = Color::rgb(0xd6, 0xdc, 0xde);
const CROSSHAIR_COLOR: Color = Color::rgb(0x95, 0x98, 0xa1);

struct OhlcData {
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
}

impl OhlcData {
    fn len(&self) -> usize {
        self.open.len()
    }
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
    data: OhlcData,
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
        data: OhlcData { open: vec![], high: vec![], low: vec![], close: vec![] },
        css_width,
        css_height,
        dpr,
        crosshair: None,
    })
}

#[wasm_bindgen]
impl AionChart {
    pub fn set_data(&mut self, open: &[f64], high: &[f64], low: &[f64], close: &[f64]) {
        assert!(
            open.len() == high.len() && high.len() == low.len() && low.len() == close.len(),
            "OHLC arrays must have equal length"
        );
        self.data = OhlcData {
            open: open.to_vec(),
            high: high.to_vec(),
            low: low.to_vec(),
            close: close.to_vec(),
        };
        let len = self.data.len();
        self.time_scale.set_points_len(len);
        self.time_scale
            .set_base_index(if len == 0 { None } else { Some(len as i64 - 1) });
    }

    /// Streaming update: replaces the last bar or appends a new one.
    pub fn update_bar(&mut self, index: u32, open: f64, high: f64, low: f64, close: f64) {
        let i = index as usize;
        let len = self.data.len();
        if i == len {
            self.data.open.push(open);
            self.data.high.push(high);
            self.data.low.push(low);
            self.data.close.push(close);
            self.time_scale.set_points_len(len + 1);
            self.time_scale.set_base_index(Some(len as i64));
        } else if i < len {
            self.data.open[i] = open;
            self.data.high[i] = high;
            self.data.low[i] = low;
            self.data.close[i] = close;
        }
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
            self.build_grid(&mut prims, hpr, vpr);
            self.build_candle_prims(&mut prims, from, to, hpr, vpr);
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
        if self.data.len() == 0 || self.time_scale.is_empty() {
            return None;
        }
        let range = self.time_scale.visible_strict_range()?;
        let from = range.left().max(0);
        let to = range.right().min(self.data.len() as i64 - 1);
        if from > to {
            return None;
        }
        Some((from, to))
    }

    /// Momentary autoscale over the visible range (chunked min/max cache comes with PlotList).
    fn autoscale(&mut self, from: i64, to: i64) {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for i in from as usize..=to as usize {
            min = min.min(self.data.low[i]);
            max = max.max(self.data.high[i]);
        }
        if min.is_finite() && max.is_finite() {
            self.price_scale
                .apply_autoscale_range(Some(PriceRange::new(min, max)), 0.01);
        }
    }

    /// Horizontal grid lines at price marks (RENDERING_SPEC.md §7). Vertical grid needs the
    /// time tick-marks port (weights/labels) and lands with the time axis.
    fn build_grid(&mut self, prims: &mut Vec<Prim>, hpr: f64, vpr: f64) {
        let line_width = 1f64.max(hpr.floor()) as i32;
        let bitmap_w = self.gfx.config.width as i32;

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

        for i in from..=to {
            let idx = i as usize;
            let open = self.data.open[idx];
            let high = self.data.high[idx];
            let low = self.data.low[idx];
            let close = self.data.close[idx];

            let up = close >= open;
            let color = if up { UP_COLOR } else { DOWN_COLOR };

            let x = self.time_scale.index_to_coordinate(i);
            // firstValue only matters for percent/indexed modes; pass close for normal mode
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

    /// Crosshair with bar snapping (RENDERING_SPEC.md §8): x snaps to the hovered bar's center.
    fn build_crosshair(&mut self, prims: &mut Vec<Prim>, hpr: f64, vpr: f64) {
        let Some((x_css, y_css)) = self.crosshair else { return };
        if self.data.len() == 0 || self.time_scale.is_empty() {
            return;
        }

        let mut index = self.time_scale.coordinate_to_index(x_css);
        if let Some(range) = self.time_scale.visible_strict_range() {
            index = index.clamp(range.left(), range.right());
        }
        index = index.clamp(0, self.data.len() as i64 - 1);
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
