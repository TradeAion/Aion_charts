//! Renderer trait — the abstraction layer between core logic and rendering backends.
//!
//! All renderers (WgpuRenderer, Canvas2DRenderer) implement this trait.
//! Both renderers internally call geometry_generator::generate() to get
//! the same DrawList, then render it with their respective APIs.

use crate::core::data::Bar;
use crate::core::viewport::Viewport;

/// Style configuration for the chart — colors, sizes, etc.
/// Shared between all renderers so the chart looks identical regardless of backend.
/// All dimension constants match LWC exactly.
#[derive(Debug, Clone)]
pub struct ChartStyle {
    pub bg_color: [f32; 4],
    pub bullish_color: [f32; 4],
    pub bearish_color: [f32; 4],
    pub bullish_volume_color: [f32; 4],
    pub bearish_volume_color: [f32; 4],
    pub wick_bullish_color: [f32; 4],
    pub wick_bearish_color: [f32; 4],
    pub grid_color: [f32; 4],
    /// Axis border / tick color (LWC: timeScale.borderColor / priceScale.borderColor).
    pub axis_border_color: [f32; 4],
    pub axis_text_color: [f32; 4],
    pub axis_bg_color: [f32; 4],
    /// Crosshair line color (LWC default: #9598A1).
    pub crosshair_color: [f32; 4],
    /// Crosshair label background (LWC default: #131722).
    pub crosshair_label_bg: [f32; 4],
    pub crosshair_label_text: [f32; 4],
    pub watermark_color: [f32; 4],
    /// Font family — LWC default: `-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif`.
    pub font_family: String,
    /// Layout font size in CSS px (LWC default: 11).
    pub font_size: f32,
    pub font_size_watermark: f32,
    /// Bar width as fraction of bar slot (0.0-1.0). 0.8 = 80%.
    pub bar_width_ratio: f32,

    // ── LWC price-axis renderer constants (in CSS px) ──
    /// Border width at the axis edge (LWC: 1).
    pub axis_border_size: f32,
    /// Tick line length perpendicular to axis (LWC: 5).
    pub axis_tick_length: f32,
}

impl ChartStyle {
    // ── LWC-derived computed paddings (all in CSS px) ──

    /// Price axis paddingInner: `fontSize/12 * tickLength`.
    #[inline]
    pub fn price_axis_padding_inner(&self) -> f64 {
        self.font_size as f64 / 12.0 * self.axis_tick_length as f64
    }
    /// Price axis paddingOuter: same as paddingInner in LWC.
    #[inline]
    pub fn price_axis_padding_outer(&self) -> f64 {
        self.font_size as f64 / 12.0 * self.axis_tick_length as f64
    }
    /// Price axis paddingTop/Bottom: `2.5/12 * fontSize`.
    #[inline]
    pub fn price_axis_padding_tb(&self) -> f64 {
        2.5 / 12.0 * self.font_size as f64
    }
    /// Price axis label offset (LWC Constants.LabelOffset = 5).
    #[inline]
    pub fn price_axis_label_offset(&self) -> f64 { 5.0 }

    /// Computed optimal price axis width (CSS px) for a given max text width.
    /// LWC: borderSize + tickLength + paddingInner + paddingOuter + LabelOffset + textWidth
    #[inline]
    pub fn price_axis_width(&self, max_text_width: f64) -> f64 {
        let raw = self.axis_border_size as f64
            + self.axis_tick_length as f64
            + self.price_axis_padding_inner()
            + self.price_axis_padding_outer()
            + self.price_axis_label_offset()
            + max_text_width;
        // LWC suggestPriceScaleWidth: make even
        let w = raw.ceil() as u32;
        (w + (w % 2)) as f64
    }

    /// Time axis optimal height (CSS px).
    /// LWC: borderSize + tickLength + fontSize + paddingTop + paddingBottom + labelBottomOffset
    #[inline]
    pub fn time_axis_height(&self) -> f64 {
        let fs = self.font_size as f64;
        self.axis_border_size as f64
            + self.axis_tick_length as f64
            + fs
            + self.time_axis_padding_top()
            + self.time_axis_padding_bottom()
            + self.time_axis_label_bottom_offset()
    }

    /// Time axis paddingTop: `3 * fontSize / 12`.
    #[inline]
    pub fn time_axis_padding_top(&self) -> f64 { 3.0 * self.font_size as f64 / 12.0 }
    /// Time axis paddingBottom: same.
    #[inline]
    pub fn time_axis_padding_bottom(&self) -> f64 { 3.0 * self.font_size as f64 / 12.0 }
    /// Time axis paddingHorizontal: `9 * fontSize / 12`.
    #[inline]
    pub fn time_axis_padding_horizontal(&self) -> f64 { 9.0 * self.font_size as f64 / 12.0 }
    /// Time axis labelBottomOffset: `4 * fontSize / 12`.
    #[inline]
    pub fn time_axis_label_bottom_offset(&self) -> f64 { 4.0 * self.font_size as f64 / 12.0 }

    /// Crosshair label additional padding (LWC: `2/12 * fontSize`).
    #[inline]
    pub fn crosshair_label_extra_padding(&self) -> f64 { 2.0 / 12.0 * self.font_size as f64 }

    /// Build the CSS font string for the axis: `"11px -apple-system, ..."`.
    #[inline]
    pub fn axis_font(&self, dpr: f64) -> String {
        format!("{}px {}", (self.font_size as f64 * dpr).round(), self.font_family)
    }

    /// Build bold font string for time-axis bold labels.
    #[inline]
    pub fn axis_font_bold(&self, dpr: f64) -> String {
        format!("bold {}px {}", (self.font_size as f64 * dpr).round(), self.font_family)
    }
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            bg_color: [0.067, 0.075, 0.094, 1.0],       // #111318
            bullish_color: [0.102, 0.737, 0.612, 1.0],   // #1ABC9C
            bearish_color: [0.906, 0.298, 0.235, 1.0],   // #E74C3C
            bullish_volume_color: [0.102, 0.737, 0.612, 0.35],
            bearish_volume_color: [0.906, 0.298, 0.235, 0.35],
            wick_bullish_color: [0.102, 0.737, 0.612, 0.9],
            wick_bearish_color: [0.906, 0.298, 0.235, 0.9],
            grid_color: [0.2, 0.2, 0.24, 0.4],
            axis_border_color: [0.2, 0.2, 0.24, 1.0],    // #333 solid
            axis_text_color: [0.55, 0.55, 0.6, 1.0],
            axis_bg_color: [0.09, 0.095, 0.11, 1.0],
            crosshair_color: [0.584, 0.596, 0.631, 1.0], // #9598A1  (LWC default)
            crosshair_label_bg: [0.075, 0.09, 0.133, 1.0], // #131722 (LWC default)
            crosshair_label_text: [0.9, 0.9, 0.9, 1.0],
            watermark_color: [0.15, 0.16, 0.18, 1.0],
            font_family: "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif".into(),
            font_size: 11.0,
            font_size_watermark: 48.0,
            bar_width_ratio: 0.8,
            axis_border_size: 1.0,
            axis_tick_length: 5.0,
        }
    }
}

/// Crosshair state — the position of the crosshair in logical coordinates.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrosshairState {
    pub active: bool,
    pub x: f64,
    pub y: f64,
    pub bar_index: Option<usize>,
    pub price: f64,
}

/// Information about the current render frame, passed to the renderer.
pub struct RenderContext<'a> {
    pub bars: &'a [Bar],
    pub viewport: &'a Viewport,
    pub style: &'a ChartStyle,
    pub crosshair: &'a CrosshairState,
    pub dpr: f64,
    pub logical_width: f64,
    pub logical_height: f64,
    /// Dynamic Y-axis width in CSS px (measured from text widths).
    pub y_axis_css_w: f64,
}

/// Tick mark for axis rendering.
#[derive(Debug, Clone)]
pub struct TickMark {
    pub value: f64,
    pub pixel: f64,
    pub label: String,
    pub major: bool,
}

/// The renderer-agnostic trait. Every rendering backend implements this.
pub trait Renderer {
    fn name(&self) -> &str;
    fn resize(&mut self, physical_width: u32, physical_height: u32, dpr: f64);
    fn render_frame(&mut self, ctx: &RenderContext) -> Result<(), String>;
    fn is_valid(&self) -> bool { true }
}

/// Enum wrapper so ChartEngine can hold either renderer without dyn dispatch overhead.
pub enum RendererBackend {
    Wgpu(super::wgpu_backend::WgpuRenderer),
    #[cfg(target_arch = "wasm32")]
    Canvas2D(super::canvas2d::Canvas2DRenderer),
}

impl Renderer for RendererBackend {
    fn name(&self) -> &str {
        match self {
            Self::Wgpu(r) => r.name(),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.name(),
        }
    }

    fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        match self {
            Self::Wgpu(r) => r.resize(pw, ph, dpr),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.resize(pw, ph, dpr),
        }
    }

    fn render_frame(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            Self::Wgpu(r) => r.render_frame(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.render_frame(ctx),
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Wgpu(r) => r.is_valid(),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.is_valid(),
        }
    }
}
