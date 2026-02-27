//! Shared WASM utility helpers — used by canvas_manager, subpane, render_frame, workspace.

use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlCanvasElement};

/// Convert an `[f32; 4]` RGBA color (0.0–1.0) to a CSS `rgba(...)` string.
pub fn rgba_css(c: &[f32; 4]) -> String {
    let r = (c[0].clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (c[1].clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (c[2].clamp(0.0, 1.0) * 255.0).round() as u8;
    let a = c[3].clamp(0.0, 1.0);
    format!("rgba({r},{g},{b},{a})")
}

/// Create an absolutely-positioned canvas element for overlay stacking.
pub fn create_canvas(
    doc: &Document,
    id: &str,
    z_index: u32,
) -> Result<HtmlCanvasElement, wasm_bindgen::JsValue> {
    let canvas = doc
        .create_element("canvas")?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| wasm_bindgen::JsValue::from_str("failed to create canvas"))?;
    canvas.set_id(id);
    canvas.style().set_css_text(&format!(
        "position:absolute;top:0;left:0;display:block;z-index:{z_index};\
         pointer-events:none;image-rendering:pixelated;image-rendering:crisp-edges;"
    ));
    Ok(canvas)
}

/// Set canvas bitmap + CSS size, skipping no-op mutations to avoid layout thrash.
pub fn set_canvas_size_with_css(
    canvas: &HtmlCanvasElement,
    pw: u32,
    ph: u32,
    css_w: f64,
    css_h: f64,
) {
    if canvas.width() != pw {
        canvas.set_width(pw);
    }
    if canvas.height() != ph {
        canvas.set_height(ph);
    }
    let style = canvas.style();
    let css_w_px = format!("{}px", css_w);
    if style.get_property_value("width").ok().as_deref() != Some(css_w_px.as_str()) {
        let _ = style.set_property("width", &css_w_px);
    }
    let css_h_px = format!("{}px", css_h);
    if style.get_property_value("height").ok().as_deref() != Some(css_h_px.as_str()) {
        let _ = style.set_property("height", &css_h_px);
    }
}
