//! WidgetLayout — LWC-style DOM layout with separate widget containers.
//!
//! Creates a CSS-grid layout matching LWC's table structure:
//!
//!   ┌─────────────────────┬──────────────┐
//!   │   Pane (chart area) │  Price Axis  │
//!   │  [grid canvas]      │  [base canvas│
//!   │  [chart canvas]     │   top canvas]│
//!   │  [overlay canvas]   │              │
//!   ├─────────────────────┴──────────────┤
//!   │         Time Axis                  │
//!   │  [base canvas | top canvas]        │
//!   └────────────────────────────────────┘
//!
//! Each widget owns its own DOM container + canvases, enabling:
//! - Proper per-widget event handlers (mouseEnter/Leave)
//! - Independently sized canvases (no wasted pixels)
//! - Natural clipping (candles can't paint into axis area)

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlCanvasElement, HtmlDivElement, HtmlElement};

/// A pair of canvases: base (static content) + top (dynamic/crosshair content).
/// Matches LWC's canvasBinding + topCanvasBinding pattern.
pub struct CanvasPair {
    pub base: HtmlCanvasElement,
    pub top: HtmlCanvasElement,
}

impl CanvasPair {
    fn new(doc: &Document, prefix: &str) -> Result<Self, JsValue> {
        let base = create_canvas(doc, &format!("{}-base", prefix), 0)?;
        let top = create_canvas(doc, &format!("{}-top", prefix), 1)?;
        Ok(Self { base, top })
    }

    pub fn set_size(&self, pw: u32, ph: u32) {
        for c in [&self.base, &self.top] {
            c.set_width(pw.max(1));
            c.set_height(ph.max(1));
        }
    }
}

/// The pane widget has 3 canvases: grid (z0), chart (z1), overlay/top (z2).
/// This matches LWC's PaneWidget which has multiple layers.
pub struct PaneCanvases {
    pub grid: HtmlCanvasElement,
    pub chart: HtmlCanvasElement,
    pub top: HtmlCanvasElement,
}

impl PaneCanvases {
    fn new(doc: &Document) -> Result<Self, JsValue> {
        let grid = create_canvas(doc, "raycore-pane-grid", 0)?;
        let chart = create_canvas(doc, "raycore-pane-chart", 1)?;
        let top = create_canvas(doc, "raycore-pane-top", 2)?;
        Ok(Self { grid, chart, top })
    }

    pub fn set_size(&self, pw: u32, ph: u32) {
        for c in [&self.grid, &self.chart, &self.top] {
            c.set_width(pw.max(1));
            c.set_height(ph.max(1));
        }
    }
}

/// The full widget layout — owns all DOM elements.
pub struct WidgetLayout {
    /// The outer container provided by the user.
    container: HtmlElement,
    /// The CSS-grid wrapper we create inside the container.
    grid_wrapper: HtmlDivElement,

    // ── Widget containers (real DOM elements for events) ──
    pub pane_container: HtmlDivElement,
    pub price_axis_container: HtmlDivElement,
    pub time_axis_container: HtmlDivElement,

    // ── Canvases per widget ──
    pub pane: PaneCanvases,
    pub price_axis: CanvasPair,
    pub time_axis: CanvasPair,
}

impl WidgetLayout {
    /// Create the full widget layout inside the given container div.
    /// Mirrors LWC's ChartWidget._init() DOM creation.
    pub fn new(container_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let doc = window.document().ok_or_else(|| JsValue::from_str("no document"))?;

        let container = doc
            .get_element_by_id(container_id)
            .ok_or_else(|| JsValue::from_str(&format!("container '{}' not found", container_id)))?
            .dyn_into::<HtmlElement>()
            .map_err(|_| JsValue::from_str("container is not an HTMLElement"))?;

        // Ensure container has position:relative
        let style = container.style();
        let pos = style.get_property_value("position").unwrap_or_default();
        if pos.is_empty() || pos == "static" {
            style.set_property("position", "relative")?;
        }
        style.set_property("overflow", "hidden")?;

        // Clear existing children
        container.set_inner_html("");

        // ── CSS-grid wrapper ──
        // LWC uses an HTML table; we use CSS grid for the same layout:
        //   columns: [1fr] [price_axis_width]
        //   rows:    [1fr] [time_axis_height]
        let grid_wrapper = doc.create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create grid wrapper"))?;
        grid_wrapper.style().set_css_text(
            "display:grid;\
             grid-template-columns:1fr auto;\
             grid-template-rows:1fr auto;\
             width:100%;height:100%;\
             position:absolute;top:0;left:0;\
             overflow:hidden;"
        );
        container.append_child(&grid_wrapper)?;

        // ── Pane container (chart area) — grid[0,0] ──
        let pane_container = create_widget_container(&doc, "raycore-pane")?;
        pane_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:1;grid-row:1;\
             min-width:0;min-height:0;"
        );
        grid_wrapper.append_child(&pane_container)?;

        let pane = PaneCanvases::new(&doc)?;
        pane_container.append_child(&pane.grid)?;
        pane_container.append_child(&pane.chart)?;
        pane_container.append_child(&pane.top)?;

        // ── Price axis container — grid[1,0] (right of pane) ──
        let price_axis_container = create_widget_container(&doc, "raycore-price-axis")?;
        price_axis_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:2;grid-row:1;\
             min-width:0;min-height:0;"
        );
        grid_wrapper.append_child(&price_axis_container)?;

        let price_axis = CanvasPair::new(&doc, "raycore-priceaxis")?;
        price_axis_container.append_child(&price_axis.base)?;
        price_axis_container.append_child(&price_axis.top)?;

        // ── Time axis container — grid[0-1,1] (bottom, spans pane + price axis) ──
        let time_axis_container = create_widget_container(&doc, "raycore-time-axis")?;
        time_axis_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:1 / -1;grid-row:2;\
             min-width:0;min-height:0;"
        );
        grid_wrapper.append_child(&time_axis_container)?;

        let time_axis = CanvasPair::new(&doc, "raycore-timeaxis")?;
        time_axis_container.append_child(&time_axis.base)?;
        time_axis_container.append_child(&time_axis.top)?;

        Ok(Self {
            container,
            grid_wrapper,
            pane_container,
            price_axis_container,
            time_axis_container,
            pane,
            price_axis,
            time_axis,
        })
    }

    /// The outer user-provided container.
    pub fn container(&self) -> &HtmlElement {
        &self.container
    }

    /// Get container CSS dimensions.
    pub fn container_css_size(&self) -> (f64, f64) {
        (
            self.container.client_width() as f64,
            self.container.client_height() as f64,
        )
    }

    /// Update the grid sizing based on computed axis dimensions (CSS px).
    /// Called when axis widths/heights change (e.g. after measuring text).
    pub fn update_axis_sizes(&self, price_axis_css_w: f64, time_axis_css_h: f64) {
        let _ = self.grid_wrapper.style().set_property(
            "grid-template-columns",
            &format!("1fr {}px", price_axis_css_w.round()),
        );
        let _ = self.grid_wrapper.style().set_property(
            "grid-template-rows",
            &format!("1fr {}px", time_axis_css_h.round()),
        );
    }

    /// Get the pane's actual CSS size (chart area only).
    pub fn pane_css_size(&self) -> (f64, f64) {
        (
            self.pane_container.client_width() as f64,
            self.pane_container.client_height() as f64,
        )
    }

    /// Get the price axis container's CSS size.
    pub fn price_axis_css_size(&self) -> (f64, f64) {
        (
            self.price_axis_container.client_width() as f64,
            self.price_axis_container.client_height() as f64,
        )
    }

    /// Get the time axis container's CSS size.
    pub fn time_axis_css_size(&self) -> (f64, f64) {
        (
            self.time_axis_container.client_width() as f64,
            self.time_axis_container.client_height() as f64,
        )
    }

    /// Resize all widget canvases to their container sizes at the given DPR.
    pub fn resize_all_canvases(&self, dpr: f64) {
        // Pane canvases
        let (pw, ph) = self.pane_css_size();
        let ppw = (pw * dpr).round() as u32;
        let pph = (ph * dpr).round() as u32;
        self.pane.set_size(ppw, pph);

        // Price axis canvases
        let (aw, ah) = self.price_axis_css_size();
        let apw = (aw * dpr).round() as u32;
        let aph = (ah * dpr).round() as u32;
        self.price_axis.set_size(apw, aph);

        // Time axis canvases
        let (tw, th) = self.time_axis_css_size();
        let tpw = (tw * dpr).round() as u32;
        let tph = (th * dpr).round() as u32;
        self.time_axis.set_size(tpw, tph);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn create_canvas(
    doc: &Document,
    id: &str,
    z_index: u32,
) -> Result<HtmlCanvasElement, JsValue> {
    let canvas = doc
        .create_element("canvas")?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("failed to create canvas"))?;

    canvas.set_id(id);
    canvas.style().set_css_text(&format!(
        "position:absolute;top:0;left:0;width:100%;height:100%;display:block;z-index:{};",
        z_index
    ));

    Ok(canvas)
}

fn create_widget_container(
    doc: &Document,
    id: &str,
) -> Result<HtmlDivElement, JsValue> {
    let div = doc
        .create_element("div")?
        .dyn_into::<HtmlDivElement>()
        .map_err(|_| JsValue::from_str("failed to create div"))?;
    div.set_id(id);
    Ok(div)
}
