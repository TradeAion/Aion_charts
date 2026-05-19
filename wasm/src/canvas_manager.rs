//! WidgetLayout — compatibility-style DOM layout with separate widget containers.
//!
//! Creates a CSS-grid layout matching the reference implementation's table structure:
//!
//!   ┌─────────────────────┬──────────────┐
//!   │   Pane (chart area) │  Price Axis  │
//!   │  [grid canvas]      │  [base canvas│
//!   │  [chart canvas]     │   top canvas]│
//!   │  [overlay canvas]   │              │
//!   ├────────────────────────────────────┤
//!   │         Time Axis (full width)     │
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
/// Matches the reference implementation's canvasBinding + topCanvasBinding pattern.
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
        let chart = utils::create_canvas(doc, "aion_charts-pane-chart", 0)?;
        let top = utils::create_canvas(doc, "aion_charts-pane-top", 1)?;
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

    // ── Canvases per widget ──
    pub pane: PaneCanvases,
    pub price_axis: CanvasPair,
    pub time_axis: CanvasPair,
}

impl WidgetLayout {
    /// Create the full widget layout inside the given container div.
    /// Mirrors the reference implementation's ChartWidget._init() DOM creation.
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
        // reference implementation uses an HTML table; we use CSS grid for the same layout:
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
        let pane_container = create_widget_container(&doc, "aion_charts-pane")?;
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

        // ── Price axis container — right side, spans main pane only ──
        let price_axis_container = create_widget_container(&doc, "aion_charts-price-axis")?;
        price_axis_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:2;grid-row:1;\
             min-width:0;min-height:0;\
             z-index:2;\
             cursor:ns-resize;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
        );
        grid_wrapper.append_child(&price_axis_container)?;

        let price_axis = CanvasPair::new(&doc, "aion_charts-priceaxis")?;
        price_axis_container.append_child(&price_axis.base)?;
        price_axis_container.append_child(&price_axis.top)?;

        // ── Time axis container — bottom row, full width (including under right axis) ──
        let time_axis_container = create_widget_container(&doc, "aion_charts-time-axis")?;
        time_axis_container.style().set_css_text(
            "position:relative;overflow:hidden;\
             grid-column:1/3;grid-row:2;\
             min-width:0;min-height:0;\
             z-index:3;\
             cursor:ew-resize;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
        );
        grid_wrapper.append_child(&time_axis_container)?;

        let time_axis = CanvasPair::new(&doc, "aion_charts-timeaxis")?;
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
        element_css_size(&self.container)
    }

    /// Update the grid sizing based on computed axis dimensions (CSS px).
    /// Called when axis widths/heights change (e.g. after measuring text).
    pub fn update_axis_sizes(&self, price_axis_css_w: f64, time_axis_css_h: f64) {
        let cols = format!("1fr {}px", price_axis_css_w.round());
        let rows = format!("1fr {}px", time_axis_css_h.round());
        let style = self.grid_wrapper.style();
        set_style_property_if_needed(&style, "grid-template-columns", &cols);
        set_style_property_if_needed(&style, "grid-template-rows", &rows);

        // Time axis is row 2 and spans full width (under both pane and price axis).
        let time_axis_style = self.time_axis_container.style();
        set_style_property_if_needed(&time_axis_style, "grid-row", "2");
        set_style_property_if_needed(&time_axis_style, "grid-column", "1/3");

        // Price axis spans only row 1 (main pane) — matches the pane height,
        // not extending into the time axis row. This matches reference behaviour
        // where the right price scale covers only the chart area.
        let price_axis_style = self.price_axis_container.style();
        set_style_property_if_needed(&price_axis_style, "grid-row", "1");
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

        // Move time axis to the last row and keep it full width.
        let time_row = 2 + subpane_heights.len() * 2;
        let time_row_str = time_row.to_string();
        let time_axis_style = self.time_axis_container.style();
        set_style_property_if_needed(&time_axis_style, "grid-row", &time_row_str);
        set_style_property_if_needed(&time_axis_style, "grid-column", "1/3");

        let price_axis_style = self.price_axis_container.style();
        set_style_property_if_needed(&price_axis_style, "grid-row", "1");
    }

    /// Get the pane's actual CSS size (chart area only).
    pub fn pane_css_size(&self) -> (f64, f64) {
        element_css_size(&self.pane_container)
    }

    /// Get the price axis container's CSS size.
    pub fn price_axis_css_size(&self) -> (f64, f64) {
        element_css_size(&self.price_axis_container)
    }

    /// Get the time axis container's CSS size.
    pub fn time_axis_css_size(&self) -> (f64, f64) {
        element_css_size(&self.time_axis_container)
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

    /// Snap all canvas layers onto device-pixel boundaries.
    ///
    /// A host page can place the chart at a fractional CSS position. Even with
    /// exact bitmap sizes, that makes the browser composite the canvas between
    /// physical pixels and softens otherwise integer-aligned candle geometry.
    /// Use `left/top`, not CSS transforms: transforms can promote the canvas
    /// to a filtered texture and reintroduce the very blur this avoids.
    pub fn snap_canvases_to_device_pixels(&self, dpr: f64) -> bool {
        if dpr <= 0.0 || !dpr.is_finite() {
            return false;
        }
        let mut changed = false;
        for canvas in [
            &self.pane.chart,
            &self.pane.top,
            &self.price_axis.base,
            &self.price_axis.top,
            &self.time_axis.base,
            &self.time_axis.top,
        ] {
            changed |= snap_canvas_to_device_pixels(canvas, dpr);
        }
        changed
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

fn element_css_size<T: AsRef<web_sys::Element>>(element: &T) -> (f64, f64) {
    let rect = element.as_ref().get_bounding_client_rect();
    (rect.width(), rect.height())
}

fn set_style_property_if_needed(style: &CssStyleDeclaration, property: &str, value: &str) -> bool {
    if style.get_property_value(property).ok().as_deref() != Some(value) {
        let _ = style.set_property(property, value);
        true
    } else {
        false
    }
}

fn snap_canvas_to_device_pixels(canvas: &HtmlCanvasElement, dpr: f64) -> bool {
    let rect = canvas.get_bounding_client_rect();
    let physical_left = rect.left() * dpr;
    let physical_top = rect.top() * dpr;
    let style = canvas.style();
    let mut changed = set_style_property_if_needed(&style, "transform", "none");
    let offset_x = parse_css_px(&style.get_property_value("left").unwrap_or_default())
        + (physical_left.round() - physical_left) / dpr;
    let offset_y = parse_css_px(&style.get_property_value("top").unwrap_or_default())
        + (physical_top.round() - physical_top) / dpr;
    if offset_x.abs() < 0.000_001 && offset_y.abs() < 0.000_001 {
        changed |= set_style_property_if_needed(&style, "left", "0px");
        changed |= set_style_property_if_needed(&style, "top", "0px");
    } else {
        changed |= set_style_property_if_needed(&style, "left", &format!("{offset_x:.6}px"));
        changed |= set_style_property_if_needed(&style, "top", &format!("{offset_y:.6}px"));
    }
    changed
}

fn parse_css_px(value: &str) -> f64 {
    value.trim_end_matches("px").parse::<f64>().unwrap_or(0.0)
}
