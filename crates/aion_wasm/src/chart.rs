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

mod inner_api;
mod inner_render;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use js_sys::Float64Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::CanvasRenderingContext2d;

use crate::axis_policy::negotiated_axis_width;
use crate::backend_policy::{surface_error_action, SurfaceErrorAction};
use aion_core::model::data_layer::SeriesId;
use aion_core::model::data_validation::sanitize_ohlc;
use aion_core::model::magnet::CrosshairMode;
use aion_core::model::plot_list::{MismatchDirection, PlotValueIndex};
use aion_core::options::{crosshair_mode, ChartOptions};
use aion_core::scale::price_scale_core::PriceScaleMode;
use aion_engine::{
    line_style_from_u8, marker_pos, marker_shape, AxisFrame, AxisTextAlign, AxisTextMidpoint,
    ChartEngine, Marker, Pane, PriceLine, PriceScaleTarget, SeriesKind, TIME_AXIS_HEIGHT,
};
use aion_render::canvas2d::{execute as execute_canvas2d, Canvas2d, Viewport as CanvasViewport};
use aion_render::color::Color;
use aion_render::draw_list::LineType;
use aion_render_wgpu::{
    geom_prims_to_tris, prims_to_instances, render_frame, DrawGroup, LabelAtlas, MsaaTarget,
    QuadRenderer, TexQuadRenderer, TriRenderer, SAMPLE_COUNT,
};

#[wasm_bindgen(inline_js = r#"
export function notify_aion_backend_loss(runtimeId) {
    window.dispatchEvent(new CustomEvent('aion-chart-backend-lost', { detail: runtimeId }));
}
"#)]
extern "C" {
    fn notify_aion_backend_loss(runtime_id: u32);
}

static NEXT_RUNTIME_ID: AtomicU32 = AtomicU32::new(1);

// lightweight-charts default palette (RENDERING_SPEC.md §2.5, §7, §8, §15)
// Axis palette (as CSS color strings for the 2D overlay)
const BORDER_CSS: &str = "#2B2B43";
// TradingView-style volume: translucent green on up bars, red on down bars.

// Crosshair marker (line/area) — line-series.ts defaults.

/// LWC default font stack (`helpers/make-font.ts` / layout defaults).
const FONT_FAMILY: &str =
    "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif";

/// Axis metrics (RENDERING_SPEC.md §10, §11), font size 12.
const FONT_SIZE: f64 = 12.0;
const TICK_MARK_MAX_CHARS: f64 = 8.0;

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
        crosshair_mode::MAGNET => CrosshairMode::Magnet,
        crosshair_mode::HIDDEN => CrosshairMode::Hidden,
        crosshair_mode::MAGNET_OHLC => CrosshairMode::MagnetOhlc,
        // Unknown wire values fall back to the default mode (Normal).
        _ => CrosshairMode::Normal,
    }
}

fn price_scale_mode_from_u8(mode: u8) -> PriceScaleMode {
    match mode {
        1 => PriceScaleMode::Logarithmic,
        2 => PriceScaleMode::Percentage,
        3 => PriceScaleMode::IndexedTo100,
        _ => PriceScaleMode::Normal,
    }
}

fn price_scale_mode_to_u8(mode: PriceScaleMode) -> u8 {
    match mode {
        PriceScaleMode::Normal => 0,
        PriceScaleMode::Logarithmic => 1,
        PriceScaleMode::Percentage => 2,
        PriceScaleMode::IndexedTo100 => 3,
    }
}

fn price_scale_target_from_u8(target: u8) -> PriceScaleTarget {
    match target {
        1 => PriceScaleTarget::Left,
        2 => PriceScaleTarget::Overlay,
        _ => PriceScaleTarget::Right,
    }
}

fn price_scale_target_to_u8(target: PriceScaleTarget) -> u8 {
    match target {
        PriceScaleTarget::Right => 0,
        PriceScaleTarget::Left => 1,
        PriceScaleTarget::Overlay => 2,
    }
}

fn mismatch_direction_from_i8(direction: i8) -> MismatchDirection {
    match direction {
        -1 => MismatchDirection::NearestLeft,
        1 => MismatchDirection::NearestRight,
        _ => MismatchDirection::None,
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
    device_lost: Arc<AtomicBool>,
}

enum PaneRenderOutcome {
    Presented,
    Timeout,
    Fallback(String),
    Canvas2d,
}

struct ChartInner {
    gfx: Option<Gfx>,
    gpu_pane: web_sys::HtmlCanvasElement,
    fallback_pane: web_sys::HtmlCanvasElement,
    pane_ctx: CanvasRenderingContext2d,
    axis_ctx: CanvasRenderingContext2d,
    bitmap_w: u32,
    bitmap_h: u32,
    engine: ChartEngine,
    frame: aion_engine::ChartFrame,
    axis_frame: AxisFrame,
    gpu_groups: Vec<DrawGroup>,
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
    runtime_id: u32,
    gpu_pane: web_sys::HtmlCanvasElement,
    fallback_pane: web_sys::HtmlCanvasElement,
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

fn set_backend_visibility(
    gpu_pane: &web_sys::HtmlCanvasElement,
    fallback_pane: &web_sys::HtmlCanvasElement,
    use_webgpu: bool,
) {
    let _ = gpu_pane
        .style()
        .set_property("visibility", if use_webgpu { "visible" } else { "hidden" });
    let _ = fallback_pane
        .style()
        .set_property("visibility", if use_webgpu { "hidden" } else { "visible" });
}

/// Sizes all three canvases to `(bw, bh)` device pixels while pinning their CSS box to the real
/// displayed size, then resizes + repaints the engine. Shared by the initial bind and every
/// observer callback.
#[allow(clippy::too_many_arguments)] // three canvases + the full size/DPR tuple
fn apply_device_size(
    inner: &Rc<RefCell<ChartInner>>,
    gpu_pane: &web_sys::HtmlCanvasElement,
    fallback_pane: &web_sys::HtmlCanvasElement,
    overlay: &web_sys::HtmlCanvasElement,
    css_w: f64,
    css_h: f64,
    bw: f64,
    bh: f64,
) {
    let (bw_u, bh_u) = (bw.max(1.0) as u32, bh.max(1.0) as u32);
    for c in [gpu_pane, fallback_pane, overlay] {
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

/// Creates a chart bound to dedicated WebGPU and Canvas2D pane canvases plus an axis/text overlay.
/// All three must be full chart size with bitmap size = css size * dpr, already set by the caller.
/// Call [`AionChart::enable_auto_resize`] to have the engine own sizing from then on.
#[allow(clippy::too_many_arguments)] // public JS entry point: three canvases + size/DPR/backend
#[wasm_bindgen]
pub async fn create_chart(
    gpu_pane_canvas: web_sys::HtmlCanvasElement,
    fallback_pane_canvas: web_sys::HtmlCanvasElement,
    overlay_canvas: web_sys::HtmlCanvasElement,
    css_width: f64,
    css_height: f64,
    dpr: f64,
    force_canvas2d: bool,
    simulate_adapter_failure: bool,
    force_fallback_adapter: bool,
) -> Result<AionChart, JsValue> {
    console_error_panic_hook::set_once();

    // Keep handles to all canvas elements so the engine can own device-pixel resizing
    // (create_surface takes the pane canvas by value; the clone is just a JS reference).
    let gpu_pane_el = gpu_pane_canvas.clone();
    let fallback_pane_el = fallback_pane_canvas.clone();
    let overlay_el = overlay_canvas.clone();

    let axis_ctx = overlay_canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    let bitmap_w = (css_width * dpr).round().max(1.0) as u32;
    let bitmap_h = (css_height * dpr).round().max(1.0) as u32;
    let runtime_id = NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
    // A canvas cannot change context type after WebGPU has claimed it. Keep a dedicated 2D pane
    // warm from construction so a device loss can switch backends without replacing DOM nodes or
    // rebuilding chart state.
    let pane_ctx = fallback_pane_el
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d pane context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;
    let gfx = if force_canvas2d {
        None
    } else {
        match try_create_gfx(
            gpu_pane_canvas,
            css_width,
            css_height,
            dpr,
            runtime_id,
            simulate_adapter_failure,
            force_fallback_adapter,
        )
        .await
        {
            Ok(gfx) => Some(gfx),
            Err(error) => {
                web_sys::console::warn_1(
                    &format!("aion: WebGPU unavailable; using Canvas2D fallback ({error:?})")
                        .into(),
                );
                None
            }
        }
    };
    set_backend_visibility(&gpu_pane_el, &fallback_pane_el, gfx.is_some());

    let inner = ChartInner {
        gfx,
        gpu_pane: gpu_pane_el.clone(),
        fallback_pane: fallback_pane_el.clone(),
        pane_ctx,
        axis_ctx,
        bitmap_w,
        bitmap_h,
        engine: ChartEngine::new(css_width, css_height, dpr),
        frame: aion_engine::ChartFrame::default(),
        axis_frame: AxisFrame::default(),
        gpu_groups: Vec::new(),
    };

    Ok(AionChart {
        inner: Rc::new(RefCell::new(inner)),
        runtime_id,
        gpu_pane: gpu_pane_el,
        fallback_pane: fallback_pane_el,
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
        let gpu_pane = self.gpu_pane.clone();
        let fallback_pane = self.fallback_pane.clone();
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
                let dpr = web_sys::window()
                    .map(|w| w.device_pixel_ratio())
                    .unwrap_or(1.0);
                ((css_w * dpr).round(), (css_h * dpr).round())
            });
            apply_device_size(
                &inner,
                &gpu_pane,
                &fallback_pane,
                &overlay,
                css_w,
                css_h,
                bw,
                bh,
            );
        }) as Box<dyn FnMut(js_sys::Array)>);

        let observer = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref())?;
        // Observe the device-pixel-content-box so the callback also fires on DPR changes.
        let opts = web_sys::ResizeObserverOptions::new();
        opts.set_box(web_sys::ResizeObserverBoxOptions::DevicePixelContentBox);
        observer.observe_with_options(&container, &opts);

        // Size once now so the first paint is correct even before the observer first fires.
        let rect = container.get_bounding_client_rect();
        let (css_w, css_h) = (rect.width().max(1.0), rect.height().max(1.0));
        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);
        apply_device_size(
            &self.inner,
            &self.gpu_pane,
            &self.fallback_pane,
            &self.overlay,
            css_w,
            css_h,
            (css_w * dpr).round(),
            (css_h * dpr).round(),
        );

        self._resize = Some(ResizeBinding {
            observer,
            _callback: callback,
        });
        Ok(())
    }

    /// Adds a series and returns its id. `kind`: 0 candles, 1 bars, 2 line, 3 area, 4 histogram.
    pub fn add_series(&mut self, kind: u8) -> u32 {
        self.inner.borrow_mut().add_series(kind)
    }

    /// Add a Rust-native simple moving-average line derived from `source_id`.
    pub fn add_sma(&mut self, source_id: u32, period: u32) -> u32 {
        self.inner.borrow_mut().add_sma(source_id, period)
    }

    /// Add a Rust-native exponential moving-average line derived from `source_id`.
    pub fn add_ema(&mut self, source_id: u32, period: u32) -> u32 {
        self.inner.borrow_mut().add_ema(source_id, period)
    }

    /// Add upper, middle, and lower Bollinger-band lines. Returns an empty array for invalid input.
    pub fn add_bollinger(&mut self, source_id: u32, period: u32, deviation: f64) -> Vec<u32> {
        self.inner
            .borrow_mut()
            .add_bollinger(source_id, period, deviation)
    }

    /// Sets the main series' data (series 0). `times` are ascending UTC seconds.
    pub fn set_data(
        &mut self,
        times: &[f64],
        open: &[f64],
        high: &[f64],
        low: &[f64],
        close: &[f64],
    ) {
        self.inner
            .borrow_mut()
            .set_data(times, open, high, low, close);
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
        self.inner
            .borrow_mut()
            .set_series_data(id, times, open, high, low, close);
    }

    /// Typed-array ingestion path: wasm-bindgen passes the JS views as externrefs and the engine
    /// takes one owned copy, avoiding the temporary slice copy generated for `&[f64]` methods.
    pub fn set_series_data_typed(
        &mut self,
        id: u32,
        times: &Float64Array,
        open: &Float64Array,
        high: &Float64Array,
        low: &Float64Array,
        close: &Float64Array,
    ) {
        self.inner
            .borrow_mut()
            .set_series_data_typed(id, times, open, high, low, close);
    }

    /// Streaming update of the main series (append new time or replace last).
    pub fn update_bar(&mut self, time: f64, open: f64, high: f64, low: f64, close: f64) {
        self.inner
            .borrow_mut()
            .update_bar(time, open, high, low, close);
    }

    /// Streaming update of an arbitrary series by id (append new time or replace last).
    pub fn update_series_bar(
        &mut self,
        series_id: u32,
        time: f64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) {
        self.inner
            .borrow_mut()
            .update_series_bar(series_id, time, open, high, low, close);
    }

    /// Sets a series' line/area color (overrides the kind default).
    pub fn set_series_color(&mut self, id: u32, r: u8, g: u8, b: u8) {
        self.inner.borrow_mut().set_series_color(id, r, g, b);
    }

    /// Toggle a series while preserving its data and derived-indicator binding.
    pub fn set_series_visible(&mut self, id: u32, visible: bool) {
        self.inner.borrow_mut().set_series_visible(id, visible);
    }

    /// Set candlestick/bar up & down body colors as CSS strings (empty string = keep default).
    pub fn set_series_updown_colors(&mut self, id: u32, up: &str, down: &str) {
        self.inner
            .borrow_mut()
            .set_series_updown_colors(id, up, down);
    }

    /// Set a line/area series' stroke width (css px).
    pub fn set_series_line_width(&mut self, id: u32, width: f64) {
        self.inner.borrow_mut().set_series_line_width(id, width);
    }

    /// Set an area series' fill gradient colors (top at the line, bottom at the base; CSS strings).
    pub fn set_series_area_colors(&mut self, id: u32, top: &str, bottom: &str) {
        self.inner
            .borrow_mut()
            .set_series_area_colors(id, top, bottom);
    }

    /// Color a histogram (volume) by the main price series' up/down direction per bar
    /// (TradingView-style volume).
    pub fn set_series_histogram_updown(&mut self, id: u32, enabled: bool) {
        self.inner
            .borrow_mut()
            .set_series_histogram_updown(id, enabled);
    }

    /// Set a line/area series' join type: 0 = simple, 1 = stepped, 2 = curved. Call `render()`
    /// after (roadmap Phase B3).
    pub fn set_series_line_type(&mut self, id: u32, line_type: u8) {
        self.inner.borrow_mut().set_series_line_type(id, line_type);
    }

    /// Toggle per-point disc markers on a line/area series. Call `render()` after (Phase B3).
    pub fn set_series_point_markers(&mut self, id: u32, visible: bool) {
        self.inner
            .borrow_mut()
            .set_series_point_markers(id, visible);
    }

    /// Set a Baseline series' baseline price (`NaN` = auto). Call `render()` after (Phase B3).
    pub fn set_series_baseline(&mut self, id: u32, price: f64) {
        self.inner.borrow_mut().set_series_baseline(id, price);
    }

    /// Toggle the pulsing last-price ring on a series (roadmap Phase B3).
    pub fn set_series_last_price_animation(&mut self, id: u32, enabled: bool) {
        self.inner
            .borrow_mut()
            .set_series_last_price_animation(id, enabled);
    }

    /// Add a horizontal price line to a series; returns its id. `style`: 0 solid, 1 dotted, 2
    /// dashed, 3 large-dashed, 4 sparse-dotted. Call `render()` after (roadmap Phase B4).
    #[allow(clippy::too_many_arguments)]
    pub fn create_price_line(
        &mut self,
        series_id: u32,
        price: f64,
        r: u8,
        g: u8,
        b: u8,
        width: u32,
        style: u8,
        title: &str,
    ) -> u32 {
        self.inner
            .borrow_mut()
            .create_price_line(series_id, price, r, g, b, width, style, title)
    }
    /// Remove a price line by id. Call `render()` after (roadmap Phase B4).
    pub fn remove_price_line(&mut self, id: u32) {
        self.inner.borrow_mut().remove_price_line(id);
    }

    /// Replace a series' markers from a JSON array. Call `render()` after (roadmap Phase B4).
    pub fn set_series_markers(&mut self, series_id: u32, json: &str) {
        self.inner.borrow_mut().set_series_markers(series_id, json);
    }
    /// Toggle marker pixel margins in price-scale autoscaling (enabled by default, as in LWC).
    pub fn set_series_markers_auto_scale(&mut self, series_id: u32, enabled: bool) {
        self.inner
            .borrow_mut()
            .set_series_markers_auto_scale(series_id, enabled);
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
        self.inner
            .borrow_mut()
            .set_series_pane(id, pane_index, stretch_factor);
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
    pub fn set_bar_spacing(&mut self, spacing: f64) {
        self.inner.borrow_mut().set_bar_spacing(spacing);
    }
    pub fn set_right_offset(&mut self, offset: f64) {
        self.inner.borrow_mut().set_right_offset(offset);
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
    pub fn right_offset(&self) -> f64 {
        self.inner.borrow().right_offset()
    }
    pub fn scroll_position(&self) -> f64 {
        self.inner.borrow().scroll_position()
    }
    pub fn scroll_to_position(&mut self, position: f64) {
        self.inner.borrow_mut().scroll_to_position(position);
    }
    pub fn scroll_to_real_time(&mut self) {
        self.inner.borrow_mut().scroll_to_real_time();
    }
    pub fn reset_time_scale(&mut self) {
        self.inner.borrow_mut().reset_time_scale();
    }
    pub fn time_scale_width(&self) -> f64 {
        self.inner.borrow().time_scale_width()
    }
    pub fn time_scale_height(&self) -> f64 {
        self.inner.borrow().time_scale_height()
    }
    pub fn price_scale_width(&self, pane: usize, target: u8) -> f64 {
        self.inner.borrow().price_scale_width(pane, target)
    }
    pub fn price_scale_visible_range(&self, pane: usize, target: u8) -> Vec<f64> {
        self.inner.borrow().price_scale_visible_range(pane, target)
    }
    pub fn set_price_scale_visible_range(&mut self, pane: usize, target: u8, from: f64, to: f64) {
        self.inner
            .borrow_mut()
            .set_price_scale_visible_range(pane, target, from, to);
    }
    pub fn price_scale_auto_scale(&self, pane: usize, target: u8) -> Option<bool> {
        self.inner.borrow().price_scale_auto_scale(pane, target)
    }
    pub fn set_price_scale_auto_scale(&mut self, pane: usize, target: u8, enabled: bool) {
        self.inner
            .borrow_mut()
            .set_price_scale_auto_scale(pane, target, enabled);
    }
    pub fn price_scale_inverted(&self, pane: usize, target: u8) -> Option<bool> {
        self.inner.borrow().price_scale_inverted(pane, target)
    }
    pub fn set_price_scale_inverted(&mut self, pane: usize, target: u8, inverted: bool) {
        self.inner
            .borrow_mut()
            .set_price_scale_inverted(pane, target, inverted);
    }
    pub fn price_scale_margins(&self, pane: usize, target: u8) -> Vec<f64> {
        self.inner.borrow().price_scale_margins(pane, target)
    }
    pub fn set_price_scale_margins(&mut self, pane: usize, target: u8, top: f64, bottom: f64) {
        self.inner
            .borrow_mut()
            .set_price_scale_margins(pane, target, top, bottom);
    }
    pub fn price_scale_mode(&self, pane: usize, target: u8) -> Option<u8> {
        self.inner.borrow().price_scale_mode(pane, target)
    }
    pub fn set_price_scale_mode(&mut self, pane: usize, target: u8, mode: u8) {
        self.inner
            .borrow_mut()
            .set_price_scale_mode(pane, target, mode);
    }
    pub fn series_pane_index(&self, id: u32) -> Option<usize> {
        self.inner.borrow().series_pane_index(id)
    }
    pub fn series_is_overlay(&self, id: u32) -> Option<bool> {
        self.inner.borrow().series_is_overlay(id)
    }
    pub fn series_price_scale_id(&self, id: u32) -> Option<u8> {
        self.inner.borrow().series_price_scale_id(id)
    }
    pub fn set_series_price_scale(&mut self, id: u32, target: u8) {
        self.inner.borrow_mut().set_series_price_scale(id, target);
    }
    pub fn series_price_to_coordinate(&self, id: u32, price: f64) -> Option<f64> {
        self.inner.borrow().series_price_to_coordinate(id, price)
    }
    pub fn series_coordinate_to_price(&self, id: u32, coordinate: f64) -> Option<f64> {
        self.inner
            .borrow()
            .series_coordinate_to_price(id, coordinate)
    }
    pub fn series_kind(&self, id: u32) -> Option<u8> {
        self.inner.borrow().series_kind(id)
    }
    pub fn series_data_by_index(&self, id: u32, index: f64, mismatch: i8) -> Vec<f64> {
        self.inner
            .borrow()
            .series_data_by_index(id, index, mismatch)
    }
    pub fn series_data(&self, id: u32) -> Vec<f64> {
        self.inner.borrow().series_data(id)
    }
    pub fn series_bars_in_logical_range(&self, id: u32, from: f64, to: f64) -> Vec<f64> {
        self.inner
            .borrow()
            .series_bars_in_logical_range(id, from, to)
    }
    pub fn price_axis_width(&self) -> f64 {
        self.inner.borrow().price_axis_width()
    }
    pub fn pane_left(&self) -> f64 {
        self.inner.borrow().pane_left()
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
    /// Integer logical (bar) index owning X (CSS px), or `undefined` if there is no data.
    pub fn coordinate_to_logical(&self, x_css: f64) -> Option<f64> {
        self.inner.borrow().coordinate_to_logical(x_css)
    }
    /// X (CSS px) for an integer logical index.
    pub fn logical_to_coordinate(&self, logical: f64) -> Option<f64> {
        self.inner.borrow().logical_to_coordinate(logical)
    }
    /// Logical index for a UTC-seconds timestamp. `find_nearest` follows LWC lower-bound rules.
    pub fn time_to_index(&self, time: f64, find_nearest: bool) -> Option<i64> {
        self.inner.borrow().time_to_index(time, find_nearest)
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
        self.inner
            .borrow_mut()
            .set_visible_time_range(from_time, to_time);
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        self.inner.borrow_mut().render()
    }

    /// Paint the retained backend-neutral frame into the warm Canvas2D pane without changing the
    /// active onscreen backend. The TypeScript package uses this to implement its synchronous,
    /// deterministic composed screenshot API even while WebGPU is active.
    #[doc(hidden)]
    pub fn render_canvas2d_snapshot(&self) -> Result<(), JsValue> {
        self.inner.borrow().render_canvas2d()
    }

    /// Reports the active pane backend for diagnostics and runtime-matrix tests.
    pub fn backend_kind(&self) -> String {
        self.inner.borrow().backend_kind()
    }

    /// Internal id used by the package shell to route device-loss notifications to this chart.
    #[doc(hidden)]
    pub fn backend_runtime_id(&self) -> u32 {
        self.runtime_id
    }

    /// Deterministic browser-matrix hook. This is intentionally absent from the public TypeScript
    /// chart API; it marks the current device as lost so the next render exercises real failover.
    #[doc(hidden)]
    pub fn simulate_device_loss_for_test(&mut self) {
        if let Some(gfx) = self.inner.borrow().gfx.as_ref() {
            gfx.device_lost.store(true, Ordering::Release);
            notify_aion_backend_loss(self.runtime_id);
        }
    }
}

impl ChartInner {
    // --- rendering ---
}

/// Attempt to initialize WebGPU. A failure is recoverable because the same chart frame can be
/// executed by the Canvas2D backend.
async fn try_create_gfx(
    pane_canvas: web_sys::HtmlCanvasElement,
    css_width: f64,
    css_height: f64,
    dpr: f64,
    runtime_id: u32,
    simulate_adapter_failure: bool,
    force_fallback_adapter: bool,
) -> Result<Gfx, JsValue> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(pane_canvas))
        .map_err(|e| JsValue::from_str(&format!("create_surface failed: {e}")))?;
    if simulate_adapter_failure {
        return Err(JsValue::from_str(
            "request_adapter failed: deterministic runtime-matrix injection",
        ));
    }
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter,
        })
        .await
        .map_err(|e| JsValue::from_str(&format!("request_adapter failed: {e}")))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|e| JsValue::from_str(&format!("request_device failed: {e}")))?;
    let device_lost = Arc::new(AtomicBool::new(false));
    let lost_flag = Arc::clone(&device_lost);
    device.set_device_lost_callback(move |reason, _message| {
        // `Destroyed` is the expected callback when resources are intentionally dropped during an
        // already-completed fallback. Only an unknown/driver loss needs to initiate recovery.
        if reason == wgpu::DeviceLostReason::Unknown {
            lost_flag.store(true, Ordering::Release);
            notify_aion_backend_loss(runtime_id);
        }
    });
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
        device_lost,
    })
}
