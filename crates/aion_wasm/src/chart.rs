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
    /// Adds a series and returns its id. `kind`: 0 candles, 1 bars, 2 line, 3 area, 4 histogram.
    pub fn add_series(&mut self, kind: u8) -> u32 {
        let id = self.engine.add_series(SeriesKind::from_u8(kind));
        id as u32
    }

    pub fn add_sma(&mut self, source_id: u32, period: u32) -> u32 {
        self.engine
            .add_sma(source_id as SeriesId, period as usize)
            .map(|id| id as u32)
            .unwrap_or(u32::MAX)
    }

    pub fn add_ema(&mut self, source_id: u32, period: u32) -> u32 {
        self.engine
            .add_ema(source_id as SeriesId, period as usize)
            .map(|id| id as u32)
            .unwrap_or(u32::MAX)
    }

    pub fn add_bollinger(&mut self, source_id: u32, period: u32, deviation: f64) -> Vec<u32> {
        self.engine
            .add_bollinger(source_id as SeriesId, period as usize, deviation)
            .into_iter()
            .map(|id| id as u32)
            .collect()
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
        self.engine
            .install_series_data(id as SeriesId, s.times, s.open, s.high, s.low, s.close);
    }

    pub fn set_series_data_typed(
        &mut self,
        id: u32,
        times: &Float64Array,
        open: &Float64Array,
        high: &Float64Array,
        low: &Float64Array,
        close: &Float64Array,
    ) {
        let s = match aion_core::model::data_validation::sanitize_ohlc_owned(
            times.to_vec(),
            open.to_vec(),
            high.to_vec(),
            low.to_vec(),
            close.to_vec(),
        ) {
            Ok(s) => s,
            Err(e) => {
                web_sys::console::warn_1(&format!("aion: set_series_data rejected — {e}").into());
                return;
            }
        };
        if !s.report.is_clean() {
            web_sys::console::warn_1(&format!("aion: set_series_data sanitized data — accepted {}, dropped {} invalid, {} duplicate{}", s.report.accepted, s.report.dropped_invalid, s.report.dropped_duplicate, if s.report.reordered { ", reordered" } else { "" }).into());
        }
        self.engine
            .install_series_data(id as SeriesId, s.times, s.open, s.high, s.low, s.close);
    }

    /// Streaming update of the main series (append new time or replace last).
    pub fn update_bar(&mut self, time: f64, open: f64, high: f64, low: f64, close: f64) {
        let id = self.series[0].id as u32;
        self.update_series_bar(id, time, open, high, low, close);
    }

    /// Streaming update of the series with `series_id` (append a new time or replace the last).
    pub fn update_series_bar(
        &mut self,
        series_id: u32,
        time: f64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) {
        // Ignore updates to an unknown series rather than corrupting the data layer.
        if !self.series.iter().any(|s| s.id == series_id as SeriesId) {
            web_sys::console::warn_1(&"aion: update_bar for unknown series id".into());
            return;
        }
        // Drop a bad tick rather than corrupting the series (roadmap Phase A3).
        if !self
            .engine
            .update_series_bar(series_id as SeriesId, time, [open, high, low, close])
        {
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

    pub fn set_series_visible(&mut self, id: u32, visible: bool) {
        self.engine.set_series_visible(id as SeriesId, visible);
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
        let id = self.next_price_line_id;
        self.next_price_line_id += 1;
        if let Some(s) = self
            .series
            .iter_mut()
            .find(|s| s.id == series_id as SeriesId)
        {
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
        self.engine
            .set_series_markers(series_id as SeriesId, markers);
    }

    pub fn set_series_markers_auto_scale(&mut self, series_id: u32, enabled: bool) {
        self.engine
            .set_series_markers_auto_scale(series_id as SeriesId, enabled);
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
            s.left_scale = false;
            pane_index = s.pane_index;
        }
        if let Some(p) = self.panes.get_mut(pane_index) {
            p.overlay_top = top.clamp(0.0, 1.0);
            p.overlay_bottom = bottom.clamp(0.0, 1.0);
            p.overlay_scale
                .set_scale_margins(p.overlay_top, p.overlay_bottom);
            p.refresh_internal_margins();
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
                self.recompute_layout(false);
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
            self.recompute_layout(false);
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
            web_sys::console::warn_1(
                &format!("aion: apply_options ignored malformed patch — {e}").into(),
            );
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
        self.recompute_layout(true);
    }

    /// Negotiates the price-axis width against its labels and sets the time-scale width /
    /// price-scale height accordingly. Idempotent; called on resize, data change, and render.
    /// (The axis labels depend only on the price range, so one refinement pass converges.)
    fn recompute_layout(&mut self, allow_axis_shrink: bool) {
        let content_h = (self.css_height - TIME_AXIS_HEIGHT).max(1.0);
        self.engine.layout_panes(content_h);
        let options = self.opts();
        let measured_axis_w = if options.right_price_scale.visible {
            self.compute_price_axis_width(PriceScaleTarget::Right)
        } else {
            0.0
        };
        let measured_left_axis_w = if options.left_price_scale.visible {
            self.compute_price_axis_width(PriceScaleTarget::Left)
        } else {
            0.0
        };
        let mut axis_w = if options.right_price_scale.visible {
            negotiated_axis_width(self.axis_w, measured_axis_w, allow_axis_shrink)
        } else {
            0.0
        };
        let mut left_axis_w = if options.left_price_scale.visible {
            negotiated_axis_width(self.left_axis_w, measured_left_axis_w, allow_axis_shrink)
        } else {
            0.0
        };
        for _ in 0..2 {
            let pane_w = (self.css_width - left_axis_w - axis_w).max(1.0);
            self.pane_left = left_axis_w;
            self.left_axis_w = left_axis_w;
            self.axis_w = axis_w;
            self.time_scale.set_width(pane_w);
            self.engine.autoscale_visible();
            let measured_new_w = if options.right_price_scale.visible {
                self.compute_price_axis_width(PriceScaleTarget::Right)
            } else {
                0.0
            };
            let measured_new_left_w = if options.left_price_scale.visible {
                self.compute_price_axis_width(PriceScaleTarget::Left)
            } else {
                0.0
            };
            let new_w = if options.right_price_scale.visible {
                negotiated_axis_width(axis_w, measured_new_w, allow_axis_shrink)
            } else {
                0.0
            };
            let new_left_w = if options.left_price_scale.visible {
                negotiated_axis_width(left_axis_w, measured_new_left_w, allow_axis_shrink)
            } else {
                0.0
            };
            if new_w == axis_w && new_left_w == left_axis_w {
                break;
            }
            axis_w = new_w;
            left_axis_w = new_left_w;
        }
        self.pane_left = left_axis_w;
        self.left_axis_w = left_axis_w;
        self.pane_w = (self.css_width - left_axis_w - axis_w).max(1.0);
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
        self.engine.fit_content();
    }
    pub fn scroll_position(&self) -> f64 {
        self.engine.scroll_position()
    }
    pub fn scroll_to_position(&mut self, position: f64) {
        self.engine.scroll_to_position(position);
    }
    pub fn scroll_to_real_time(&mut self) {
        self.engine.scroll_to_real_time();
    }
    pub fn reset_time_scale(&mut self) {
        self.engine.reset_time_scale();
    }
    pub fn time_scale_width(&self) -> f64 {
        self.time_scale.width()
    }
    pub fn time_scale_height(&self) -> f64 {
        if self.time_visible {
            TIME_AXIS_HEIGHT
        } else {
            0.0
        }
    }
    pub fn price_scale_width(&self, pane: usize, target: u8) -> f64 {
        if pane >= self.panes.len() {
            return 0.0;
        }
        match price_scale_target_from_u8(target) {
            PriceScaleTarget::Right => self.axis_w,
            PriceScaleTarget::Left => self.left_axis_w,
            PriceScaleTarget::Overlay => 0.0,
        }
    }
    pub fn price_scale_visible_range(&self, pane: usize, target: u8) -> Vec<f64> {
        self.engine
            .price_scale_visible_range_for(pane, price_scale_target_from_u8(target))
            .map(|(from, to)| vec![from, to])
            .unwrap_or_default()
    }
    pub fn set_price_scale_visible_range(&mut self, pane: usize, target: u8, from: f64, to: f64) {
        self.engine.set_price_scale_visible_range_for(
            pane,
            price_scale_target_from_u8(target),
            from,
            to,
        );
    }
    pub fn price_scale_auto_scale(&self, pane: usize, target: u8) -> Option<bool> {
        self.engine
            .price_scale_auto_scale_for(pane, price_scale_target_from_u8(target))
    }
    pub fn set_price_scale_auto_scale(&mut self, pane: usize, target: u8, enabled: bool) {
        self.engine.set_price_scale_auto_scale_for(
            pane,
            price_scale_target_from_u8(target),
            enabled,
        );
    }
    pub fn price_scale_inverted(&self, pane: usize, target: u8) -> Option<bool> {
        self.engine
            .price_scale_inverted_for(pane, price_scale_target_from_u8(target))
    }
    pub fn set_price_scale_inverted(&mut self, pane: usize, target: u8, inverted: bool) {
        self.engine.set_price_scale_inverted_for(
            pane,
            price_scale_target_from_u8(target),
            inverted,
        );
    }
    pub fn price_scale_margins(&self, pane: usize, target: u8) -> Vec<f64> {
        self.engine
            .price_scale_margins_for(pane, price_scale_target_from_u8(target))
            .map(|(top, bottom)| vec![top, bottom])
            .unwrap_or_default()
    }
    pub fn set_price_scale_margins(&mut self, pane: usize, target: u8, top: f64, bottom: f64) {
        self.engine.set_price_scale_margins_for(
            pane,
            price_scale_target_from_u8(target),
            top,
            bottom,
        );
    }
    pub fn price_scale_mode(&self, pane: usize, target: u8) -> Option<u8> {
        self.engine
            .price_scale_mode_for(pane, price_scale_target_from_u8(target))
            .map(price_scale_mode_to_u8)
    }
    pub fn set_price_scale_mode(&mut self, pane: usize, target: u8, mode: u8) {
        self.engine.set_price_scale_mode_for(
            pane,
            price_scale_target_from_u8(target),
            price_scale_mode_from_u8(mode),
        );
        // A mode change is a full layout invalidation in LWC: label formatting can become wider
        // (percentage) or narrower (indexed/normal), so the grow-fast/shrink-on-full-layout axis
        // policy must be allowed to renegotiate in both directions immediately.
        self.recompute_layout(true);
    }
    pub fn series_pane_index(&self, id: u32) -> Option<usize> {
        self.engine
            .series_price_scale(id as usize)
            .map(|(pane, _)| pane)
    }
    pub fn series_is_overlay(&self, id: u32) -> Option<bool> {
        self.engine
            .series_price_scale(id as usize)
            .map(|(_, target)| target == PriceScaleTarget::Overlay)
    }
    pub fn series_price_scale_id(&self, id: u32) -> Option<u8> {
        self.engine
            .series_price_scale(id as usize)
            .map(|(_, target)| price_scale_target_to_u8(target))
    }
    pub fn set_series_price_scale(&mut self, id: u32, target: u8) {
        self.engine
            .set_series_price_scale(id as usize, price_scale_target_from_u8(target));
        self.recompute_layout(true);
    }
    pub fn series_price_to_coordinate(&self, id: u32, price: f64) -> Option<f64> {
        self.engine.series_price_to_coordinate(id as usize, price)
    }
    pub fn series_coordinate_to_price(&self, id: u32, coordinate: f64) -> Option<f64> {
        self.engine
            .series_coordinate_to_price(id as usize, coordinate)
    }
    pub fn series_kind(&self, id: u32) -> Option<u8> {
        self.engine.series_kind(id as usize).map(SeriesKind::to_u8)
    }
    pub fn series_data_by_index(&self, id: u32, index: f64, mismatch: i8) -> Vec<f64> {
        if !index.is_finite() || index.fract() != 0.0 {
            return Vec::new();
        }
        self.engine
            .series_data_by_index(
                id as usize,
                index as i64,
                mismatch_direction_from_i8(mismatch),
            )
            .map(|point| {
                vec![
                    point.time as f64,
                    point.open,
                    point.high,
                    point.low,
                    point.close,
                ]
            })
            .unwrap_or_default()
    }
    pub fn series_data(&self, id: u32) -> Vec<f64> {
        let points = self.engine.series_data(id as usize);
        let mut output = Vec::with_capacity(points.len() * 5);
        for point in points {
            output.extend_from_slice(&[
                point.time as f64,
                point.open,
                point.high,
                point.low,
                point.close,
            ]);
        }
        output
    }
    pub fn series_bars_in_logical_range(&self, id: u32, from: f64, to: f64) -> Vec<f64> {
        self.engine
            .series_bars_in_logical_range(id as usize, from, to)
            .map(|info| {
                let mut output = vec![info.bars_before, info.bars_after];
                if let (Some(from), Some(to)) = (info.from, info.to) {
                    output.extend_from_slice(&[from as f64, to as f64]);
                }
                output
            })
            .unwrap_or_default()
    }
    pub fn set_crosshair(&mut self, x_css: f64, y_css: f64) {
        self.crosshair = Some((x_css, y_css));
    }
    pub fn clear_crosshair(&mut self) {
        self.crosshair = None;
    }
    pub fn price_axis_width(&self) -> f64 {
        self.axis_w
    }
    pub fn pane_left(&self) -> f64 {
        self.pane_left
    }

    // --- coordinate & logical-range API (roadmap Phase A4) ---
    //
    // Reflects the state of the last render (scale height/width, price range). All coordinates
    // are media (CSS) pixels relative to the pane origin, matching the pointer coords JS passes
    // to `set_crosshair`. `None`/empty means the query falls off the chart or there is no data.

    /// Y (CSS px) for a price on the active price scale, or `None` if the scale has no range yet.
    /// In percentage/indexed modes the price is its own base value (as in the render path).
    pub fn price_to_coordinate(&self, price: f64) -> Option<f64> {
        self.engine
            .series_price_to_coordinate(self.series[0].id, price)
    }

    /// Price for a Y (CSS px), or `None` if the scale has no range yet.
    pub fn coordinate_to_price(&self, y_css: f64) -> Option<f64> {
        self.engine
            .series_coordinate_to_price(self.series[0].id, y_css)
    }

    /// X (CSS px) for a UTC-seconds timestamp that sits exactly on a data point, else `None`
    /// (mirrors LWC `timeToCoordinate`, which does not snap to the nearest bar).
    pub fn time_to_coordinate(&self, time: f64) -> Option<f64> {
        self.engine.time_to_coordinate(time)
    }

    /// UTC-seconds timestamp of the data point nearest to X (CSS px), or `None` if X maps outside
    /// the data range (mirrors LWC `coordinateToTime`).
    pub fn coordinate_to_time(&self, x_css: f64) -> Option<f64> {
        self.engine.coordinate_to_time(x_css)
    }

    /// Integer logical bar owning an X coordinate, or `None` when there is no data. May be negative
    /// or beyond the last bar, matching LWC's public `coordinateToLogical`.
    pub fn coordinate_to_logical(&self, x_css: f64) -> Option<f64> {
        self.engine.coordinate_to_logical(x_css)
    }

    pub fn logical_to_coordinate(&self, logical: f64) -> Option<f64> {
        self.engine.logical_to_coordinate(logical)
    }

    pub fn time_to_index(&self, time: f64, find_nearest: bool) -> Option<i64> {
        self.engine.time_to_index(time, find_nearest)
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
        match self.engine.visible_logical_range() {
            Some((from, to)) => vec![from, to],
            None => Vec::new(),
        }
    }

    /// Set the visible window in logical (bar) units. No-op if `from > to`. Call `render()` after.
    pub fn set_visible_logical_range(&mut self, from: f64, to: f64) {
        self.engine.set_visible_logical_range(from, to);
    }

    /// Visible window as `[from_time, to_time]` UTC seconds (data points nearest each edge), or
    /// empty when there is no data.
    pub fn visible_time_range(&self) -> Vec<f64> {
        self.engine
            .visible_time_range()
            .map(|(from, to)| vec![from, to])
            .unwrap_or_default()
    }

    /// Set the visible window to span the data points bracketing `[from_time, to_time]` (UTC
    /// seconds). No-op if the times are reversed or there is no data. Call `render()` after.
    pub fn set_visible_time_range(&mut self, from_time: f64, to_time: f64) {
        self.engine.set_visible_time_range(from_time, to_time);
    }

    // --- rendering ---

    /// Reports the active pane backend for diagnostics and runtime-matrix tests.
    pub fn backend_kind(&self) -> String {
        if self.gfx.is_some() {
            "webgpu".into()
        } else {
            "canvas2d".into()
        }
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        // ---- layout (price axis width negotiated against the price labels) ----
        self.recompute_layout(false);

        // time tick marks: built once (needs &mut), shared by GPU grid + 2D labels
        let pixels_per_character = (FONT_SIZE + 4.0) * 5.0 / 8.0;
        let max_label_width = pixels_per_character * TICK_MARK_MAX_CHARS;
        let axis_ctx = &self.axis_ctx;
        let dpr = self.dpr;
        self.axis_frame = self.engine.build_axis_frame(max_label_width, |text| {
            measure_text_ctx(axis_ctx, dpr, text)
        });

        // ---- GPU: one scissored draw group per stacked pane ----
        // The headless engine owns chart geometry. The WASM host only adds browser-adapter
        // concerns such as crosshair interaction and text labels.
        self.engine.build_frame_into(&mut self.frame);

        if self
            .gfx
            .as_ref()
            .is_some_and(|gfx| gfx.device_lost.load(Ordering::Acquire))
        {
            self.activate_canvas2d("WebGPU device was lost");
        }

        let bg = Color::parse_css(&self.opts().layout.background.color)
            .unwrap_or(Color::rgb(0xff, 0xff, 0xff));
        let pane_outcome = if self.gfx.is_some() {
            let engine_frame = &self.frame;
            self.gpu_groups
                .resize_with(engine_frame.panes.len(), DrawGroup::default);
            self.gpu_groups.truncate(engine_frame.panes.len());
            for (group, pane_frame) in self.gpu_groups.iter_mut().zip(&engine_frame.panes) {
                group.scissor = Some(pane_frame.scissor);
                group.under_quads.clear();
                group.fill_tris.clear();
                group.stroke_tris.clear();
                group.quads.clear();
                group.tex_quads.clear();
                // Convert the shared frame only at the WebGPU backend boundary.
                geom_prims_to_tris(
                    &pane_frame.main,
                    &pane_frame.points,
                    &mut group.fill_tris,
                    &mut group.stroke_tris,
                );
                prims_to_instances(&pane_frame.under, &mut group.under_quads);
                prims_to_instances(&pane_frame.main, &mut group.quads);
            }
            let groups = &self.gpu_groups[..];
            let Some(gfx) = self.gfx.as_mut() else {
                return Err(JsValue::from_str("WebGPU state disappeared mid-render"));
            };
            gfx.msaa.ensure(
                &gfx.device,
                gfx.config.format,
                gfx.config.width,
                gfx.config.height,
            );

            let acquired = match gfx.surface.get_current_texture() {
                Ok(frame) => Ok(Some(frame)),
                Err(error) => match surface_error_action(&error) {
                    SurfaceErrorAction::Reconfigure => {
                        // Resize and suspend/resume can invalidate only the swapchain. Reconfigure
                        // and retry once; if that fails, the warm Canvas2D pane takes over.
                        gfx.surface.configure(&gfx.device, &gfx.config);
                        match gfx.surface.get_current_texture() {
                            Ok(frame) => Ok(Some(frame)),
                            Err(retry_error)
                                if surface_error_action(&retry_error)
                                    == SurfaceErrorAction::SkipFrame =>
                            {
                                Ok(None)
                            }
                            Err(retry_error) => Err(retry_error),
                        }
                    }
                    SurfaceErrorAction::SkipFrame => Ok(None),
                    SurfaceErrorAction::Fallback => Err(error),
                },
            };

            match acquired {
                Ok(Some(frame)) => {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
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
                        groups,
                    );
                    frame.present();
                    PaneRenderOutcome::Presented
                }
                Ok(None) => PaneRenderOutcome::Timeout,
                Err(error) => PaneRenderOutcome::Fallback(format!(
                    "WebGPU surface acquisition failed after recovery: {error}"
                )),
            }
        } else {
            PaneRenderOutcome::Canvas2d
        };

        match pane_outcome {
            PaneRenderOutcome::Presented => {}
            PaneRenderOutcome::Timeout => {
                // Keep the last complete frame. The next animation/input repaint retries.
                return Ok(());
            }
            PaneRenderOutcome::Fallback(reason) => {
                self.activate_canvas2d(&reason);
                self.render_canvas2d()?;
            }
            PaneRenderOutcome::Canvas2d => self.render_canvas2d()?,
        }

        self.draw_axes_2d(&self.axis_frame)?;
        Ok(())
    }

    // --- data / scale bookkeeping ---

    fn compute_price_axis_width(&mut self, target: PriceScaleTarget) -> f64 {
        let axis_ctx = self.axis_ctx.clone();
        let dpr = self.dpr;
        self.engine
            .optimal_price_axis_width_for(target, |text| measure_text_ctx(&axis_ctx, dpr, text))
    }

    // ---- Canvas2D axis overlay ----

    fn draw_axes_2d(&self, axis_frame: &AxisFrame) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        let dpr = self.dpr;
        let bitmap_w = self.bitmap_w as f64;
        let bitmap_h = self.bitmap_h as f64;
        let pane_left = self.pane_left;
        let pane_w = self.pane_w;
        let pane_h = self.pane_h;

        ctx.clear_rect(0.0, 0.0, bitmap_w, bitmap_h);
        let border_w = 1f64.max(dpr.floor());

        ctx.set_fill_style_str(BORDER_CSS);
        if self.left_axis_w > 0.0 {
            ctx.fill_rect(
                (pane_left * dpr).round() - border_w,
                0.0,
                border_w,
                (pane_h * dpr).round(),
            );
        }
        if self.axis_w > 0.0 {
            ctx.fill_rect(
                ((pane_left + pane_w) * dpr).round(),
                0.0,
                border_w,
                (pane_h * dpr).round(),
            );
        }
        ctx.fill_rect(0.0, (pane_h * dpr).round(), bitmap_w, border_w);

        // separators between stacked panes (roadmap Phase B1): a border line at each pane boundary
        for separator in &axis_frame.separators {
            let y = (separator * dpr).round();
            ctx.fill_rect(
                (pane_left * dpr).round(),
                y,
                (pane_w * dpr).round(),
                (PANE_SEPARATOR * dpr).max(border_w),
            );
        }

        self.draw_axis_labels(axis_frame, dpr)?;
        Ok(())
    }

    fn draw_axis_labels(&self, axis_frame: &AxisFrame, dpr: f64) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        // Label backgrounds are bitmap-aligned geometry, matching LWC's bitmap-coordinate pass.
        for label in &axis_frame.labels {
            if let Some((x, y, w, h, color)) = label.background {
                ctx.set_fill_style_str(&color.to_hex());
                ctx.fill_rect(
                    (x * dpr).round(),
                    (y * dpr).round(),
                    (w * dpr).round(),
                    (h * dpr).round(),
                );
            }
        }

        // LWC draws glyphs in media-coordinate space: the context is scaled by DPR while the font
        // remains 12 CSS px. Using an independently hinted 12*dpr bitmap font is observably
        // different at fractional DPR even when every logical coordinate is identical.
        ctx.save();
        if let Err(error) = ctx.scale(dpr, dpr) {
            ctx.restore();
            return Err(error);
        }
        ctx.set_text_baseline("middle");
        let mut draw_result = Ok(());
        for label in &axis_frame.labels {
            ctx.set_font(&if label.bold {
                format!("bold {FONT_SIZE}px {FONT_FAMILY}")
            } else {
                format!("{FONT_SIZE}px {FONT_FAMILY}")
            });
            ctx.set_text_align(match label.align {
                AxisTextAlign::Left => "left",
                AxisTextAlign::Right => "right",
                AxisTextAlign::Center => "center",
            });
            ctx.set_fill_style_str(&label.color.to_hex());
            let metrics_text = match label.midpoint {
                AxisTextMidpoint::None => None,
                AxisTextMidpoint::Label => Some(label.text.as_str()),
                AxisTextMidpoint::StableTime => Some("Apr0"),
            };
            let y_mid_correction = metrics_text
                .and_then(|text| ctx.measure_text(text).ok())
                .map(|metrics| {
                    (metrics.actual_bounding_box_ascent() - metrics.actual_bounding_box_descent())
                        / 2.0
                })
                .unwrap_or(0.0);
            if let Err(error) = ctx.fill_text(&label.text, label.x, label.y + y_mid_correction) {
                draw_result = Err(error);
                break;
            }
        }
        ctx.restore();
        draw_result
    }

    /// Permanently switch this chart instance to its already-initialized Canvas2D pane.
    fn activate_canvas2d(&mut self, reason: &str) {
        if self.gfx.take().is_some() {
            set_backend_visibility(&self.gpu_pane, &self.fallback_pane, false);
            web_sys::console::warn_1(&format!("aion: {reason}; continuing with Canvas2D").into());
        }
    }

    /// Execute the exact same retained frame consumed by WebGPU through Canvas2D.
    fn render_canvas2d(&self) -> Result<(), JsValue> {
        let ctx = &self.pane_ctx;
        let width = self.bitmap_w as f64;
        let height = self.bitmap_h as f64;
        ctx.clear_rect(0.0, 0.0, width, height);
        let bg = self.opts().layout.background.color;
        ctx.set_fill_style_str(&bg);
        ctx.fill_rect(0.0, 0.0, width, height);
        let mut target = crate::canvas2d_target::WasmCanvas2d::new(ctx);
        let viewport = CanvasViewport {
            width: width as f32,
            height: height as f32,
        };
        for pane in &self.frame.panes {
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

fn measure_text_ctx(ctx: &CanvasRenderingContext2d, dpr: f64, text: &str) -> f64 {
    ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr));
    ctx.measure_text(text).map(|m| m.width()).unwrap_or(0.0) / dpr
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
