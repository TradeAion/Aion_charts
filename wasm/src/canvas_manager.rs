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
use web_sys::{CssStyleDeclaration, Document, HtmlCanvasElement, HtmlDivElement, HtmlElement};

use crate::utils;

/// A pair of canvases: base (static content) + top (dynamic/crosshair content).
/// Matches LWC's canvasBinding + topCanvasBinding pattern.
pub struct CanvasPair {
    pub base: HtmlCanvasElement,
    pub top: HtmlCanvasElement,
}

impl CanvasPair {
    fn new(doc: &Document, prefix: &str) -> Result<Self, JsValue> {
        let base = utils::create_canvas(doc, &format!("{}-base", prefix), 0)?;
        let top = utils::create_canvas(doc, &format!("{}-top", prefix), 1)?;
        Ok(Self { base, top })
    }

    /// Set size with explicit CSS dimensions for crisp rendering.
    /// `pw`, `ph` are physical pixel dimensions (bitmap size).
    /// `css_w`, `css_h` are CSS pixel dimensions for layout.
    pub fn set_size_with_css(&self, pw: u32, ph: u32, css_w: f64, css_h: f64) {
        for c in [&self.base, &self.top] {
            utils::set_canvas_size_with_css(c, pw.max(1), ph.max(1), css_w, css_h);
        }
    }
}

/// The pane widget has 2 canvases: chart (z0), overlay/top (z1).
pub struct PaneCanvases {
    pub chart: HtmlCanvasElement,
    pub top: HtmlCanvasElement,
}

impl PaneCanvases {
    fn new(doc: &Document) -> Result<Self, JsValue> {
        let chart = utils::create_canvas(doc, "raycore-pane-chart", 0)?;
        let top = utils::create_canvas(doc, "raycore-pane-top", 1)?;
        Ok(Self { chart, top })
    }

    /// Set size with explicit CSS dimensions for crisp rendering.
    pub fn set_size_with_css(&self, pw: u32, ph: u32, css_w: f64, css_h: f64) {
        for c in [&self.chart, &self.top] {
            utils::set_canvas_size_with_css(c, pw.max(1), ph.max(1), css_w, css_h);
        }
    }
}

/// The full widget layout — owns all DOM elements.
pub struct WidgetLayout {
    /// The outer container provided by the user.
    container: HtmlElement,
    /// The CSS-grid wrapper we create inside the container.
    pub grid_wrapper: HtmlDivElement,

    // ── Widget containers (real DOM elements for events) ──
    pub pane_container: HtmlDivElement,
    pub price_axis_container: HtmlDivElement,
    pub time_axis_container: HtmlDivElement,
    /// Corner stub — bottom-right intersection of time axis row + price axis column.
    /// Matches LWC's PriceAxisStub widget.
    pub corner_stub_container: HtmlDivElement,

    // ── Canvases per widget ──
    pub pane: PaneCanvases,
    pub price_axis: CanvasPair,
    pub time_axis: CanvasPair,
    /// Corner stub canvas (single layer — just bg + borders).
    pub corner_stub: HtmlCanvasElement,
}

impl WidgetLayout {
    /// Create the full widget layout inside the given container div.
    /// Mirrors LWC's ChartWidget._init() DOM creation.
    pub fn new(container_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let doc = window
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;

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
        let grid_wrapper = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create grid wrapper"))?;
        grid_wrapper.style().set_css_text(
            "display:grid;\
             grid-template-columns:1fr auto;\
             grid-template-rows:1fr auto;\
             width:100%;height:100%;\
             position:absolute;top:0;left:0;\
             overflow:hidden;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
        );
        container.append_child(&grid_wrapper)?;

        // ── Pane container (chart area) — grid[0,0] ──
        let pane_container = create_widget_container(&doc, "raycore-pane")?;
        pane_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:1;grid-row:1;\
             min-width:0;min-height:0;\
             cursor:crosshair;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
        );
        grid_wrapper.append_child(&pane_container)?;

        let pane = PaneCanvases::new(&doc)?;
        pane_container.append_child(&pane.chart)?;
        pane_container.append_child(&pane.top)?;

        // ── Price axis container — grid[1,0] (right of pane) ──
        let price_axis_container = create_widget_container(&doc, "raycore-price-axis")?;
        price_axis_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:2;grid-row:1;\
             min-width:0;min-height:0;\
             cursor:ns-resize;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
        );
        grid_wrapper.append_child(&price_axis_container)?;

        let price_axis = CanvasPair::new(&doc, "raycore-priceaxis")?;
        price_axis_container.append_child(&price_axis.base)?;
        price_axis_container.append_child(&price_axis.top)?;

        // ── Time axis container — grid[1,1] (bottom of pane, NOT spanning price axis) ──
        // LWC: time axis td is sized to paneWidth only; corner stub is separate.
        let time_axis_container = create_widget_container(&doc, "raycore-time-axis")?;
        time_axis_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:1;grid-row:2;\
             min-width:0;min-height:0;\
             cursor:ew-resize;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
        );
        grid_wrapper.append_child(&time_axis_container)?;

        let time_axis = CanvasPair::new(&doc, "raycore-timeaxis")?;
        time_axis_container.append_child(&time_axis.base)?;
        time_axis_container.append_child(&time_axis.top)?;

        // ── Corner stub — grid[2,2] (bottom-right intersection) ──
        // LWC: PriceAxisStub draws bg + border at intersection.
        let corner_stub_container = create_widget_container(&doc, "raycore-corner-stub")?;
        corner_stub_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:2;grid-row:2;\
             min-width:0;min-height:0;\
             cursor:default;",
        );
        grid_wrapper.append_child(&corner_stub_container)?;

        let corner_stub = utils::create_canvas(&doc, "raycore-corner-stub-canvas", 0)?;
        corner_stub_container.append_child(&corner_stub)?;

        Ok(Self {
            container,
            grid_wrapper,
            pane_container,
            price_axis_container,
            time_axis_container,
            corner_stub_container,
            pane,
            price_axis,
            time_axis,
            corner_stub,
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
        let cols = format!("1fr {}px", price_axis_css_w.round());
        let rows = format!("1fr {}px", time_axis_css_h.round());
        let style = self.grid_wrapper.style();
        set_style_property_if_needed(&style, "grid-template-columns", &cols);
        set_style_property_if_needed(&style, "grid-template-rows", &rows);
    }

    /// Update grid sizing with subpane support.
    /// `subpane_heights` contains CSS-px heights for each indicator subpane.
    /// Grid rows: main(1fr) [sep(1px) pane(Npx)]... time_axis(Mpx)
    pub fn update_axis_sizes_with_subpanes(
        &self,
        price_axis_css_w: f64,
        time_axis_css_h: f64,
        subpane_heights: &[f64],
    ) {
        let cols = format!("1fr {}px", price_axis_css_w.round());
        let style = self.grid_wrapper.style();
        set_style_property_if_needed(&style, "grid-template-columns", &cols);

        // Build rows: "1fr [1px Npx]... Mpx"
        let mut rows = String::from("1fr ");
        for h in subpane_heights {
            rows.push_str(&format!("1px {}px ", h.max(30.0)));
        }
        rows.push_str(&format!("{}px", time_axis_css_h.round()));

        set_style_property_if_needed(&style, "grid-template-rows", &rows);

        // Move time axis + corner stub to the correct last row
        let time_row = 2 + subpane_heights.len() * 2;
        let time_row_str = time_row.to_string();
        let time_axis_style = self.time_axis_container.style();
        set_style_property_if_needed(&time_axis_style, "grid-row", &time_row_str);
        let corner_style = self.corner_stub_container.style();
        set_style_property_if_needed(&corner_style, "grid-row", &time_row_str);
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

    /// Get the corner stub container's CSS size.
    pub fn corner_stub_css_size(&self) -> (f64, f64) {
        (
            self.corner_stub_container.client_width() as f64,
            self.corner_stub_container.client_height() as f64,
        )
    }

    /// Resize all widget canvases to their container sizes at the given DPR.
    /// Uses fallback `round(css * dpr)` sizing. Prefer `resize_canvases_exact`
    /// when device-pixel-content-box sizes are available from ResizeObserver.
    pub fn resize_all_canvases(&self, dpr: f64) {
        // Pane canvases
        let (pw, ph) = self.pane_css_size();
        let ppw = (pw * dpr).round() as u32;
        let pph = (ph * dpr).round() as u32;
        self.pane.set_size_with_css(ppw, pph, pw, ph);

        // Price axis canvases
        let (aw, ah) = self.price_axis_css_size();
        let apw = (aw * dpr).round() as u32;
        let aph = (ah * dpr).round() as u32;
        self.price_axis.set_size_with_css(apw, aph, aw, ah);

        // Time axis canvases
        let (tw, th) = self.time_axis_css_size();
        let tpw = (tw * dpr).round() as u32;
        let tph = (th * dpr).round() as u32;
        self.time_axis.set_size_with_css(tpw, tph, tw, th);

        // Corner stub canvas
        let (sw, sh) = self.corner_stub_css_size();
        let spw = (sw * dpr).round() as u32;
        let sph = (sh * dpr).round() as u32;
        utils::set_canvas_size_with_css(&self.corner_stub, spw.max(1), sph.max(1), sw, sh);
    }

    /// Resize a specific widget's canvases using exact device-pixel sizes
    /// reported by `ResizeObserver` with `device-pixel-content-box`.
    /// This avoids the ±1px rounding error from `round(css * dpr)`.
    pub fn resize_pane_exact(&self, exact_pw: u32, exact_ph: u32, css_w: f64, css_h: f64) {
        self.pane
            .set_size_with_css(exact_pw.max(1), exact_ph.max(1), css_w, css_h);
    }

    pub fn resize_price_axis_exact(&self, exact_pw: u32, exact_ph: u32, css_w: f64, css_h: f64) {
        self.price_axis
            .set_size_with_css(exact_pw.max(1), exact_ph.max(1), css_w, css_h);
    }

    pub fn resize_time_axis_exact(&self, exact_pw: u32, exact_ph: u32, css_w: f64, css_h: f64) {
        self.time_axis
            .set_size_with_css(exact_pw.max(1), exact_ph.max(1), css_w, css_h);
    }

    pub fn resize_corner_stub_exact(&self, exact_pw: u32, exact_ph: u32, css_w: f64, css_h: f64) {
        utils::set_canvas_size_with_css(
            &self.corner_stub,
            exact_pw.max(1),
            exact_ph.max(1),
            css_w,
            css_h,
        );
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn create_widget_container(doc: &Document, id: &str) -> Result<HtmlDivElement, JsValue> {
    let div = doc
        .create_element("div")?
        .dyn_into::<HtmlDivElement>()
        .map_err(|_| JsValue::from_str("failed to create div"))?;
    div.set_id(id);
    Ok(div)
}

fn set_style_property_if_needed(style: &CssStyleDeclaration, property: &str, value: &str) {
    if style.get_property_value(property).ok().as_deref() != Some(value) {
        let _ = style.set_property(property, value);
    }
}
