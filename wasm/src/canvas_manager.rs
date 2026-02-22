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

/// The pane widget has 2 canvases: chart (z0), overlay/top (z1).
pub struct PaneCanvases {
    pub chart: HtmlCanvasElement,
    pub top: HtmlCanvasElement,
}

impl PaneCanvases {
    fn new(doc: &Document) -> Result<Self, JsValue> {
        let chart = create_canvas(doc, "raycore-pane-chart", 0)?;
        let top = create_canvas(doc, "raycore-pane-top", 1)?;
        Ok(Self { chart, top })
    }

    pub fn set_size(&self, pw: u32, ph: u32) {
        for c in [&self.chart, &self.top] {
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

        let corner_stub = create_canvas(&doc, "raycore-corner-stub-canvas", 0)?;
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
        let _ = self.grid_wrapper.style().set_property(
            "grid-template-columns",
            &format!("1fr {}px", price_axis_css_w.round()),
        );
        let _ = self.grid_wrapper.style().set_property(
            "grid-template-rows",
            &format!("1fr {}px", time_axis_css_h.round()),
        );
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
        let _ = self.grid_wrapper.style().set_property(
            "grid-template-columns",
            &format!("1fr {}px", price_axis_css_w.round()),
        );

        // Build rows: "1fr [1px Npx]... Mpx"
        let mut rows = String::from("1fr ");
        for h in subpane_heights {
            rows.push_str(&format!("1px {}px ", h.max(30.0)));
        }
        rows.push_str(&format!("{}px", time_axis_css_h.round()));

        let _ = self
            .grid_wrapper
            .style()
            .set_property("grid-template-rows", &rows);

        // Move time axis + corner stub to the correct last row
        let time_row = 2 + subpane_heights.len() * 2;
        let _ = self
            .time_axis_container
            .style()
            .set_property("grid-row", &time_row.to_string());
        let _ = self
            .corner_stub_container
            .style()
            .set_property("grid-row", &time_row.to_string());
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

        // Corner stub canvas
        let (sw, sh) = self.corner_stub_css_size();
        let spw = (sw * dpr).round() as u32;
        let sph = (sh * dpr).round() as u32;
        self.corner_stub.set_width(spw.max(1));
        self.corner_stub.set_height(sph.max(1));
    }

    /// Resize a specific widget's canvases using exact device-pixel sizes
    /// reported by `ResizeObserver` with `device-pixel-content-box`.
    /// This avoids the ±1px rounding error from `round(css * dpr)`.
    pub fn resize_pane_exact(&self, exact_pw: u32, exact_ph: u32) {
        self.pane.set_size(exact_pw.max(1), exact_ph.max(1));
    }

    pub fn resize_price_axis_exact(&self, exact_pw: u32, exact_ph: u32) {
        self.price_axis.set_size(exact_pw.max(1), exact_ph.max(1));
    }

    pub fn resize_time_axis_exact(&self, exact_pw: u32, exact_ph: u32) {
        self.time_axis.set_size(exact_pw.max(1), exact_ph.max(1));
    }

    pub fn resize_corner_stub_exact(&self, exact_pw: u32, exact_ph: u32) {
        self.corner_stub.set_width(exact_pw.max(1));
        self.corner_stub.set_height(exact_ph.max(1));
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn create_canvas(doc: &Document, id: &str, z_index: u32) -> Result<HtmlCanvasElement, JsValue> {
    let canvas = doc
        .create_element("canvas")?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("failed to create canvas"))?;

    canvas.set_id(id);
    canvas.style().set_css_text(&format!(
        "position:absolute;top:0;left:0;width:100%;height:100%;display:block;z-index:{};pointer-events:none;",
        z_index
    ));

    Ok(canvas)
}

fn create_widget_container(doc: &Document, id: &str) -> Result<HtmlDivElement, JsValue> {
    let div = doc
        .create_element("div")?
        .dyn_into::<HtmlDivElement>()
        .map_err(|_| JsValue::from_str("failed to create div"))?;
    div.set_id(id);
    Ok(div)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Multi-Pane Support
// ═══════════════════════════════════════════════════════════════════════════════

/// A single pane widget with its own chart canvas, overlay, and price axis.
pub struct PaneWidget {
    pub id: u32,
    pub container: HtmlDivElement,
    pub pane: PaneCanvases,
    pub price_axis_container: HtmlDivElement,
    pub price_axis: CanvasPair,
    /// Height in CSS pixels.
    pub height_css: f64,
}

impl PaneWidget {
    fn new(doc: &Document, id: u32) -> Result<Self, JsValue> {
        let id_str = format!("raycore-pane-{}", id);

        // Create pane container (flex row: chart area + price axis)
        let container = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create pane container"))?;
        container.set_id(&id_str);
        container.style().set_css_text(
            "display:flex;flex-direction:row;\
             position:relative;overflow:hidden;\
             min-height:50px;flex-shrink:0;",
        );

        // Chart area (takes remaining space)
        let chart_container = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create chart container"))?;
        chart_container.set_id(&format!("{}-chart", id_str));
        chart_container.style().set_css_text(
            "flex:1;position:relative;overflow:hidden;\
             min-width:0;cursor:crosshair;\
             touch-action:none;-webkit-user-select:none;user-select:none;",
        );
        container.append_child(&chart_container)?;

        // Create pane canvases
        let pane_chart = create_canvas(doc, &format!("{}-chart-canvas", id_str), 0)?;
        let pane_top = create_canvas(doc, &format!("{}-overlay", id_str), 1)?;
        chart_container.append_child(&pane_chart)?;
        chart_container.append_child(&pane_top)?;

        let pane = PaneCanvases {
            chart: pane_chart,
            top: pane_top,
        };

        // Price axis container
        let price_axis_container = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create price axis container"))?;
        price_axis_container.set_id(&format!("{}-price-axis", id_str));
        price_axis_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             min-width:50px;cursor:ns-resize;\
             touch-action:none;-webkit-user-select:none;user-select:none;",
        );
        container.append_child(&price_axis_container)?;

        // Price axis canvases
        let price_axis = CanvasPair::new(doc, &format!("{}-priceaxis", id_str))?;
        price_axis_container.append_child(&price_axis.base)?;
        price_axis_container.append_child(&price_axis.top)?;

        Ok(Self {
            id,
            container,
            pane,
            price_axis_container,
            price_axis,
            height_css: 200.0, // default
        })
    }

    /// Set the height of this pane in CSS pixels.
    pub fn set_height(&mut self, height_css: f64) {
        self.height_css = height_css;
        let _ = self
            .container
            .style()
            .set_property("height", &format!("{}px", height_css));
    }

    /// Set the price axis width in CSS pixels.
    pub fn set_price_axis_width(&self, width_css: f64) {
        let _ = self
            .price_axis_container
            .style()
            .set_property("width", &format!("{}px", width_css));
    }

    /// Get the chart area CSS size.
    pub fn chart_css_size(&self) -> (f64, f64) {
        // The chart is the pane container minus the price axis
        let total_w = self.container.client_width() as f64;
        let axis_w = self.price_axis_container.client_width() as f64;
        let h = self.container.client_height() as f64;
        ((total_w - axis_w).max(0.0), h)
    }

    /// Resize canvases to match container sizes.
    pub fn resize_canvases(&self, dpr: f64) {
        let (cw, ch) = self.chart_css_size();
        let cpw = (cw * dpr).round() as u32;
        let cph = (ch * dpr).round() as u32;
        self.pane.set_size(cpw.max(1), cph.max(1));

        let aw = self.price_axis_container.client_width() as f64;
        let ah = self.price_axis_container.client_height() as f64;
        let apw = (aw * dpr).round() as u32;
        let aph = (ah * dpr).round() as u32;
        self.price_axis.set_size(apw.max(1), aph.max(1));
    }
}

/// A separator between panes (draggable to resize).
pub struct PaneSeparator {
    pub index: usize,
    pub element: HtmlDivElement,
}

impl PaneSeparator {
    fn new(doc: &Document, index: usize) -> Result<Self, JsValue> {
        let element = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create separator"))?;
        element.set_id(&format!("raycore-separator-{}", index));
        element.style().set_css_text(
            "height:4px;background:#2a2a30;cursor:ns-resize;\
             flex-shrink:0;position:relative;z-index:10;\
             touch-action:none;-webkit-user-select:none;user-select:none;",
        );

        // Add hover effect
        element.set_attribute("onmouseenter", "this.style.background='#4a9eff'")?;
        element.set_attribute("onmouseleave", "this.style.background='#2a2a30'")?;

        Ok(Self { index, element })
    }
}

/// Multi-pane layout manager.
/// Supports N panes with independent price scales, draggable separators.
pub struct MultiPaneLayout {
    /// The outer container provided by the user.
    pub container: HtmlElement,
    /// Flex wrapper containing all panes + separators + time axis.
    pub flex_wrapper: HtmlDivElement,
    /// The pane widgets (index 0 = main pane).
    pub panes: Vec<PaneWidget>,
    /// Separators between panes.
    pub separators: Vec<PaneSeparator>,
    /// Time axis (shared by all panes).
    pub time_axis_row: HtmlDivElement,
    pub time_axis_container: HtmlDivElement,
    pub time_axis: CanvasPair,
    /// Corner stub.
    pub corner_stub_container: HtmlDivElement,
    pub corner_stub: HtmlCanvasElement,
    /// Next pane ID.
    next_pane_id: u32,
}

impl MultiPaneLayout {
    /// Create a new multi-pane layout with just the main pane.
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

        // Create flex wrapper (column: panes + separators + time axis row)
        let flex_wrapper = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create flex wrapper"))?;
        flex_wrapper.set_id("raycore-multi-pane-wrapper");
        flex_wrapper.style().set_css_text(
            "display:flex;flex-direction:column;\
             width:100%;height:100%;\
             position:absolute;top:0;left:0;\
             overflow:hidden;\
             touch-action:none;-webkit-user-select:none;user-select:none;",
        );
        container.append_child(&flex_wrapper)?;

        // Create main pane (ID 0)
        let main_pane = PaneWidget::new(&doc, 0)?;
        main_pane.container.style().set_property("flex", "1")?; // Take remaining space
        flex_wrapper.append_child(&main_pane.container)?;

        // Time axis row (flex row: time axis + corner stub)
        let time_axis_row = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create time axis row"))?;
        time_axis_row.set_id("raycore-time-axis-row");
        time_axis_row.style().set_css_text(
            "display:flex;flex-direction:row;\
             flex-shrink:0;height:auto;",
        );
        flex_wrapper.append_child(&time_axis_row)?;

        // Time axis container
        let time_axis_container = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create time axis container"))?;
        time_axis_container.set_id("raycore-time-axis");
        time_axis_container.style().set_css_text(
            "flex:1;position:relative;overflow:hidden;\
             min-width:0;cursor:ew-resize;\
             touch-action:none;-webkit-user-select:none;user-select:none;",
        );
        time_axis_row.append_child(&time_axis_container)?;

        let time_axis = CanvasPair::new(&doc, "raycore-timeaxis")?;
        time_axis_container.append_child(&time_axis.base)?;
        time_axis_container.append_child(&time_axis.top)?;

        // Corner stub
        let corner_stub_container = doc
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()
            .map_err(|_| JsValue::from_str("failed to create corner stub container"))?;
        corner_stub_container.set_id("raycore-corner-stub");
        corner_stub_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             min-width:50px;cursor:default;",
        );
        time_axis_row.append_child(&corner_stub_container)?;

        let corner_stub = create_canvas(&doc, "raycore-corner-stub-canvas", 0)?;
        corner_stub_container.append_child(&corner_stub)?;

        Ok(Self {
            container,
            flex_wrapper,
            panes: vec![main_pane],
            separators: Vec::new(),
            time_axis_row,
            time_axis_container,
            time_axis,
            corner_stub_container,
            corner_stub,
            next_pane_id: 1,
        })
    }

    /// Add a new sub-pane below existing panes. Returns the pane ID.
    pub fn add_pane(&mut self, height_css: f64) -> Result<u32, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let doc = window
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;

        let id = self.next_pane_id;
        self.next_pane_id += 1;

        // Create separator before the new pane
        let sep_idx = self.separators.len();
        let separator = PaneSeparator::new(&doc, sep_idx)?;

        // Insert separator before time axis row
        self.flex_wrapper
            .insert_before(&separator.element, Some(&self.time_axis_row))?;
        self.separators.push(separator);

        // Create new pane
        let mut pane = PaneWidget::new(&doc, id)?;
        pane.set_height(height_css);

        // Insert pane before time axis row
        self.flex_wrapper
            .insert_before(&pane.container, Some(&self.time_axis_row))?;
        self.panes.push(pane);

        // Update main pane to not use flex:1 anymore, use explicit height
        if self.panes.len() > 1 {
            let _ = self.panes[0].container.style().set_property("flex", "none");
            // Main pane gets remaining height
            self.recompute_heights();
        }

        Ok(id)
    }

    /// Remove a sub-pane by ID. Cannot remove main pane (ID 0).
    pub fn remove_pane(&mut self, id: u32) -> bool {
        if id == 0 {
            return false; // Cannot remove main pane
        }

        if let Some(pos) = self.panes.iter().position(|p| p.id == id) {
            // Remove pane
            let pane = self.panes.remove(pos);
            let _ = pane.container.remove();

            // Remove corresponding separator (if not the first pane)
            if pos > 0 && !self.separators.is_empty() {
                let sep_idx = pos - 1;
                if sep_idx < self.separators.len() {
                    let sep = self.separators.remove(sep_idx);
                    let _ = sep.element.remove();
                }
            }

            // If back to single pane, restore flex:1
            if self.panes.len() == 1 {
                let _ = self.panes[0].container.style().set_property("flex", "1");
            } else {
                self.recompute_heights();
            }

            true
        } else {
            false
        }
    }

    /// Get the main pane.
    pub fn main_pane(&self) -> &PaneWidget {
        &self.panes[0]
    }

    /// Get a pane by ID.
    pub fn get_pane(&self, id: u32) -> Option<&PaneWidget> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Get a mutable pane by ID.
    pub fn get_pane_mut(&mut self, id: u32) -> Option<&mut PaneWidget> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// Number of panes.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Recompute pane heights when separators are dragged.
    pub fn recompute_heights(&mut self) {
        // Get total available height
        let container_h = self.container.client_height() as f64;
        let time_axis_h = self.time_axis_container.client_height() as f64;
        let separator_h = 4.0 * self.separators.len() as f64;
        let available = container_h - time_axis_h - separator_h;

        if available <= 0.0 || self.panes.is_empty() {
            return;
        }

        // Simple equal distribution for now (can add stretch factors later)
        let per_pane = available / self.panes.len() as f64;
        for pane in &mut self.panes {
            pane.set_height(per_pane.max(50.0));
        }
    }

    /// Set the time axis height in CSS pixels.
    pub fn set_time_axis_height(&self, height_css: f64) {
        let _ = self
            .time_axis_row
            .style()
            .set_property("height", &format!("{}px", height_css));
    }

    /// Set the price axis width for all panes (and corner stub).
    pub fn set_price_axis_width(&self, width_css: f64) {
        for pane in &self.panes {
            pane.set_price_axis_width(width_css);
        }
        let _ = self
            .corner_stub_container
            .style()
            .set_property("width", &format!("{}px", width_css));
    }

    /// Resize all canvases at the given DPR.
    pub fn resize_all_canvases(&self, dpr: f64) {
        // Pane canvases
        for pane in &self.panes {
            pane.resize_canvases(dpr);
        }

        // Time axis canvases
        let tw = self.time_axis_container.client_width() as f64;
        let th = self.time_axis_container.client_height() as f64;
        let tpw = (tw * dpr).round() as u32;
        let tph = (th * dpr).round() as u32;
        self.time_axis.set_size(tpw.max(1), tph.max(1));

        // Corner stub
        let sw = self.corner_stub_container.client_width() as f64;
        let sh = self.corner_stub_container.client_height() as f64;
        let spw = (sw * dpr).round() as u32;
        let sph = (sh * dpr).round() as u32;
        self.corner_stub.set_width(spw.max(1));
        self.corner_stub.set_height(sph.max(1));
    }

    /// Get time axis CSS size.
    pub fn time_axis_css_size(&self) -> (f64, f64) {
        (
            self.time_axis_container.client_width() as f64,
            self.time_axis_container.client_height() as f64,
        )
    }

    /// Get corner stub CSS size.
    pub fn corner_stub_css_size(&self) -> (f64, f64) {
        (
            self.corner_stub_container.client_width() as f64,
            self.corner_stub_container.client_height() as f64,
        )
    }
}
