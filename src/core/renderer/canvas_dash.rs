//! Canvas2D line-dash helpers shared by overlay/subpane renderers.

#![cfg(target_arch = "wasm32")]

use crate::core::series::LineStyle;
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;

/// Apply the LWC-style dash pattern for a line style and physical line width.
#[inline]
pub fn set_canvas_line_dash(ctx: &CanvasRenderingContext2d, style: LineStyle, line_width: f64) {
    let (dash, gap) = style.dash_pattern(line_width.max(1.0));
    if dash > 0.0 && gap > 0.0 {
        let _ = ctx.set_line_dash(&js_sys::Array::of2(
            &JsValue::from(dash),
            &JsValue::from(gap),
        ));
    } else {
        clear_canvas_line_dash(ctx);
    }
}

/// Clear Canvas2D dash state.
#[inline]
pub fn clear_canvas_line_dash(ctx: &CanvasRenderingContext2d) {
    let _ = ctx.set_line_dash(&js_sys::Array::new());
}
