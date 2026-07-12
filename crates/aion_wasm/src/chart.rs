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
//! Multiple series share one time axis via [`DataLayer`] (the merged time-point list). Each
//! series maps its data onto merged indices; a series absent at an index is whitespace there.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::CanvasRenderingContext2d;

use aion_core::format::price_formatter::PriceFormatter;
use aion_core::format::time_formatter::{
    format_crosshair_time, format_tick_label, weight_to_tick_mark_type,
};
use aion_core::model::data_layer::{DataLayer, SeriesId};
use aion_core::model::data_validation::{sanitize_ohlc, sanitize_point};
use aion_core::model::magnet::{magnet_snap, CrosshairMode};
use aion_core::model::plot_list::{PlotList, PlotValueIndex};
use aion_core::model::price_range::PriceRange;
use aion_core::model::range::{LogicalRange, StrictRange};
use aion_core::TimePointIndex;
use aion_core::scale::price_scale_core::{PriceScaleCore, PriceScaleCoreOptions};
use aion_core::scale::time_scale_core::{TimeScaleCore, TimeScaleOptions};
use aion_core::scale::time_tick_marks::{fill_weights_for_points, TimeTickMarks};
use aion_render::bars::{build_bars, BarItem, BarsParams};
use aion_render::candles::{build_candles, CandleItem, CandlesParams};
use aion_render::color::Color;
use aion_render::draw_list::{LineStyle, LineType, Prim};
use aion_render::histogram::{build_histogram, HistogramItem, HistogramParams};
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
const HISTOGRAM_COLOR: Color = Color::rgba(0x26, 0xa6, 0x9a, 0x80);
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
const PRICE_PADDING_INNER: f64 = 5.0;
const PRICE_PADDING_OUTER: f64 = 5.0;
const PRICE_LABEL_OFFSET: f64 = 5.0;
const PRICE_DEFAULT_TEXT_WIDTH: f64 = 34.0;
const PRICE_LABEL_PADDING_TB: f64 = 2.5;
const TIME_PADDING_TOP: f64 = 3.0;
const TIME_PADDING_BOTTOM: f64 = 3.0;
const TIME_PADDING_HORZ: f64 = 9.0;
const TICK_MARK_MAX_CHARS: f64 = 8.0;
const TIME_AXIS_HEIGHT: f64 = 28.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SeriesKind {
    Candlestick,
    Bar,
    Line,
    Area,
    Histogram,
}

impl SeriesKind {
    fn from_u8(kind: u8) -> Self {
        match kind {
            1 => SeriesKind::Bar,
            2 => SeriesKind::Line,
            3 => SeriesKind::Area,
            4 => SeriesKind::Histogram,
            _ => SeriesKind::Candlestick,
        }
    }
}

struct SeriesEntry {
    id: SeriesId,
    kind: SeriesKind,
    /// Overrides the default line/area color when set (e.g. an SMA overlay).
    line_color: Color,
}

struct Gfx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    quad_renderer: QuadRenderer,
    tri_renderer: TriRenderer,
    msaa: MsaaTarget,
    // Reserved for future in-pane text (legend, watermark). The atlas owns the texture the
    // tex renderer's bind group references, so it must stay alive.
    _atlas: LabelAtlas,
    tex_renderer: TexQuadRenderer,
}

struct ChartInner {
    gfx: Gfx,
    axis_ctx: CanvasRenderingContext2d,
    time_scale: TimeScaleCore,
    price_scale: PriceScaleCore,
    price_formatter: PriceFormatter,
    data: DataLayer,
    series: Vec<SeriesEntry>,
    tick_marks: TimeTickMarks,
    crosshair_mode: CrosshairMode,
    time_visible: bool,
    css_width: f64,
    css_height: f64,
    dpr: f64,
    crosshair: Option<(f64, f64)>,
    pane_w: f64,
    pane_h: f64,
    axis_w: f64,
}

/// Keeps the `ResizeObserver` and its callback alive for the chart's lifetime.
struct ResizeBinding {
    observer: web_sys::ResizeObserver,
    _callback: Closure<dyn FnMut(js_sys::Array)>,
}

impl Drop for ResizeBinding {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

/// The chart handle exported to JS. Wraps [`ChartInner`] in `Rc<RefCell<..>>` so an
/// engine-owned `ResizeObserver` callback can mutate it, and holds the canvas elements so
/// the engine can size their backing stores itself. Public methods delegate to the inner.
#[wasm_bindgen]
pub struct AionChart {
    inner: Rc<RefCell<ChartInner>>,
    pane: web_sys::HtmlCanvasElement,
    overlay: web_sys::HtmlCanvasElement,
    _resize: Option<ResizeBinding>,
}

/// Reads the exact physical-pixel size of a `ResizeObserverEntry`'s device-pixel content box.
/// This is the crisp-rendering crux: `round(cssSize * devicePixelRatio)` only approximates the
/// element's true physical footprint, so at fractional ratios (e.g. 150% scaling) the backing
/// store no longer maps 1:1 to device pixels and the compositor resamples the bitmap — soft,
/// "thicker" 1px wicks. `devicePixelContentBoxSize` is the exact integer count. Returns `None`
/// when the browser lacks the API (Safari < 16.4), so the caller can fall back to the approx.
fn device_pixel_box(entry: &web_sys::ResizeObserverEntry) -> Option<(f64, f64)> {
    let arr = entry.device_pixel_content_box_size();
    let first = arr.get(0);
    if first.is_undefined() {
        return None;
    }
    let size = first.dyn_into::<web_sys::ResizeObserverSize>().ok()?;
    Some((size.inline_size(), size.block_size()))
}

/// Sizes both canvases to `(bw, bh)` device pixels while pinning their CSS box to the real
/// displayed size, then resizes + repaints the engine. Shared by the initial bind and every
/// observer callback.
fn apply_device_size(
    inner: &Rc<RefCell<ChartInner>>,
    pane: &web_sys::HtmlCanvasElement,
    overlay: &web_sys::HtmlCanvasElement,
    css_w: f64,
    css_h: f64,
    bw: f64,
    bh: f64,
) {
    let (bw_u, bh_u) = (bw.max(1.0) as u32, bh.max(1.0) as u32);
    for c in [pane, overlay] {
        c.set_width(bw_u);
        c.set_height(bh_u);
        let style = c.style();
        let _ = style.set_property("width", &format!("{css_w}px"));
        let _ = style.set_property("height", &format!("{css_h}px"));
    }
    // Exact effective ratio -> the engine's internal round(css*dpr) lands back on (bw, bh),
    // so surface, canvas backing store and physical pixels all agree.
    let dpr = bw / css_w.max(1.0);
    let mut c = inner.borrow_mut();
    c.resize(css_w.max(1.0), css_h.max(1.0), dpr);
    let _ = c.render();
}

/// Creates a chart bound to `pane_canvas` (WebGPU) and `overlay_canvas` (Canvas2D). Both must
/// be full chart size with bitmap size = css size * dpr, already set by the caller.
/// Call [`AionChart::enable_auto_resize`] to have the engine own sizing from then on.
#[wasm_bindgen]
pub async fn create_chart(
    pane_canvas: web_sys::HtmlCanvasElement,
    overlay_canvas: web_sys::HtmlCanvasElement,
    css_width: f64,
    css_height: f64,
    dpr: f64,
) -> Result<AionChart, JsValue> {
    console_error_panic_hook::set_once();

    // Keep handles to both canvas elements so the engine can own device-pixel resizing
    // (create_surface takes the pane canvas by value; the clone is just a JS reference).
    let pane_el = pane_canvas.clone();
    let overlay_el = overlay_canvas.clone();

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

    // main series (id 0) — set_data / set_series_type target it for API compatibility
    let mut data = DataLayer::new();
    let main = data.add_series();

    let inner = ChartInner {
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
        time_scale: TimeScaleCore::new(TimeScaleOptions::default()),
        price_scale: PriceScaleCore::new(PriceScaleCoreOptions::default()),
        price_formatter: PriceFormatter::default(),
        data,
        series: vec![SeriesEntry { id: main, kind: SeriesKind::Candlestick, line_color: LINE_COLOR }],
        tick_marks: TimeTickMarks::new(),
        crosshair_mode: CrosshairMode::Magnet,
        time_visible: true,
        css_width,
        css_height,
        dpr,
        crosshair: None,
        pane_w: css_width,
        pane_h: css_height,
        axis_w: 0.0,
    };

    Ok(AionChart {
        inner: Rc::new(RefCell::new(inner)),
        pane: pane_el,
        overlay: overlay_el,
        _resize: None,
    })
}

/// Public JS surface. Sizing is engine-owned once [`enable_auto_resize`] is called; the rest
/// delegate straight through to the inner chart.
#[wasm_bindgen]
impl AionChart {
    /// Binds the engine to `container`, sizing both canvases to the container's exact
    /// device-pixel content box (crisp at any devicePixelRatio, fractional included) and
    /// re-rendering on every size/DPR change. After this, the embedder never sizes canvases.
    pub fn enable_auto_resize(&mut self, container: web_sys::HtmlElement) -> Result<(), JsValue> {
        let inner = self.inner.clone();
        let pane = self.pane.clone();
        let overlay = self.overlay.clone();
        let container_cb = container.clone();

        let callback = Closure::wrap(Box::new(move |entries: js_sys::Array| {
            let rect = container_cb.get_bounding_client_rect();
            let (css_w, css_h) = (rect.width().max(1.0), rect.height().max(1.0));
            // Prefer the exact device-pixel content box; fall back to round(css*dpr).
            let device = entries
                .get(0)
                .dyn_into::<web_sys::ResizeObserverEntry>()
                .ok()
                .and_then(|e| device_pixel_box(&e));
            let (bw, bh) = device.unwrap_or_else(|| {
                let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0);
                ((css_w * dpr).round(), (css_h * dpr).round())
            });
            apply_device_size(&inner, &pane, &overlay, css_w, css_h, bw, bh);
        }) as Box<dyn FnMut(js_sys::Array)>);

        let observer = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref())?;
        // Observe the device-pixel-content-box so the callback also fires on DPR changes.
        let opts = web_sys::ResizeObserverOptions::new();
        opts.set_box(web_sys::ResizeObserverBoxOptions::DevicePixelContentBox);
        observer.observe_with_options(&container, &opts);

        // Size once now so the first paint is correct even before the observer first fires.
        let rect = container.get_bounding_client_rect();
        let (css_w, css_h) = (rect.width().max(1.0), rect.height().max(1.0));
        let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0);
        apply_device_size(
            &self.inner,
            &self.pane,
            &self.overlay,
            css_w,
            css_h,
            (css_w * dpr).round(),
            (css_h * dpr).round(),
        );

        self._resize = Some(ResizeBinding { observer, _callback: callback });
        Ok(())
    }

    /// Adds a series and returns its id. `kind`: 0 candles, 1 bars, 2 line, 3 area, 4 histogram.
    pub fn add_series(&mut self, kind: u8) -> u32 {
        self.inner.borrow_mut().add_series(kind)
    }

    /// Sets the main series' data (series 0). `times` are ascending UTC seconds.
    pub fn set_data(&mut self, times: &[f64], open: &[f64], high: &[f64], low: &[f64], close: &[f64]) {
        self.inner.borrow_mut().set_data(times, open, high, low, close);
    }

    /// Sets a series' data by id.
    pub fn set_series_data(&mut self, id: u32, times: &[f64], open: &[f64], high: &[f64], low: &[f64], close: &[f64]) {
        self.inner.borrow_mut().set_series_data(id, times, open, high, low, close);
    }

    /// Streaming update of the main series (append new time or replace last).
    pub fn update_bar(&mut self, time: f64, open: f64, high: f64, low: f64, close: f64) {
        self.inner.borrow_mut().update_bar(time, open, high, low, close);
    }

    /// Sets a series' line/area color (overrides the kind default).
    pub fn set_series_color(&mut self, id: u32, r: u8, g: u8, b: u8) {
        self.inner.borrow_mut().set_series_color(id, r, g, b);
    }

    /// 0 = candlestick, 1 = OHLC bars, 2 = line, 3 = area, 4 = histogram (sets the main series).
    pub fn set_series_type(&mut self, kind: u8) {
        self.inner.borrow_mut().set_series_type(kind);
    }

    pub fn set_time_visible(&mut self, visible: bool) {
        self.inner.borrow_mut().set_time_visible(visible);
    }

    /// 0 = normal, 1 = magnet (LWC default), 2 = hidden, 3 = magnet OHLC.
    pub fn set_crosshair_mode(&mut self, mode: u8) {
        self.inner.borrow_mut().set_crosshair_mode(mode);
    }

    /// Manual resize (still available for embedders not using `enable_auto_resize`, and for tests).
    pub fn resize(&mut self, css_width: f64, css_height: f64, dpr: f64) {
        self.inner.borrow_mut().resize(css_width, css_height, dpr);
    }

    pub fn zoom(&mut self, x_css: f64, scale: f64) {
        self.inner.borrow_mut().zoom(x_css, scale);
    }
    pub fn scroll_start(&mut self, x_css: f64) {
        self.inner.borrow_mut().scroll_start(x_css);
    }
    pub fn scroll_move(&mut self, x_css: f64) {
        self.inner.borrow_mut().scroll_move(x_css);
    }
    pub fn scroll_end(&mut self) {
        self.inner.borrow_mut().scroll_end();
    }
    pub fn fit_content(&mut self) {
        self.inner.borrow_mut().fit_content();
    }
    pub fn set_crosshair(&mut self, x_css: f64, y_css: f64) {
        self.inner.borrow_mut().set_crosshair(x_css, y_css);
    }
    pub fn clear_crosshair(&mut self) {
        self.inner.borrow_mut().clear_crosshair();
    }
    pub fn bar_spacing(&self) -> f64 {
        self.inner.borrow().bar_spacing()
    }
    pub fn price_axis_width(&self) -> f64 {
        self.inner.borrow().price_axis_width()
    }

    // --- coordinate & logical-range API (roadmap Phase A4) ---

    /// Y (CSS px) for a price, or `undefined` if the price scale has no range yet.
    pub fn price_to_coordinate(&self, price: f64) -> Option<f64> {
        self.inner.borrow().price_to_coordinate(price)
    }
    /// Price for a Y (CSS px), or `undefined` if the price scale has no range yet.
    pub fn coordinate_to_price(&self, y_css: f64) -> Option<f64> {
        self.inner.borrow().coordinate_to_price(y_css)
    }
    /// X (CSS px) for a UTC-seconds timestamp on a data point, else `undefined`.
    pub fn time_to_coordinate(&self, time: f64) -> Option<f64> {
        self.inner.borrow().time_to_coordinate(time)
    }
    /// UTC seconds of the data point nearest X (CSS px), or `undefined` off-chart.
    pub fn coordinate_to_time(&self, x_css: f64) -> Option<f64> {
        self.inner.borrow().coordinate_to_time(x_css)
    }
    /// Visible window in logical (bar) units as a `[from, to]` Float64Array (empty if no data).
    pub fn visible_logical_range(&self) -> Vec<f64> {
        self.inner.borrow().visible_logical_range()
    }
    /// Set the visible window in logical (bar) units; call `render()` after.
    pub fn set_visible_logical_range(&mut self, from: f64, to: f64) {
        self.inner.borrow_mut().set_visible_logical_range(from, to);
    }
    /// Visible window as a `[from_time, to_time]` Float64Array of UTC seconds (empty if no data).
    pub fn visible_time_range(&self) -> Vec<f64> {
        self.inner.borrow().visible_time_range()
    }
    /// Set the visible window to bracket `[from_time, to_time]` UTC seconds; call `render()` after.
    pub fn set_visible_time_range(&mut self, from_time: f64, to_time: f64) {
        self.inner.borrow_mut().set_visible_time_range(from_time, to_time);
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        self.inner.borrow_mut().render()
    }
}

impl ChartInner {
    /// Adds a series and returns its id. `kind`: 0 candles, 1 bars, 2 line, 3 area, 4 histogram.
    pub fn add_series(&mut self, kind: u8) -> u32 {
        let id = self.data.add_series();
        self.series.push(SeriesEntry { id, kind: SeriesKind::from_u8(kind), line_color: LINE_COLOR });
        id as u32
    }

    /// Sets the main series' data (series 0). `times` are ascending UTC seconds.
    pub fn set_data(&mut self, times: &[f64], open: &[f64], high: &[f64], low: &[f64], close: &[f64]) {
        let id = self.series[0].id;
        self.set_series_data(id as u32, times, open, high, low, close);
    }

    /// Sets a series' data by id.
    pub fn set_series_data(
        &mut self,
        id: u32,
        times: &[f64],
        open: &[f64],
        high: &[f64],
        low: &[f64],
        close: &[f64],
    ) {
        // Repair messy feed data (out-of-order, duplicate times, NaN/Inf, length mismatch) at the
        // boundary so the DataLayer's ascending-unique-finite contract always holds — a malformed
        // feed yields a warning and a rendered chart, never a wasm panic (roadmap Phase A3).
        let s = match sanitize_ohlc(times, open, high, low, close) {
            Ok(s) => s,
            Err(e) => {
                web_sys::console::warn_1(&format!("aion: set_series_data rejected — {e}").into());
                return;
            }
        };
        if !s.report.is_clean() {
            web_sys::console::warn_1(
                &format!(
                    "aion: set_series_data sanitized data — accepted {}, dropped {} invalid, {} duplicate{}",
                    s.report.accepted,
                    s.report.dropped_invalid,
                    s.report.dropped_duplicate,
                    if s.report.reordered { ", reordered" } else { "" },
                )
                .into(),
            );
        }
        self.data.set_data(id as SeriesId, s.times, s.open, s.high, s.low, s.close);
        self.on_time_points_changed();
    }

    /// Streaming update of the main series (append new time or replace last).
    pub fn update_bar(&mut self, time: f64, open: f64, high: f64, low: f64, close: f64) {
        let id = self.series[0].id;
        // Drop a bad tick rather than corrupting the series (roadmap Phase A3).
        let Some((t, values)) = sanitize_point(time, [open, high, low, close]) else {
            web_sys::console::warn_1(&"aion: update_bar dropped a non-finite point".into());
            return;
        };
        self.data.update(id, t, values);
        self.on_time_points_changed();
    }

    /// Sets a series' line/area color (overrides the kind default).
    pub fn set_series_color(&mut self, id: u32, r: u8, g: u8, b: u8) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.line_color = Color::rgb(r, g, b);
        }
    }

    /// 0 = candlestick, 1 = OHLC bars, 2 = line, 3 = area, 4 = histogram (sets the main series).
    pub fn set_series_type(&mut self, kind: u8) {
        self.series[0].kind = SeriesKind::from_u8(kind);
    }

    pub fn set_time_visible(&mut self, visible: bool) {
        self.time_visible = visible;
    }

    /// 0 = normal, 1 = magnet (LWC default), 2 = hidden, 3 = magnet OHLC.
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
        // Update geometry eagerly so fit_content/zoom/scroll called before the next render
        // (and the price_axis_width getter) see the new pane size, not a stale one.
        self.recompute_layout();
    }

    /// Negotiates the price-axis width against its labels and sets the time-scale width /
    /// price-scale height accordingly. Idempotent; called on resize, data change, and render.
    /// (The axis labels depend only on the price range, so one refinement pass converges.)
    fn recompute_layout(&mut self) {
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
        self.pane_w = (self.css_width - axis_w).max(1.0);
        self.pane_h = pane_h;
        self.axis_w = axis_w;
    }

    // --- gestures ---

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
    pub fn price_axis_width(&self) -> f64 {
        self.axis_w
    }

    // --- coordinate & logical-range API (roadmap Phase A4) ---
    //
    // Reflects the state of the last render (scale height/width, price range). All coordinates
    // are media (CSS) pixels relative to the pane origin, matching the pointer coords JS passes
    // to `set_crosshair`. `None`/empty means the query falls off the chart or there is no data.

    /// Y (CSS px) for a price on the active price scale, or `None` if the scale has no range yet.
    /// In percentage/indexed modes the price is its own base value (as in the render path).
    pub fn price_to_coordinate(&self, price: f64) -> Option<f64> {
        if self.price_scale.price_range().is_none() {
            return None;
        }
        Some(self.price_scale.price_to_coordinate(price, price))
    }

    /// Price for a Y (CSS px), or `None` if the scale has no range yet.
    pub fn coordinate_to_price(&self, y_css: f64) -> Option<f64> {
        if self.price_scale.price_range().is_none() {
            return None;
        }
        Some(self.price_scale.coordinate_to_price(y_css, 0.0))
    }

    /// X (CSS px) for a UTC-seconds timestamp that sits exactly on a data point, else `None`
    /// (mirrors LWC `timeToCoordinate`, which does not snap to the nearest bar).
    pub fn time_to_coordinate(&self, time: f64) -> Option<f64> {
        let t = time as i64;
        let idx = self.data.merged_times().binary_search(&t).ok()?;
        Some(self.time_scale.index_to_coordinate(idx as TimePointIndex))
    }

    /// UTC-seconds timestamp of the data point nearest to X (CSS px), or `None` if X maps outside
    /// the data range (mirrors LWC `coordinateToTime`).
    pub fn coordinate_to_time(&self, x_css: f64) -> Option<f64> {
        let times = self.data.merged_times();
        if times.is_empty() {
            return None;
        }
        let idx = self.time_scale.coordinate_to_index(x_css);
        if idx < 0 || idx as usize >= times.len() {
            return None;
        }
        Some(times[idx as usize] as f64)
    }

    /// Visible window in logical (bar) units as `[from, to]`, or empty when there is no data.
    pub fn visible_logical_range(&self) -> Vec<f64> {
        match self.time_scale.visible_logical_range() {
            Some(r) => vec![r.left(), r.right()],
            None => Vec::new(),
        }
    }

    /// Set the visible window in logical (bar) units. No-op if `from > to`. Call `render()` after.
    pub fn set_visible_logical_range(&mut self, from: f64, to: f64) {
        if from <= to {
            self.time_scale.set_logical_range(LogicalRange::new(from, to));
        }
    }

    /// Visible window as `[from_time, to_time]` UTC seconds (data points nearest each edge), or
    /// empty when there is no data.
    pub fn visible_time_range(&self) -> Vec<f64> {
        let times = self.data.merged_times();
        let Some(r) = self.time_scale.visible_strict_range() else { return Vec::new() };
        if times.is_empty() {
            return Vec::new();
        }
        let last = times.len() as i64 - 1;
        let l = r.left().clamp(0, last) as usize;
        let rr = r.right().clamp(0, last) as usize;
        vec![times[l] as f64, times[rr] as f64]
    }

    /// Set the visible window to span the data points bracketing `[from_time, to_time]` (UTC
    /// seconds). No-op if the times are reversed or there is no data. Call `render()` after.
    pub fn set_visible_time_range(&mut self, from_time: f64, to_time: f64) {
        if from_time > to_time {
            return;
        }
        let times = self.data.merged_times();
        if times.is_empty() {
            return;
        }
        // nearest bracketing indices: first point >= from, last point <= to
        let left = times.partition_point(|&t| (t as f64) < from_time);
        let right = times.partition_point(|&t| (t as f64) <= to_time);
        if right == 0 || left >= times.len() {
            return; // window lies entirely outside the data
        }
        let last = times.len() - 1;
        let l = left.min(last) as i64;
        let r = (right - 1).min(last) as i64;
        if l <= r {
            self.time_scale.set_visible_range(StrictRange::new(l, r), false);
        }
    }

    // --- rendering ---

    pub fn render(&mut self) -> Result<(), JsValue> {
        let hpr = self.dpr;
        let vpr = self.dpr;

        // ---- layout (price axis width negotiated against the price labels) ----
        self.recompute_layout();
        let pane_w = self.pane_w;
        let pane_h = self.pane_h;

        let pane_w_px = (pane_w * hpr).round() as u32;
        let pane_h_px = (pane_h * vpr).round() as u32;

        // time tick marks: built once (needs &mut), shared by GPU grid + 2D labels
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
        let mut pane_group =
            DrawGroup { scissor: Some([0, 0, pane_w_px, pane_h_px]), ..Default::default() };
        let mut pane_prims: Vec<Prim> = Vec::new();

        let visible = self.visible_data_range();
        if let Some((from, to)) = visible {
            self.build_grid(&mut pane_prims, &time_marks, from, to, pane_w_px as i32, pane_h_px as i32, hpr, vpr);

            // draw each series (snapshot to avoid borrowing self.series during the calls)
            let series: Vec<(SeriesId, SeriesKind, Color)> =
                self.series.iter().map(|s| (s.id, s.kind, s.line_color)).collect();
            for (id, kind, color) in series {
                match kind {
                    SeriesKind::Candlestick => self.build_candle_prims(id, from, to, hpr, vpr, &mut pane_prims),
                    SeriesKind::Bar => self.build_bar_prims(id, from, to, hpr, vpr, &mut pane_prims),
                    SeriesKind::Histogram => self.build_histogram_prims(id, from, to, hpr, vpr, &mut pane_prims),
                    SeriesKind::Line | SeriesKind::Area => {
                        self.build_line_prims(id, kind, color, from, to, pane_h, hpr, vpr, &mut pane_group)
                    }
                }
            }
            self.build_last_value_line(&mut pane_prims, pane_w_px as i32, vpr);
        }

        let mut crosshair_stroke: Vec<TriVertex> = Vec::new();
        self.build_crosshair_lines(&mut pane_prims, &mut crosshair_stroke, pane_w_px as i32, pane_h_px as i32, pane_w, pane_h, hpr, vpr);
        pane_group.stroke_tris.extend(crosshair_stroke);
        prims_to_instances(&pane_prims, &mut pane_group.quads);

        self.gfx.msaa.ensure(&self.gfx.device, self.gfx.config.format, self.gfx.config.width, self.gfx.config.height);

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
            wgpu::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
            &self.gfx.quad_renderer,
            &self.gfx.tex_renderer,
            &self.gfx.tri_renderer,
            &[pane_group],
        );
        frame.present();

        self.draw_axes_2d(visible, &time_marks)?;
        Ok(())
    }

    // --- data / scale bookkeeping ---

    fn on_time_points_changed(&mut self) {
        let n = self.data.merged_times().len();
        let mut weights = vec![0u8; n];
        fill_weights_for_points(self.data.merged_times(), &mut weights, 0);
        self.tick_marks.set_weights(&weights);
        self.time_scale.set_points_len(n);
        self.time_scale.set_base_index(self.data.base_index());
        // keep geometry current so fit_content/zoom right after set_data use the real width
        if self.css_width > 0.0 {
            self.recompute_layout();
        }
    }

    fn main_plot(&self) -> &PlotList {
        self.data.plot(self.series[0].id)
    }

    fn visible_data_range(&self) -> Option<(i64, i64)> {
        let n = self.data.merged_times().len() as i64;
        if n == 0 || self.time_scale.is_empty() {
            return None;
        }
        let range = self.time_scale.visible_strict_range()?;
        let from = range.left().max(0);
        let to = range.right().min(n - 1);
        if from > to {
            return None;
        }
        Some((from, to))
    }

    /// Autoscale over the union of all series' visible min/max on the (single) price scale.
    fn autoscale(&mut self, from: i64, to: i64) {
        let mut merged: Option<PriceRange> = None;
        let ids: Vec<SeriesId> = self.series.iter().map(|s| s.id).collect();
        for id in ids {
            if let Some(mm) = self
                .data
                .plot_mut(id)
                .min_max_on_range_cached(from, to, &[PlotValueIndex::Low, PlotValueIndex::High])
            {
                let r = PriceRange::new(mm.min, mm.max);
                merged = Some(match merged {
                    None => r,
                    Some(m) => m.merge(Some(&r)),
                });
            }
        }
        if let Some(r) = merged {
            self.price_scale.apply_autoscale_range(Some(r), 0.01);
        }
    }

    fn measure(&self, text: &str) -> f64 {
        self.axis_ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * self.dpr));
        let device_w = self.axis_ctx.measure_text(text).map(|m| m.width()).unwrap_or(0.0);
        device_w / self.dpr
    }

    fn compute_price_axis_width(&self) -> f64 {
        let mut max_text_w = 0f64;
        for mark in self.price_scale.build_tick_marks(100, 0.0) {
            max_text_w = max_text_w.max(self.measure(&self.price_formatter.format(mark.logical)));
        }
        if let Some((_, y)) = self.crosshair {
            if !self.price_scale.is_empty() {
                let price = self.price_scale.coordinate_to_price(y, 0.0);
                max_text_w = max_text_w.max(self.measure(&self.price_formatter.format(price)));
            }
        }
        if !self.main_plot().is_empty() {
            let last = self.main_plot().size() - 1;
            let close = self.main_plot().value_at(last, PlotValueIndex::Close);
            max_text_w = max_text_w.max(self.measure(&self.price_formatter.format(close)));
        }
        let text_w = if max_text_w > 0.0 { max_text_w } else { PRICE_DEFAULT_TEXT_WIDTH };
        let w = (AXIS_BORDER_SIZE + AXIS_TICK_LENGTH + PRICE_PADDING_INNER + PRICE_PADDING_OUTER + PRICE_LABEL_OFFSET + text_w).ceil();
        w + (w as i64 % 2) as f64
    }

    // ---- GPU pane builders (each iterates its series' rows in the visible window) ----

    #[allow(clippy::too_many_arguments)]
    fn build_grid(&self, prims: &mut Vec<Prim>, time_marks: &[(i64, u8)], from: i64, to: i64, pane_w_px: i32, pane_h_px: i32, hpr: f64, vpr: f64) {
        let line_width = 1f64.max(hpr.floor()) as i32;
        for &(index, _weight) in time_marks {
            if index < from || index > to {
                continue;
            }
            let x = (self.time_scale.index_to_coordinate(index) * hpr).round() as i32;
            prims.push(Prim::VLine { x, y0: -line_width, y1: pane_h_px + line_width, width: line_width, style: LineStyle::Solid, color: GRID_COLOR });
        }
        for mark in self.price_scale.build_tick_marks(100, 0.0) {
            let y = (mark.coord * vpr).round() as i32;
            prims.push(Prim::HLine { y, x0: -line_width, x1: pane_w_px + line_width, width: line_width, style: LineStyle::Solid, color: GRID_COLOR });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_candle_prims(&self, id: SeriesId, from: i64, to: i64, hpr: f64, vpr: f64, prims: &mut Vec<Prim>) {
        let plot = self.data.plot(id);
        let idxs = plot.indices();
        let (o, h, l, c) = (
            plot.column(PlotValueIndex::Open),
            plot.column(PlotValueIndex::High),
            plot.column(PlotValueIndex::Low),
            plot.column(PlotValueIndex::Close),
        );
        let mut items = Vec::new();
        for r in plot.visible_rows(from, to) {
            let (open, high, low, close) = (o[r], h[r], l[r], c[r]);
            let color = if close >= open { UP_COLOR } else { DOWN_COLOR };
            items.push(CandleItem {
                x: self.time_scale.index_to_coordinate(idxs[r]),
                open_y: self.price_scale.price_to_coordinate(open, close),
                high_y: self.price_scale.price_to_coordinate(high, close),
                low_y: self.price_scale.price_to_coordinate(low, close),
                close_y: self.price_scale.price_to_coordinate(close, close),
                body_color: color,
                border_color: color,
                wick_color: color,
            });
        }
        build_candles(&items, &CandlesParams { bar_spacing: self.time_scale.bar_spacing(), horizontal_pixel_ratio: hpr, vertical_pixel_ratio: vpr, wick_visible: true, border_visible: true }, prims);
    }

    #[allow(clippy::too_many_arguments)]
    fn build_bar_prims(&self, id: SeriesId, from: i64, to: i64, hpr: f64, vpr: f64, prims: &mut Vec<Prim>) {
        let plot = self.data.plot(id);
        let idxs = plot.indices();
        let (o, h, l, c) = (
            plot.column(PlotValueIndex::Open),
            plot.column(PlotValueIndex::High),
            plot.column(PlotValueIndex::Low),
            plot.column(PlotValueIndex::Close),
        );
        let mut items = Vec::new();
        for r in plot.visible_rows(from, to) {
            let (open, high, low, close) = (o[r], h[r], l[r], c[r]);
            items.push(BarItem {
                x: self.time_scale.index_to_coordinate(idxs[r]),
                open_y: self.price_scale.price_to_coordinate(open, close),
                high_y: self.price_scale.price_to_coordinate(high, close),
                low_y: self.price_scale.price_to_coordinate(low, close),
                close_y: self.price_scale.price_to_coordinate(close, close),
                color: if close >= open { UP_COLOR } else { DOWN_COLOR },
            });
        }
        build_bars(&items, &BarsParams { bar_spacing: self.time_scale.bar_spacing(), horizontal_pixel_ratio: hpr, vertical_pixel_ratio: vpr, open_visible: true, thin_bars: true }, prims);
    }

    #[allow(clippy::too_many_arguments)]
    fn build_histogram_prims(&self, id: SeriesId, from: i64, to: i64, hpr: f64, vpr: f64, prims: &mut Vec<Prim>) {
        let plot = self.data.plot(id);
        let idxs = plot.indices();
        let c = plot.column(PlotValueIndex::Close);
        // base = coordinate of price 0 (histogram grows from the bottom for volume-like data)
        let base = self.price_scale.price_to_coordinate(0.0, 0.0);
        let mut items = Vec::new();
        for r in plot.visible_rows(from, to) {
            let value = c[r];
            items.push(HistogramItem {
                x: self.time_scale.index_to_coordinate(idxs[r]),
                y: self.price_scale.price_to_coordinate(value, value),
                time: idxs[r],
                color: HISTOGRAM_COLOR,
            });
        }
        build_histogram(&items, &HistogramParams { bar_spacing: self.time_scale.bar_spacing(), horizontal_pixel_ratio: hpr, vertical_pixel_ratio: vpr, histogram_base: base }, prims);
    }

    #[allow(clippy::too_many_arguments)]
    fn build_line_prims(&self, id: SeriesId, kind: SeriesKind, color: Color, from: i64, to: i64, pane_h: f64, hpr: f64, vpr: f64, group: &mut DrawGroup) {
        let plot = self.data.plot(id);
        let idxs = plot.indices();
        let c = plot.column(PlotValueIndex::Close);
        let mut points: Vec<LinePoint> = Vec::new();
        for r in plot.visible_rows(from, to) {
            let close = c[r];
            points.push(LinePoint { x: self.time_scale.index_to_coordinate(idxs[r]), y: self.price_scale.price_to_coordinate(close, close) });
        }

        let params = LineParams { horizontal_pixel_ratio: hpr, vertical_pixel_ratio: vpr, line_width: DEFAULT_LINE_WIDTH, line_type: LineType::Simple };
        // default line color per kind unless overridden (line_color != LINE_COLOR sentinel)
        let line_color = if color != LINE_COLOR {
            color
        } else if kind == SeriesKind::Area {
            AREA_LINE_COLOR
        } else {
            LINE_COLOR
        };

        if kind == SeriesKind::Area {
            let mut area = AreaMesh::default();
            build_area_fill(&points, pane_h, AREA_TOP_COLOR, AREA_BOTTOM_COLOR, &params, &mut area);
            group.fill_tris.extend(area.vertices.iter().map(mesh_vertex));
        }
        let mut stroke = StrokeMesh::default();
        build_line_stroke(&points, line_color, &params, &mut stroke);
        group.stroke_tris.extend(stroke.vertices.iter().map(mesh_vertex));
    }

    /// Dashed line at the main series' last close (priceLineSource LastBar).
    fn build_last_value_line(&self, prims: &mut Vec<Prim>, pane_w_px: i32, vpr: f64) {
        if self.main_plot().is_empty() || self.price_scale.is_empty() {
            return;
        }
        let last = self.main_plot().size() - 1;
        let close = self.main_plot().value_at(last, PlotValueIndex::Close);
        let y = self.price_scale.price_to_coordinate(close, close);
        let width = 1f64.max(vpr.floor()) as i32;
        prims.push(Prim::HLine { y: (y * vpr).round() as i32, x0: 0, x1: pane_w_px, width, style: LineStyle::Dashed, color: self.last_value_color() });
    }

    fn last_value_color(&self) -> Color {
        match self.series[0].kind {
            SeriesKind::Line => LINE_COLOR,
            SeriesKind::Area => AREA_LINE_COLOR,
            SeriesKind::Histogram => HISTOGRAM_COLOR,
            _ => {
                let last = self.main_plot().size() - 1;
                let open = self.main_plot().value_at(last, PlotValueIndex::Open);
                let close = self.main_plot().value_at(last, PlotValueIndex::Close);
                if close >= open { UP_COLOR } else { DOWN_COLOR }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_crosshair_lines(&self, prims: &mut Vec<Prim>, group_stroke: &mut Vec<TriVertex>, pane_w_px: i32, pane_h_px: i32, pane_w: f64, pane_h: f64, hpr: f64, vpr: f64) {
        let Some((x_css, y_css)) = self.crosshair else { return };
        if self.main_plot().is_empty() || self.time_scale.is_empty() {
            return;
        }
        if x_css > pane_w || y_css > pane_h || self.crosshair_mode == CrosshairMode::Hidden {
            return;
        }

        let snapped_x = self.snapped_crosshair_x(x_css);
        let (_price, snap_y) = self.crosshair_snap(x_css, y_css);
        let line_width = 1f64.max(hpr.floor()) as i32;

        prims.push(Prim::VLine { x: (snapped_x * hpr).round() as i32, y0: 0, y1: pane_h_px, width: line_width, style: LineStyle::LargeDashed, color: CROSSHAIR_COLOR });
        prims.push(Prim::HLine { y: (snap_y * vpr).round() as i32, x0: 0, x1: pane_w_px, width: line_width, style: LineStyle::LargeDashed, color: CROSSHAIR_COLOR });

        // crosshair marker on line/area main series
        if matches!(self.series[0].kind, SeriesKind::Line | SeriesKind::Area) {
            let index = self.snapped_crosshair_index(x_css);
            if let Some(row) = self.main_plot().search(index, aion_core::model::plot_list::MismatchDirection::None) {
                let close = self.main_plot().value_at(row, PlotValueIndex::Close);
                let cx = (self.time_scale.index_to_coordinate(index) * hpr) as f32;
                let cy = (self.price_scale.price_to_coordinate(close, close) * vpr) as f32;
                let fill = if self.series[0].kind == SeriesKind::Area { AREA_LINE_COLOR } else { LINE_COLOR };
                let outer_r = ((CROSSHAIR_MARKER_RADIUS + CROSSHAIR_MARKER_BORDER_WIDTH) * vpr) as f32;
                let inner_r = (CROSSHAIR_MARKER_RADIUS * vpr) as f32;
                let mut disc = Vec::new();
                build_disc([cx, cy], outer_r, MARKER_BORDER_COLOR, &mut disc);
                build_disc([cx, cy], inner_r, fill, &mut disc);
                group_stroke.extend(disc.iter().map(mesh_vertex));
            }
        }
    }

    fn snapped_crosshair_index(&self, x_css: f64) -> i64 {
        let mut index = self.time_scale.coordinate_to_index(x_css);
        if let Some(range) = self.time_scale.visible_strict_range() {
            index = index.clamp(range.left(), range.right());
        }
        let n = self.data.merged_times().len() as i64;
        index.clamp(0, (n - 1).max(0))
    }

    fn snapped_crosshair_x(&self, x_css: f64) -> f64 {
        self.time_scale.index_to_coordinate(self.snapped_crosshair_index(x_css))
    }

    /// Magnet-snapped crosshair price + pane y (main series). RENDERING_SPEC.md §8.
    fn crosshair_snap(&self, x_css: f64, y_css: f64) -> (f64, f64) {
        let index = self.snapped_crosshair_index(x_css);
        let plot = self.main_plot();
        let row = plot.search(index, aion_core::model::plot_list::MismatchDirection::NearestLeft);
        let close = row.map(|r| plot.value_at(r, PlotValueIndex::Close));

        let Some(close) = close else {
            return (self.price_scale.coordinate_to_price(y_css, 0.0), y_css);
        };

        let price = match self.crosshair_mode {
            CrosshairMode::Normal | CrosshairMode::Hidden => {
                return (self.price_scale.coordinate_to_price(y_css, close), y_css)
            }
            CrosshairMode::Magnet => close,
            CrosshairMode::MagnetOhlc => {
                let row = row.expect("close present");
                let open = plot.value_at(row, PlotValueIndex::Open);
                let high = plot.value_at(row, PlotValueIndex::High);
                let low = plot.value_at(row, PlotValueIndex::Low);
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

    // ---- Canvas2D axis overlay ----

    fn draw_axes_2d(&self, visible: Option<(i64, i64)>, time_marks: &[(i64, u8)]) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        let dpr = self.dpr;
        let bitmap_w = self.gfx.config.width as f64;
        let bitmap_h = self.gfx.config.height as f64;
        let pane_w = self.pane_w;
        let pane_h = self.pane_h;

        ctx.clear_rect(0.0, 0.0, bitmap_w, bitmap_h);
        let font = format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr);
        let border_w = 1f64.max(dpr.floor());

        ctx.set_fill_style_str(BORDER_CSS);
        ctx.fill_rect((pane_w * dpr).round(), 0.0, border_w, (pane_h * dpr).round());
        ctx.fill_rect(0.0, (pane_h * dpr).round(), bitmap_w, border_w);

        ctx.set_font(&font);
        ctx.set_text_baseline("middle");
        ctx.set_text_align("left");
        ctx.set_fill_style_str(TEXT_CSS);
        let text_x = (pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER) * dpr;
        for mark in self.price_scale.build_tick_marks(100, 0.0) {
            ctx.fill_text(&self.price_formatter.format(mark.logical), text_x, (mark.coord * dpr).round())?;
        }

        if let Some((from, to)) = visible {
            ctx.set_text_align("center");
            ctx.set_fill_style_str(TEXT_CSS);
            let y_center = pane_h + AXIS_BORDER_SIZE + AXIS_TICK_LENGTH + TIME_PADDING_TOP + FONT_SIZE / 2.0;
            let times = self.data.merged_times();
            for &(index, weight) in time_marks {
                if index < from || index > to {
                    continue;
                }
                let ts = times[index as usize];
                let mark_type = weight_to_tick_mark_type(weight, self.time_visible, false);
                let label = format_tick_label(ts, mark_type);
                let x_center = self.time_scale.index_to_coordinate(index);
                ctx.fill_text(&label, (x_center * dpr).round(), (y_center * dpr).round())?;
            }
        }

        self.draw_last_value_label_2d(pane_w, pane_h, dpr)?;
        self.draw_crosshair_labels_2d(pane_w, pane_h, dpr, &font)?;
        Ok(())
    }

    fn draw_last_value_label_2d(&self, pane_w: f64, pane_h: f64, dpr: f64) -> Result<(), JsValue> {
        if self.main_plot().is_empty() || self.price_scale.is_empty() {
            return Ok(());
        }
        let last = self.main_plot().size() - 1;
        let close = self.main_plot().value_at(last, PlotValueIndex::Close);
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
        let box_w = ((AXIS_BORDER_SIZE + PRICE_PADDING_INNER + PRICE_PADDING_OUTER + AXIS_TICK_LENGTH + text_w) * dpr).round();
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

    fn draw_crosshair_labels_2d(&self, pane_w: f64, pane_h: f64, dpr: f64, font: &str) -> Result<(), JsValue> {
        let Some((x_css, y_css)) = self.crosshair else { return Ok(()) };
        if self.main_plot().is_empty() || self.time_scale.is_empty() || self.crosshair_mode == CrosshairMode::Hidden {
            return Ok(());
        }
        let ctx = &self.axis_ctx;
        ctx.set_font(font);
        ctx.set_text_baseline("middle");

        if y_css <= pane_h && !self.price_scale.is_empty() {
            let (price, snap_y) = self.crosshair_snap(x_css, y_css);
            let label = self.price_formatter.format(price);
            let text_w = self.measure(&label);
            let box_h = ((FONT_SIZE + PRICE_LABEL_PADDING_TB * 2.0) * dpr).round();
            let box_w = ((AXIS_BORDER_SIZE + PRICE_PADDING_INNER + PRICE_PADDING_OUTER + AXIS_TICK_LENGTH + text_w) * dpr).round();
            let box_x = (pane_w * dpr).round();
            let box_y = ((snap_y * dpr).round() - box_h / 2.0).round();
            ctx.set_fill_style_str(LABEL_BG_CSS);
            ctx.fill_rect(box_x, box_y, box_w, box_h);
            ctx.set_text_align("left");
            ctx.set_fill_style_str(WHITE_CSS);
            let text_x = (pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER) * dpr;
            ctx.fill_text(&label, text_x, (snap_y * dpr).round())?;
        }

        if x_css <= pane_w {
            let index = self.snapped_crosshair_index(x_css);
            let ts = self.data.merged_times()[index as usize];
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
