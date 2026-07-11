//! Canvas2D-backed text measurement and rasterization.
//!
//! Labels are rasterized white-on-transparent by the browser's own font stack — the exact
//! same glyphs lightweight-charts draws with `fillText` — then tinted in the shader. This
//! is the "pixel-identical by construction" text path from docs/ARCHITECTURE.md §10 risks.

use std::collections::HashMap;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// LWC default font stack (`helpers/make-font.ts` / layout defaults).
const FONT_FAMILY: &str =
    "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif";
pub const FONT_SIZE: f64 = 12.0;

/// Horizontal padding baked into each rasterized label (media px) so antialiasing at the
/// glyph edges isn't clipped; subtracted again at draw time.
const RASTER_PAD: f64 = 1.0;

pub struct TextPainter {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    width_cache: HashMap<String, f64>,
}

impl TextPainter {
    pub fn new() -> Result<Self, JsValue> {
        let document = web_sys::window()
            .ok_or_else(|| JsValue::from_str("no window"))?
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: HtmlCanvasElement =
            document.create_element("canvas")?.dyn_into::<HtmlCanvasElement>()?;
        canvas.set_width(512);
        canvas.set_height(64);
        let ctx = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("no 2d context"))?
            .dyn_into::<CanvasRenderingContext2d>()?;
        Ok(Self { canvas, ctx, width_cache: HashMap::new() })
    }

    fn font(size: f64) -> String {
        format!("{size}px {FONT_FAMILY}")
    }

    /// Text width in media px at the layout font size (mirrors `TextWidthCache`).
    pub fn measure(&mut self, text: &str) -> f64 {
        if let Some(&w) = self.width_cache.get(text) {
            return w;
        }
        self.ctx.set_font(&Self::font(FONT_SIZE));
        let w = self.ctx.measure_text(text).map(|m| m.width()).unwrap_or(0.0);
        if self.width_cache.len() > 512 {
            self.width_cache.clear();
        }
        self.width_cache.insert(text.to_string(), w);
        w
    }

    /// Rasterizes `text` white-on-transparent at `dpr` scale.
    /// Returns (pixels RGBA8, width_px, height_px). The glyph baseline is vertically
    /// centered (`textBaseline: middle`), so callers center the returned bitmap on the
    /// target y coordinate — same convention as LWC's axis label drawing.
    pub fn rasterize(&mut self, text: &str, dpr: f64) -> Result<(Vec<u8>, u32, u32), JsValue> {
        let media_width = self.measure(text);
        let w = ((media_width + RASTER_PAD * 2.0) * dpr).ceil().max(1.0) as u32;
        // 1.5x font size covers ascenders + descenders for the label character set
        let h = (FONT_SIZE * 1.5 * dpr).ceil() as u32;

        if self.canvas.width() < w || self.canvas.height() < h {
            self.canvas.set_width(w.next_power_of_two());
            self.canvas.set_height(h.next_power_of_two());
        }

        self.ctx.clear_rect(0.0, 0.0, self.canvas.width() as f64, self.canvas.height() as f64);
        // canvas state persists (no resize reset unless we resized above), set everything
        self.ctx.set_font(&Self::font(FONT_SIZE * dpr));
        self.ctx.set_text_baseline("middle");
        self.ctx.set_fill_style_str("#FFFFFF");
        self.ctx
            .fill_text(text, RASTER_PAD * dpr, (h / 2) as f64)
            .map_err(|e| JsValue::from(e))?;

        let image = self.ctx.get_image_data(0.0, 0.0, w as f64, h as f64)?;
        Ok((image.data().to_vec(), w, h))
    }

    /// Draw-time x offset compensating the baked-in raster padding, in bitmap px.
    pub fn pad_px(dpr: f64) -> f64 {
        RASTER_PAD * dpr
    }
}
