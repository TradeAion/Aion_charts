//! Renderer trait — the abstraction layer between core logic and rendering backends.
//!
//! All renderers (WgpuRenderer, Canvas2DRenderer) implement this trait.
//! Both renderers internally call geometry_generator::generate() to get
//! the same DrawList, then render it with their respective APIs.

use crate::core::data::Bar;
use crate::core::viewport::Viewport;

/// Style configuration for the chart — colors, sizes, etc.
/// Shared between all renderers so the chart looks identical regardless of backend.
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
    pub axis_text_color: [f32; 4],
    pub axis_bg_color: [f32; 4],
    pub crosshair_color: [f32; 4],
    pub crosshair_label_bg: [f32; 4],
    pub crosshair_label_text: [f32; 4],
    pub watermark_color: [f32; 4],
    pub font_family: String,
    pub font_size_axis: f32,
    pub font_size_watermark: f32,
    /// Bar width as fraction of bar slot (0.0-1.0). 0.8 = 80%.
    pub bar_width_ratio: f32,
    /// Y-axis width in logical pixels.
    pub y_axis_width: f32,
    /// X-axis height in logical pixels.
    pub x_axis_height: f32,
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
            axis_text_color: [0.55, 0.55, 0.6, 1.0],
            axis_bg_color: [0.09, 0.095, 0.11, 1.0],
            crosshair_color: [0.5, 0.5, 0.55, 0.6],
            crosshair_label_bg: [0.2, 0.21, 0.24, 0.95],
            crosshair_label_text: [0.9, 0.9, 0.9, 1.0],
            watermark_color: [0.15, 0.16, 0.18, 1.0],
            font_family: "monospace".into(),
            font_size_axis: 11.0,
            font_size_watermark: 48.0,
            bar_width_ratio: 0.8,
            y_axis_width: 70.0,
            x_axis_height: 28.0,
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
