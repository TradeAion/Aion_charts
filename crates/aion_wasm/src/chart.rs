//! The chart object exported to JS.
//!
//! Hybrid rendering, mirroring lightweight-charts' per-cell canvas layout:
//! - the **pane** (grid, series, crosshair lines) is drawn with WebGPU or the shared Canvas2D
//!   fallback;
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

use aion_core::format::time_formatter::{
    format_crosshair_time, format_tick_label, weight_to_tick_mark_type,
};
use aion_core::model::data_layer::SeriesId;
use aion_core::model::data_validation::sanitize_ohlc;
use aion_core::model::magnet::{magnet_snap, CrosshairMode};
use aion_core::model::plot_list::{PlotList, PlotValueIndex};
use aion_core::model::range::{LogicalRange, StrictRange};
use aion_core::options::{crosshair_mode, ChartOptions};
use aion_core::TimePointIndex;
use aion_core::scale::price_scale_core::PriceScaleCore;
use aion_engine::{
    line_style_from_u8, marker_pos, marker_shape, ChartEngine, Marker, Pane, PriceLine, SeriesKind,
};
use aion_render::canvas2d::{execute as execute_canvas2d, Canvas2d, Viewport as CanvasViewport};
use aion_render::color::Color;
use aion_render::draw_list::LineType;
use aion_render_wgpu::{
    geom_prims_to_tris, prims_to_instances, render_frame, DrawGroup, LabelAtlas, MsaaTarget,
    QuadRenderer, TexQuadRenderer, TriRenderer, SAMPLE_COUNT,
};

// lightweight-charts default palette (RENDERING_SPEC.md §2.5, §7, §8, §15)
const UP_COLOR: Color = Color::rgb(0x26, 0xa6, 0x9a);
const DOWN_COLOR: Color = Color::rgb(0xef, 0x53, 0x50);

// Axis palette (as CSS color strings for the 2D overlay)
const BORDER_CSS: &str = "#2B2B43";
const LABEL_BG_CSS: &str = "#131722";
const TEXT_CSS: &str = "#191919";
const WHITE_CSS: &str = "#FFFFFF";

// Line/Area series defaults (line-series.ts / area-series.ts)
const LINE_COLOR: Color = Color::rgb(0x21, 0x96, 0xf3);
const AREA_LINE_COLOR: Color = Color::rgb(0x33, 0xd7, 0x78);
const HISTOGRAM_COLOR: Color = Color::rgba(0x26, 0xa6, 0x9a, 0x80);
// TradingView-style volume: translucent green on up bars, red on down bars.


// Crosshair marker (line/area) — line-series.ts defaults.

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

/// JSON shape accepted from the JS boundary for `set_series_markers`.
#[derive(serde::Deserialize)]
struct MarkerInput {
    time: f64,
    #[serde(default)]
    position: String,
    #[serde(default)]
    shape: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    text: String,
}

fn crosshair_mode_from_u8(mode: u8) -> CrosshairMode {
    match mode {
        crosshair_mode::NORMAL => CrosshairMode::Normal,
        crosshair_mode::HIDDEN => CrosshairMode::Hidden,
        crosshair_mode::MAGNET_OHLC => CrosshairMode::MagnetOhlc,
        _ => CrosshairMode::Magnet,
    }
}

/// Height (css px) of the separator between stacked panes.
const PANE_SEPARATOR: f64 = 1.0;

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
    gfx: Option<Gfx>,
    pane_ctx: Option<CanvasRenderingContext2d>,
    axis_ctx: CanvasRenderingContext2d,
    bitmap_w: u32,
    bitmap_h: u32,
    engine: ChartEngine,
}
impl std::ops::Deref for ChartInner {
    type Target = ChartEngine;

    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

impl std::ops::DerefMut for ChartInner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.engine
    }
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

/// Creates a chart bound to `pane_canvas` (WebGPU, falling back to Canvas2D) and `overlay_canvas`
/// (Canvas2D axis/text layer). Both must be full chart size with bitmap size = css size * dpr,
/// already set by the caller.
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

    let bitmap_w = (css_width * dpr).round().max(1.0) as u32;
    let bitmap_h = (css_height * dpr).round().max(1.0) as u32;
    let (gfx, pane_ctx) = match try_create_gfx(pane_canvas, css_width, css_height, dpr).await {
        Ok(gfx) => (Some(gfx), None),
        Err(error) => {
            web_sys::console::warn_1(
                &format!("aion: WebGPU unavailable; using Canvas2D fallback ({error:?})").into(),
            );
            let pane_ctx = pane_el
                .get_context("2d")?
                .ok_or_else(|| JsValue::from_str("no 2d pane context"))?
                .dyn_into::<CanvasRenderingContext2d>()?;
            (None, Some(pane_ctx))
        }
    };

    let inner = ChartInner {
        gfx,
        pane_ctx,
        axis_ctx,
        bitmap_w,
        bitmap_h,
        engine: ChartEngine::new(css_width, css_height, dpr),
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

    /// Streaming update of an arbitrary series by id (append new time or replace last).
    pub fn update_series_bar(&mut self, series_id: u32, time: f64, open: f64, high: f64, low: f64, close: f64) {
        self.inner.borrow_mut().update_series_bar(series_id, time, open, high, low, close);
    }

    /// Sets a series' line/area color (overrides the kind default).
    pub fn set_series_color(&mut self, id: u32, r: u8, g: u8, b: u8) {
        self.inner.borrow_mut().set_series_color(id, r, g, b);
    }

    /// Set candlestick/bar up & down body colors as CSS strings (empty string = keep default).
    pub fn set_series_updown_colors(&mut self, id: u32, up: &str, down: &str) {
        self.inner.borrow_mut().set_series_updown_colors(id, up, down);
    }

    /// Set a line/area series' stroke width (css px).
    pub fn set_series_line_width(&mut self, id: u32, width: f64) {
        self.inner.borrow_mut().set_series_line_width(id, width);
    }

    /// Set an area series' fill gradient colors (top at the line, bottom at the base; CSS strings).
    pub fn set_series_area_colors(&mut self, id: u32, top: &str, bottom: &str) {
        self.inner.borrow_mut().set_series_area_colors(id, top, bottom);
    }

    /// Color a histogram (volume) by the main price series' up/down direction per bar
    /// (TradingView-style volume).
    pub fn set_series_histogram_updown(&mut self, id: u32, enabled: bool) {
        self.inner.borrow_mut().set_series_histogram_updown(id, enabled);
    }

    /// Set a line/area series' join type: 0 = simple, 1 = stepped, 2 = curved. Call `render()`
    /// after (roadmap Phase B3).
    pub fn set_series_line_type(&mut self, id: u32, line_type: u8) {
        self.inner.borrow_mut().set_series_line_type(id, line_type);
    }

    /// Toggle per-point disc markers on a line/area series. Call `render()` after (Phase B3).
    pub fn set_series_point_markers(&mut self, id: u32, visible: bool) {
        self.inner.borrow_mut().set_series_point_markers(id, visible);
    }

    /// Set a Baseline series' baseline price (`NaN` = auto). Call `render()` after (Phase B3).
    pub fn set_series_baseline(&mut self, id: u32, price: f64) {
        self.inner.borrow_mut().set_series_baseline(id, price);
    }

    /// Toggle the pulsing last-price ring on a series (roadmap Phase B3).
    pub fn set_series_last_price_animation(&mut self, id: u32, enabled: bool) {
        self.inner.borrow_mut().set_series_last_price_animation(id, enabled);
    }

    /// Add a horizontal price line to a series; returns its id. `style`: 0 solid, 1 dotted, 2
    /// dashed, 3 large-dashed, 4 sparse-dotted. Call `render()` after (roadmap Phase B4).
    #[allow(clippy::too_many_arguments)]
    pub fn create_price_line(&mut self, series_id: u32, price: f64, r: u8, g: u8, b: u8, width: u32, style: u8, title: &str) -> u32 {
        self.inner.borrow_mut().create_price_line(series_id, price, r, g, b, width, style, title)
    }
    /// Remove a price line by id. Call `render()` after (roadmap Phase B4).
    pub fn remove_price_line(&mut self, id: u32) {
        self.inner.borrow_mut().remove_price_line(id);
    }

    /// Replace a series' markers from a JSON array. Call `render()` after (roadmap Phase B4).
    pub fn set_series_markers(&mut self, series_id: u32, json: &str) {
        self.inner.borrow_mut().set_series_markers(series_id, json);
    }
    /// Whether any series wants the last-price pulse (host uses this to run/stop its rAF loop).
    pub fn wants_animation(&self) -> bool {
        self.inner.borrow().wants_animation()
    }
    /// Set the host animation clock (ms). Call before `render()` in the rAF loop (Phase B3).
    pub fn set_animation_time(&mut self, t_ms: f64) {
        self.inner.borrow_mut().set_animation_time(t_ms);
    }

    /// Move a series to the bottom-band overlay (volume) price scale with the given fractional
    /// margins (top/bottom of pane height). Call `render()` after (roadmap Phase B2).
    pub fn set_series_overlay(&mut self, id: u32, top: f64, bottom: f64) {
        self.inner.borrow_mut().set_series_overlay(id, top, bottom);
    }

    /// Move a series into stacked pane `pane_index` (0 = top/price pane), creating panes as needed;
    /// `stretch_factor` sizes a newly-created pane relative to the others. Call `render()` after
    /// (roadmap Phase B1).
    pub fn set_series_pane(&mut self, id: u32, pane_index: usize, stretch_factor: f64) {
        self.inner.borrow_mut().set_series_pane(id, pane_index, stretch_factor);
    }

    /// Number of stacked panes.
    pub fn pane_count(&self) -> usize {
        self.inner.borrow().pane_count()
    }
    /// CSS Y of each pane boundary (for the host to hit-test separators).
    pub fn pane_separator_ys(&self) -> Vec<f64> {
        self.inner.borrow().pane_separator_ys()
    }
    /// Drag the separator below pane `i` by `delta_css`. Call `render()` after (roadmap Phase B1).
    pub fn drag_pane_separator(&mut self, i: usize, delta_css: f64) {
        self.inner.borrow_mut().drag_pane_separator(i, delta_css);
    }
    /// CSS height of pane `i` from the last layout pass (0 if out of range).
    pub fn pane_height(&self, i: usize) -> f64 {
        self.inner.borrow().pane_height(i)
    }
    /// Relative stretch factor of pane `i` (1 if out of range).
    pub fn pane_stretch(&self, i: usize) -> f64 {
        self.inner.borrow().pane_stretch(i)
    }
    /// Set pane `i`'s stretch factor (relative height weight). Call `render()` after.
    pub fn set_pane_stretch(&mut self, i: usize, factor: f64) {
        self.inner.borrow_mut().set_pane_stretch(i, factor);
    }
    /// Resize pane `i` to `height_css` px, taking the difference from its neighbour. Render after.
    pub fn set_pane_height(&mut self, i: usize, height_css: f64) {
        self.inner.borrow_mut().set_pane_height(i, height_css);
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

    /// Deep-merge a JSON options patch (LWC `applyOptions` semantics) — e.g.
    /// `{"grid":{"vertLines":{"color":"#334"}},"layout":{"background":{"color":"#111"}}}`.
    /// Malformed JSON is ignored with a console warning. Call `render()` after (roadmap Phase A2).
    pub fn apply_options(&mut self, patch_json: &str) {
        self.inner.borrow_mut().apply_options(patch_json);
    }

    /// Current (deep-merged) chart options as a JSON string.
    pub fn options_json(&self) -> String {
        self.inner.borrow().options_json()
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
    /// Float logical (bar) index under X (CSS px), or `undefined` if there is no data.
    pub fn coordinate_to_logical(&self, x_css: f64) -> Option<f64> {
        self.inner.borrow().coordinate_to_logical(x_css)
    }
    /// Per-series OHLC at the bar under X (CSS px) as a flat `[id, o, h, l, c, ...]` Float64Array
    /// (see the inner method); empty off-chart. Backs crosshair/click `seriesData`.
    pub fn hover_data(&self, x_css: f64) -> Vec<f64> {
        self.inner.borrow().hover_data(x_css)
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
        let id = self.engine.add_series(SeriesKind::from_u8(kind));
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
        self.engine.install_series_data(id as SeriesId, s.times, s.open, s.high, s.low, s.close);
    }

    /// Streaming update of the main series (append new time or replace last).
    pub fn update_bar(&mut self, time: f64, open: f64, high: f64, low: f64, close: f64) {
        let id = self.series[0].id as u32;
        self.update_series_bar(id, time, open, high, low, close);
    }

    /// Streaming update of the series with `series_id` (append a new time or replace the last).
    pub fn update_series_bar(&mut self, series_id: u32, time: f64, open: f64, high: f64, low: f64, close: f64) {
        // Ignore updates to an unknown series rather than corrupting the data layer.
        if !self.series.iter().any(|s| s.id == series_id as SeriesId) {
            web_sys::console::warn_1(&"aion: update_bar for unknown series id".into());
            return;
        }
        // Drop a bad tick rather than corrupting the series (roadmap Phase A3).
        if !self.engine.update_series_bar(series_id as SeriesId, time, [open, high, low, close]) {
            web_sys::console::warn_1(&"aion: update_bar dropped a non-finite point".into());
            return;
        }
    }

    /// Sets a series' line/area color (overrides the kind default).
    pub fn set_series_color(&mut self, id: u32, r: u8, g: u8, b: u8) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.line_color = Color::rgb(r, g, b);
        }
    }

    /// Set candlestick/bar up & down body colors (CSS strings; empty/unparseable = keep default).
    pub fn set_series_updown_colors(&mut self, id: u32, up: &str, down: &str) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            if let Some(c) = Color::parse_css(up) {
                s.up_color = Some(c);
            }
            if let Some(c) = Color::parse_css(down) {
                s.down_color = Some(c);
            }
        }
    }

    /// Set a line/area series' stroke width (css px; non-positive ignored).
    pub fn set_series_line_width(&mut self, id: u32, width: f64) {
        if width > 0.0 {
            if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
                s.line_width = Some(width);
            }
        }
    }

    /// Set an area series' fill gradient colors (top at the line, bottom at the base; CSS strings).
    pub fn set_series_area_colors(&mut self, id: u32, top: &str, bottom: &str) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            if let Some(c) = Color::parse_css(top) {
                s.area_top_color = Some(c);
            }
            if let Some(c) = Color::parse_css(bottom) {
                s.area_bottom_color = Some(c);
            }
        }
    }

    /// Color a histogram by the main price series' up/down direction per bar (TradingView volume).
    pub fn set_series_histogram_updown(&mut self, id: u32, enabled: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.histogram_updown = enabled;
        }
    }

    /// Set a line/area series' join type: 0 = simple, 1 = stepped, 2 = curved (roadmap Phase B3).
    pub fn set_series_line_type(&mut self, id: u32, line_type: u8) {
        let lt = match line_type {
            1 => LineType::WithSteps,
            2 => LineType::Curved,
            _ => LineType::Simple,
        };
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.line_type = lt;
        }
    }

    /// Toggle per-point disc markers on a line/area series (roadmap Phase B3).
    pub fn set_series_point_markers(&mut self, id: u32, visible: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.point_markers = visible;
        }
    }

    /// Set a Baseline series' baseline price. `NaN` resets to auto (visible-range midpoint).
    pub fn set_series_baseline(&mut self, id: u32, price: f64) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.baseline = if price.is_finite() { Some(price) } else { None };
        }
    }

    /// Add a horizontal price line to a series; returns its id (roadmap Phase B4).
    #[allow(clippy::too_many_arguments)]
    pub fn create_price_line(&mut self, series_id: u32, price: f64, r: u8, g: u8, b: u8, width: u32, style: u8, title: &str) -> u32 {
        let id = self.next_price_line_id;
        self.next_price_line_id += 1;
        if let Some(s) = self.series.iter_mut().find(|s| s.id == series_id as SeriesId) {
            s.price_lines.push(PriceLine {
                id,
                price,
                color: Color::rgb(r, g, b),
                width: width.max(1) as i32,
                style: line_style_from_u8(style),
                title: title.to_string(),
            });
        }
        id
    }

    /// Remove a price line by id (from whichever series holds it).
    pub fn remove_price_line(&mut self, id: u32) {
        for s in &mut self.series {
            s.price_lines.retain(|pl| pl.id != id);
        }
    }

    /// Replace a series' markers from a JSON array `[{time, position, shape, color, text}]`
    /// (position: above|below|inBar; shape: circle|square|arrowUp|arrowDown). Roadmap Phase B4.
    pub fn set_series_markers(&mut self, series_id: u32, json: &str) {
        let inputs: Vec<MarkerInput> = serde_json::from_str(json).unwrap_or_default();
        let markers: Vec<Marker> = inputs
            .into_iter()
            .map(|m| Marker {
                time: m.time as i64,
                position: match m.position.as_str() {
                    "below" | "belowBar" => marker_pos::BELOW,
                    "inBar" | "in" => marker_pos::IN_BAR,
                    _ => marker_pos::ABOVE,
                },
                shape: match m.shape.as_str() {
                    "square" => marker_shape::SQUARE,
                    "arrowUp" | "arrow_up" => marker_shape::ARROW_UP,
                    "arrowDown" | "arrow_down" => marker_shape::ARROW_DOWN,
                    _ => marker_shape::CIRCLE,
                },
                color: Color::parse_css(&m.color).unwrap_or(Color::rgb(0x21, 0x96, 0xf3)),
                text: m.text,
            })
            .collect();
        if let Some(s) = self.series.iter_mut().find(|s| s.id == series_id as SeriesId) {
            s.markers = markers;
        }
    }

    /// Toggle the pulsing last-price ring on a series (roadmap Phase B3).
    pub fn set_series_last_price_animation(&mut self, id: u32, enabled: bool) {
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.last_price_animation = enabled;
        }
    }

    /// Whether any series wants the last-price pulse (so the host can start/stop its rAF loop).
    pub fn wants_animation(&self) -> bool {
        self.series.iter().any(|s| s.last_price_animation)
    }

    /// Set the host animation clock (ms). The shell's rAF loop calls this then `render()`.
    pub fn set_animation_time(&mut self, t_ms: f64) {
        self.animation_time = t_ms;
    }

    /// Move a series onto its pane's bottom-band overlay scale (volume-style) and set that band's
    /// margins as fractions of the pane slot: `top` leaves that fraction above the band, `bottom`
    /// below it (e.g. top=0.8, bottom=0.0 ⇒ bottom 20%). Excludes the series from the pane's main
    /// autoscale (roadmap Phase B2).
    pub fn set_series_overlay(&mut self, id: u32, top: f64, bottom: f64) {
        let mut pane_index = 0;
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.overlay = true;
            pane_index = s.pane_index;
        }
        if let Some(p) = self.panes.get_mut(pane_index) {
            p.overlay_top = top.clamp(0.0, 1.0);
            p.overlay_bottom = bottom.clamp(0.0, 1.0);
        }
    }

    /// Move a series into pane `pane_index`, creating panes (with the given stretch factor for a
    /// newly-created last pane) as needed. Pane 0 is the top/price pane (roadmap Phase B1).
    pub fn set_series_pane(&mut self, id: u32, pane_index: usize, stretch_factor: f64) {
        while self.panes.len() <= pane_index {
            let mut p = Pane::new();
            p.stretch_factor = stretch_factor.max(0.01);
            self.panes.push(p);
        }
        if let Some(s) = self.series.iter_mut().find(|s| s.id == id as SeriesId) {
            s.pane_index = pane_index;
        }
    }

    /// Number of stacked panes.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// CSS Y of each pane boundary (top edge of panes 1..n), for separator hit-testing by the host.
    /// Reflects the last layout pass.
    pub fn pane_separator_ys(&self) -> Vec<f64> {
        self.panes.iter().skip(1).map(|p| p.top).collect()
    }

    /// Drag the separator below pane `i` by `delta_css` (positive grows pane `i`, shrinks `i+1`),
    /// keeping both at least a minimum height. Freezes current heights as stretch factors so the
    /// other panes hold their size, then re-lays out (roadmap Phase B1).
    pub fn drag_pane_separator(&mut self, i: usize, delta_css: f64) {
        if i + 1 >= self.panes.len() {
            return;
        }
        const MIN_PANE_H: f64 = 24.0;
        for p in &mut self.panes {
            p.stretch_factor = p.height.max(1.0);
        }
        let top = self.panes[i].height;
        let bot = self.panes[i + 1].height;
        let new_top = (top + delta_css).clamp(MIN_PANE_H, (top + bot - MIN_PANE_H).max(MIN_PANE_H));
        let actual = new_top - top;
        self.panes[i].stretch_factor = new_top;
        self.panes[i + 1].stretch_factor = bot - actual;
    }

    /// CSS height of pane `i` from the last layout pass.
    pub fn pane_height(&self, i: usize) -> f64 {
        self.panes.get(i).map(|p| p.height).unwrap_or(0.0)
    }

    /// Relative stretch factor of pane `i`.
    pub fn pane_stretch(&self, i: usize) -> f64 {
        self.panes.get(i).map(|p| p.stretch_factor).unwrap_or(1.0)
    }

    /// Set pane `i`'s stretch factor (its share of the content height relative to the others).
    pub fn set_pane_stretch(&mut self, i: usize, factor: f64) {
        if let Some(p) = self.panes.get_mut(i) {
            p.stretch_factor = factor.max(0.01);
            if self.css_width > 0.0 {
                self.recompute_layout();
            }
        }
    }

    /// Resize pane `i` to `height_css`, absorbing the delta from its neighbour below (or above for
    /// the last pane) — the same freeze-and-redistribute behavior as dragging its separator.
    pub fn set_pane_height(&mut self, i: usize, height_css: f64) {
        if i >= self.panes.len() {
            return;
        }
        let current = self.panes[i].height;
        let delta = height_css - current;
        if i + 1 < self.panes.len() {
            self.drag_pane_separator(i, delta);
        } else if i > 0 {
            // last pane: move the separator above it the other way to grow/shrink it
            self.drag_pane_separator(i - 1, -delta);
        }
        if self.css_width > 0.0 {
            self.recompute_layout();
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
        self.crosshair_mode = crosshair_mode_from_u8(mode);
        // keep the options store consistent so `options()` reflects it
        self.options.apply(&aion_core::options::patch(
            "crosshair",
            serde_json::json!({ "mode": mode }),
        ));
    }

    /// Deep-merge a JSON options patch and apply the runtime-affecting fields (crosshair mode).
    /// Colors (grid/crosshair/background) are read from the store during `render`. Call `render()`
    /// after to repaint (roadmap Phase A2).
    pub fn apply_options(&mut self, patch_json: &str) {
        if let Err(e) = self.options.apply_str(patch_json) {
            web_sys::console::warn_1(&format!("aion: apply_options ignored malformed patch — {e}").into());
            return;
        }
        // Re-derive runtime state that isn't read straight from the store each frame.
        self.crosshair_mode = crosshair_mode_from_u8(self.options.get().crosshair.mode);
    }

    /// Current options as a JSON string (round-trips the deep-merged state back to JS).
    pub fn options_json(&self) -> String {
        self.options.value().to_string()
    }

    /// Typed snapshot of the current options for the render path.
    fn opts(&self) -> ChartOptions {
        self.options.get()
    }

    pub fn resize(&mut self, css_width: f64, css_height: f64, dpr: f64) {
        self.css_width = css_width;
        self.css_height = css_height;
        self.dpr = dpr;
        let bitmap_w = (css_width * dpr).round().max(1.0) as u32;
        let bitmap_h = (css_height * dpr).round().max(1.0) as u32;
        self.bitmap_w = bitmap_w;
        self.bitmap_h = bitmap_h;
        if let Some(gfx) = self.gfx.as_mut() {
            gfx.config.width = bitmap_w;
            gfx.config.height = bitmap_h;
            gfx.surface.configure(&gfx.device, &gfx.config);
        }
        // Update geometry eagerly so fit_content/zoom/scroll called before the next render
        // (and the price_axis_width getter) see the new pane size, not a stale one.
        self.recompute_layout();
    }

    /// Negotiates the price-axis width against its labels and sets the time-scale width /
    /// price-scale height accordingly. Idempotent; called on resize, data change, and render.
    /// (The axis labels depend only on the price range, so one refinement pass converges.)
    fn recompute_layout(&mut self) {
        let content_h = (self.css_height - TIME_AXIS_HEIGHT).max(1.0);
        self.engine.layout_panes(content_h);

        let mut axis_w = self.compute_price_axis_width();
        for _ in 0..2 {
            let pane_w = (self.css_width - axis_w).max(1.0);
            self.time_scale.set_width(pane_w);
            self.engine.autoscale_visible();
            let new_w = self.compute_price_axis_width();
            if new_w == axis_w {
                break;
            }
            axis_w = new_w;
        }
        self.pane_w = (self.css_width - axis_w).max(1.0);
        self.pane_h = content_h;
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
        if self.price_scale().price_range().is_none() {
            return None;
        }
        Some(self.price_scale().price_to_coordinate(price, price))
    }

    /// Price for a Y (CSS px), or `None` if the scale has no range yet.
    pub fn coordinate_to_price(&self, y_css: f64) -> Option<f64> {
        if self.price_scale().price_range().is_none() {
            return None;
        }
        Some(self.price_scale().coordinate_to_price(y_css, 0.0))
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

    /// Float logical (bar) index under an X coordinate, or `None` when there is no data. May be
    /// negative or beyond the last bar (positions off the ends), matching LWC's `Logical`.
    pub fn coordinate_to_logical(&self, x_css: f64) -> Option<f64> {
        if self.data.merged_times().is_empty() {
            return None;
        }
        Some(self.time_scale.coordinate_to_float_index(x_css))
    }

    /// Per-series values at the bar under an X coordinate, flattened as groups of five:
    /// `[series_id, open, high, low, close, ...]`. Only series that actually have a point at that
    /// bar are included (single-value series report the value in all four slots). Empty when the
    /// cursor is off the data. Backs the façade's `seriesData` map for crosshair/click events.
    pub fn hover_data(&self, x_css: f64) -> Vec<f64> {
        use aion_core::model::plot_list::MismatchDirection;
        let n = self.data.merged_times().len() as i64;
        if n == 0 {
            return Vec::new();
        }
        let index = self.time_scale.coordinate_to_index(x_css);
        if index < 0 || index >= n {
            return Vec::new();
        }
        let mut out = Vec::new();
        for s in &self.series {
            let plot = self.data.plot(s.id);
            if let Some(row) = plot.search(index, MismatchDirection::None) {
                out.push(s.id as f64);
                out.push(plot.value_at(row, PlotValueIndex::Open));
                out.push(plot.value_at(row, PlotValueIndex::High));
                out.push(plot.value_at(row, PlotValueIndex::Low));
                out.push(plot.value_at(row, PlotValueIndex::Close));
            }
        }
        out
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
        // ---- layout (price axis width negotiated against the price labels) ----
        self.recompute_layout();

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

        // ---- GPU: one scissored draw group per stacked pane ----
        let visible = self.visible_data_range();
        // The headless engine owns chart geometry. The WASM host only adds browser-adapter
        // concerns such as crosshair interaction and text labels.
        let engine_frame = self.engine.build_frame();
        let groups = if self.gfx.is_some() {
            let mut groups: Vec<DrawGroup> = Vec::with_capacity(engine_frame.panes.len());
            for pane_frame in &engine_frame.panes {
                let mut group = DrawGroup {
                    scissor: Some(pane_frame.scissor),
                    ..Default::default()
                };
                let prims = pane_frame.main.clone();
                let grid_prims = pane_frame.under.clone();
                let points = pane_frame.points.clone();

                // Convert the shared frame only at the WebGPU backend boundary.
                geom_prims_to_tris(&prims, &points, &mut group.fill_tris, &mut group.stroke_tris);
                prims_to_instances(&grid_prims, &mut group.under_quads);
                prims_to_instances(&prims, &mut group.quads);
                groups.push(group);
            }
            groups
        } else {
            Vec::new()
        };

        let bg = Color::parse_css(&self.opts().layout.background.color)
            .unwrap_or(Color::rgb(0xff, 0xff, 0xff));
        if let Some(gfx) = self.gfx.as_mut() {
            gfx.msaa.ensure(&gfx.device, gfx.config.format, gfx.config.width, gfx.config.height);
            let frame = gfx
                .surface
                .get_current_texture()
                .map_err(|e| JsValue::from_str(&format!("get_current_texture failed: {e}")))?;
            let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

            // Background clear color from layout.background (roadmap Phase A2).
            let bg_clear = wgpu::Color {
                r: bg.r() as f64 / 255.0,
                g: bg.g() as f64 / 255.0,
                b: bg.b() as f64 / 255.0,
                a: 1.0,
            };
            render_frame(
                &gfx.device,
                &gfx.queue,
                gfx.msaa.view(),
                &view,
                gfx.config.width,
                gfx.config.height,
                bg_clear,
                &gfx.quad_renderer,
                &gfx.tex_renderer,
                &gfx.tri_renderer,
                &groups,
            );
            frame.present();
        } else {
            self.render_canvas2d(&engine_frame)?;
        }

        self.draw_axes_2d(visible, &time_marks)?;
        Ok(())
    }

    // --- data / scale bookkeeping ---

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

    /// Autoscale every pane's price + overlay scale over just its own series' visible min/max.
    /// The main (right-axis) price scale — pane 0's. Used by the price axis, crosshair, last-value
    /// line, and the public coordinate API.
    fn price_scale(&self) -> &PriceScaleCore {
        &self.panes[0].price_scale
    }

    fn measure(&self, text: &str) -> f64 {
        self.axis_ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * self.dpr));
        let device_w = self.axis_ctx.measure_text(text).map(|m| m.width()).unwrap_or(0.0);
        device_w / self.dpr
    }

    fn compute_price_axis_width(&self) -> f64 {
        let mut max_text_w = 0f64;
        // widest tick label across all panes' price scales (volume numbers can exceed price ones)
        for pane in &self.panes {
            for mark in pane.price_scale.build_tick_marks(100, 0.0) {
                max_text_w = max_text_w.max(self.measure(&self.price_formatter.format(mark.logical)));
            }
        }
        if let Some((_, y)) = self.crosshair {
            if !self.price_scale().is_empty() {
                let price = self.price_scale().coordinate_to_price(y, 0.0);
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

    /// Index of the pane whose vertical band contains css-y `y`, if any.
    fn pane_at_y(&self, y: f64) -> Option<usize> {
        self.panes.iter().position(|p| y >= p.top && y <= p.top + p.height)
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
            return (self.price_scale().coordinate_to_price(y_css, 0.0), y_css);
        };

        let price = match self.crosshair_mode {
            CrosshairMode::Normal | CrosshairMode::Hidden => {
                return (self.price_scale().coordinate_to_price(y_css, close), y_css)
            }
            CrosshairMode::Magnet => close,
            CrosshairMode::MagnetOhlc => {
                let row = row.expect("close present");
                let open = plot.value_at(row, PlotValueIndex::Open);
                let high = plot.value_at(row, PlotValueIndex::High);
                let low = plot.value_at(row, PlotValueIndex::Low);
                let candidates = [
                    (open, self.price_scale().price_to_coordinate(open, open)),
                    (high, self.price_scale().price_to_coordinate(high, high)),
                    (low, self.price_scale().price_to_coordinate(low, low)),
                    (close, self.price_scale().price_to_coordinate(close, close)),
                ];
                magnet_snap(y_css, &candidates).unwrap_or(close)
            }
        };
        (price, self.price_scale().price_to_coordinate(price, price))
    }

    // ---- Canvas2D axis overlay ----

    fn draw_axes_2d(&self, visible: Option<(i64, i64)>, time_marks: &[(i64, u8)]) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        let dpr = self.dpr;
        let bitmap_w = self.bitmap_w as f64;
        let bitmap_h = self.bitmap_h as f64;
        let pane_w = self.pane_w;
        let pane_h = self.pane_h;

        ctx.clear_rect(0.0, 0.0, bitmap_w, bitmap_h);
        let font = format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr);
        let border_w = 1f64.max(dpr.floor());

        ctx.set_fill_style_str(BORDER_CSS);
        ctx.fill_rect((pane_w * dpr).round(), 0.0, border_w, (pane_h * dpr).round());
        ctx.fill_rect(0.0, (pane_h * dpr).round(), bitmap_w, border_w);

        // separators between stacked panes (roadmap Phase B1): a border line at each pane boundary
        for pane in self.panes.iter().skip(1) {
            let y = ((pane.top - PANE_SEPARATOR) * dpr).round();
            ctx.fill_rect(0.0, y, (pane_w * dpr).round(), (PANE_SEPARATOR * dpr).max(border_w));
        }

        // price tick labels for every pane, each clipped to its own band (roadmap Phase B1). Scale
        // coords are canvas-absolute, so a label just draws at its coord if it falls in the band.
        ctx.set_font(&font);
        ctx.set_text_baseline("middle");
        ctx.set_text_align("left");
        ctx.set_fill_style_str(TEXT_CSS);
        let text_x = (pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER) * dpr;
        for pane in &self.panes {
            let band_top = pane.top * dpr;
            let band_bot = (pane.top + pane.height) * dpr;
            for mark in pane.price_scale.build_tick_marks(100, 0.0) {
                let y = (mark.coord * dpr).round();
                if y < band_top - 0.5 || y > band_bot + 0.5 {
                    continue;
                }
                ctx.fill_text(&self.price_formatter.format(mark.logical), text_x, y)?;
            }
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

        self.draw_price_line_labels_2d(pane_w, dpr)?;
        self.draw_marker_labels_2d(visible, pane_w, dpr)?;
        self.draw_last_value_label_2d(pane_w, pane_h, dpr)?;
        self.draw_crosshair_labels_2d(pane_w, pane_h, dpr, &font)?;
        Ok(())
    }

    /// Text labels for series markers, drawn on the 2D overlay just outside each marker shape
    /// (above above-markers, below below-markers). Positions mirror `build_markers`. Roadmap B4.
    fn draw_marker_labels_2d(&self, visible: Option<(i64, i64)>, pane_w: f64, dpr: f64) -> Result<(), JsValue> {
        let Some((from, to)) = visible else { return Ok(()) };
        const MARKER_SIZE: f64 = 6.0;
        const MARKER_GAP: f64 = 4.0;
        let ctx = &self.axis_ctx;
        ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr));
        ctx.set_text_baseline("middle");
        ctx.set_text_align("center");
        let times = self.data.merged_times();
        for (pi, pane) in self.panes.iter().enumerate() {
            let (band_top, band_bot) = (pane.top, pane.top + pane.height);
            for s in &self.series {
                if s.pane_index.min(self.panes.len() - 1) != pi {
                    continue;
                }
                let scale = if s.overlay { &pane.overlay_scale } else { &pane.price_scale };
                if scale.is_empty() {
                    continue;
                }
                let plot = self.data.plot(s.id);
                for m in &s.markers {
                    if m.text.is_empty() {
                        continue;
                    }
                    let Ok(pos) = times.binary_search(&m.time) else { continue };
                    let idx = pos as i64;
                    if idx < from || idx > to {
                        continue;
                    }
                    let Some(row) = plot.search(idx, aion_core::model::plot_list::MismatchDirection::None) else { continue };
                    let high = plot.value_at(row, PlotValueIndex::High);
                    let low = plot.value_at(row, PlotValueIndex::Low);
                    let x = self.time_scale.index_to_coordinate(idx);
                    if x < 0.0 || x > pane_w {
                        continue;
                    }
                    // shape center y (css), then place the label clear of the shape
                    let text_y = match m.position {
                        marker_pos::BELOW => {
                            scale.price_to_coordinate(low, low) + 2.0 * MARKER_SIZE + MARKER_GAP + FONT_SIZE / 2.0 + 2.0
                        }
                        marker_pos::ABOVE => {
                            scale.price_to_coordinate(high, high) - 2.0 * MARKER_SIZE - MARKER_GAP - FONT_SIZE / 2.0 - 2.0
                        }
                        _ => scale.price_to_coordinate((high + low) / 2.0, high) - 2.0 * MARKER_SIZE - FONT_SIZE / 2.0 - 2.0,
                    };
                    if text_y < band_top || text_y > band_bot {
                        continue;
                    }
                    ctx.set_fill_style_str(&m.color.to_hex());
                    ctx.fill_text(&m.text, (x * dpr).round(), (text_y * dpr).round())?;
                }
            }
        }
        Ok(())
    }

    /// Colored axis labels for every series' price lines (roadmap Phase B4). Uses the price value
    /// (or the line's title, when set) on a filled box in the line's color, like the last-value tag.
    fn draw_price_line_labels_2d(&self, pane_w: f64, dpr: f64) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr));
        ctx.set_text_baseline("middle");
        for (pi, pane) in self.panes.iter().enumerate() {
            let band_top = pane.top * dpr;
            let band_bot = (pane.top + pane.height) * dpr;
            for s in &self.series {
                if s.pane_index.min(self.panes.len() - 1) != pi {
                    continue;
                }
                let scale = if s.overlay { &pane.overlay_scale } else { &pane.price_scale };
                if scale.is_empty() {
                    continue;
                }
                for pl in &s.price_lines {
                    let y = scale.price_to_coordinate(pl.price, pl.price) * dpr;
                    if y < band_top || y > band_bot {
                        continue;
                    }
                    let label = if pl.title.is_empty() { self.price_formatter.format(pl.price) } else { pl.title.clone() };
                    let text_w = self.measure(&label);
                    let box_h = ((FONT_SIZE + PRICE_LABEL_PADDING_TB * 2.0) * dpr).round();
                    let box_w = ((AXIS_BORDER_SIZE + PRICE_PADDING_INNER + PRICE_PADDING_OUTER + AXIS_TICK_LENGTH + text_w) * dpr).round();
                    let box_x = (pane_w * dpr).round();
                    let box_y = (y.round() - box_h / 2.0).round();
                    ctx.set_fill_style_str(&pl.color.to_hex());
                    ctx.fill_rect(box_x, box_y, box_w, box_h);
                    ctx.set_text_align("left");
                    ctx.set_fill_style_str(&pl.color.contrast_text().to_hex());
                    let text_x = (pane_w + AXIS_TICK_LENGTH + PRICE_PADDING_INNER) * dpr;
                    ctx.fill_text(&label, text_x, y.round())?;
                }
            }
        }
        Ok(())
    }

    fn draw_last_value_label_2d(&self, pane_w: f64, pane_h: f64, dpr: f64) -> Result<(), JsValue> {
        if self.main_plot().is_empty() || self.price_scale().is_empty() {
            return Ok(());
        }
        let last = self.main_plot().size() - 1;
        let close = self.main_plot().value_at(last, PlotValueIndex::Close);
        let y = self.price_scale().price_to_coordinate(close, close);
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

        // price label for the pane under the cursor, using that pane's scale (roadmap Phase B1).
        // The price pane (0) magnet-snaps to its series; other panes read the raw cursor y.
        if let Some(pi) = self.pane_at_y(y_css).filter(|_| y_css <= pane_h) {
            let scale = &self.panes[pi].price_scale;
            if !scale.is_empty() {
            let (price, snap_y) = if pi == 0 {
                self.crosshair_snap(x_css, y_css)
            } else {
                (scale.coordinate_to_price(y_css, 0.0), y_css)
            };
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
        }

        if x_css <= pane_w {
            let index = self.snapped_crosshair_index(x_css);
            let ts = self.data.merged_times()[index as usize];
            let label = format_crosshair_time(ts, self.time_visible, false);
            let text_w = self.measure(&label);
            let box_w = ((text_w + TIME_PADDING_HORZ * 2.0) * dpr).round();
            let box_h = ((FONT_SIZE + TIME_PADDING_TOP + TIME_PADDING_BOTTOM) * dpr).round();
            let snapped_x = self.snapped_crosshair_x(x_css);
            let bitmap_w = self.bitmap_w as f64;
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

    /// Execute the exact same frame consumed by WebGPU through the browser's 2D canvas backend.
    fn render_canvas2d(&self, frame: &aion_engine::ChartFrame) -> Result<(), JsValue> {
        let Some(ctx) = self.pane_ctx.as_ref() else {
            return Err(JsValue::from_str("Canvas2D pane backend is not initialized"));
        };
        let width = self.bitmap_w as f64;
        let height = self.bitmap_h as f64;
        ctx.clear_rect(0.0, 0.0, width, height);
        let bg = self.opts().layout.background.color;
        ctx.set_fill_style_str(&bg);
        ctx.fill_rect(0.0, 0.0, width, height);
        let mut target = crate::canvas2d_target::WasmCanvas2d::new(ctx);
        let viewport = CanvasViewport { width: width as f32, height: height as f32 };
        for pane in &frame.panes {
            target.save();
            let [x, y, w, h] = pane.scissor;
            target.clip_rect(x as f32, y as f32, w as f32, h as f32);
            execute_canvas2d(&pane.under, &pane.points, &mut target, viewport);
            execute_canvas2d(&pane.main, &pane.points, &mut target, viewport);
            target.restore();
        }
        Ok(())
    }
}

/// Attempt to initialize WebGPU. A failure is recoverable because the same chart frame can be
/// executed by the Canvas2D backend.
async fn try_create_gfx(
    pane_canvas: web_sys::HtmlCanvasElement,
    css_width: f64,
    css_height: f64,
    dpr: f64,
) -> Result<Gfx, JsValue> {
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
    Ok(Gfx {
        device,
        queue,
        surface,
        config,
        quad_renderer,
        tri_renderer,
        msaa,
        _atlas: atlas,
        tex_renderer,
    })
}
