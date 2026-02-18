//! CanvasManager — creates and manages the 3-canvas stack via web-sys.
//!
//! Given a container div ID, internally creates:
//!   - grid canvas    (z-index:0) — background grid lines
//!   - chart canvas   (z-index:1) — candles, volume (WebGPU or Canvas2D)
//!   - overlay canvas (z-index:2) — axes, crosshair, watermark
//!
//! No JS needed to create or replace canvases.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlCanvasElement, HtmlElement};

/// Owns the 3-canvas stack inside a container div.
pub struct CanvasManager {
    container: HtmlElement,
    pub grid_canvas: HtmlCanvasElement,
    pub chart_canvas: HtmlCanvasElement,
    pub overlay_canvas: HtmlCanvasElement,
}

impl CanvasManager {
    /// Create the 3-canvas stack inside the given container div.
    /// Clears the container first.
    pub fn new(container_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;

        let container = document
            .get_element_by_id(container_id)
            .ok_or_else(|| JsValue::from_str(&format!("container '{}' not found", container_id)))?
            .dyn_into::<HtmlElement>()
            .map_err(|_| JsValue::from_str("container is not an HTMLElement"))?;

        // Ensure container has position:relative for absolute canvas stacking
        let style = container.style();
        let pos = style.get_property_value("position").unwrap_or_default();
        if pos.is_empty() || pos == "static" {
            style.set_property("position", "relative")?;
        }
        style.set_property("overflow", "hidden")?;

        // Clear existing children
        container.set_inner_html("");

        let grid_canvas = Self::create_canvas(&document, "raycore-grid", 0)?;
        let chart_canvas = Self::create_canvas(&document, "raycore-chart", 1)?;
        let overlay_canvas = Self::create_canvas(&document, "raycore-overlay", 2)?;

        container.append_child(&grid_canvas)?;
        container.append_child(&chart_canvas)?;
        container.append_child(&overlay_canvas)?;

        Ok(Self {
            container,
            grid_canvas,
            chart_canvas,
            overlay_canvas,
        })
    }

    fn create_canvas(
        document: &Document,
        id: &str,
        z_index: u32,
    ) -> Result<HtmlCanvasElement, JsValue> {
        let canvas = document
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

    /// Get container CSS dimensions.
    pub fn css_size(&self) -> (f64, f64) {
        (
            self.container.client_width() as f64,
            self.container.client_height() as f64,
        )
    }

    /// Set all canvases to the given physical pixel dimensions.
    pub fn set_physical_size(&self, pw: u32, ph: u32) {
        for c in [&self.grid_canvas, &self.chart_canvas, &self.overlay_canvas] {
            c.set_width(pw.max(1));
            c.set_height(ph.max(1));
        }
    }
}
