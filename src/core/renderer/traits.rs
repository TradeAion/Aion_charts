//! Renderer trait — the abstraction layer between core logic and rendering backends.
//!
//! The `ChartRenderer` trait splits rendering into discrete phases:
//! candles, volume, grid, lines, text, crosshair — each as a separate pass.
//!
//! Production backends on WASM are WebGPU and Canvas2D. A no-op backend exists
//! for native compilation (benchmarks, tests).

use crate::core::data::BarArray;
use crate::core::drawings::types::DrawingGeometry;
use crate::core::indicators::render::types::DrawInstruction;
use crate::core::series::{LineStyle, SeriesCollection};
use crate::core::viewport::Viewport;

/// Crosshair line options (LWC-style) for vertical/horizontal lines.
#[derive(Debug, Clone, Copy)]
pub struct CrosshairLineStyle {
    /// Line color [R, G, B, A].
    pub color: [f32; 4],
    /// Line width in CSS px.
    pub width: f64,
    /// Dash style.
    pub style: LineStyle,
    /// Whether the line itself is rendered.
    pub visible: bool,
    /// Whether the corresponding axis label is rendered.
    pub label_visible: bool,
    /// Axis label background color.
    pub label_bg_color: [f32; 4],
}

/// Main-series live price line options (LWC-style series price line).
#[derive(Debug, Clone, Copy)]
pub struct LastPriceLineStyle {
    /// Whether the live price line is rendered.
    pub visible: bool,
    /// Line width in CSS px.
    pub width: f64,
    /// Dash style.
    pub style: LineStyle,
    /// Whether the live price label on price axis is rendered.
    pub label_visible: bool,
}

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
    /// Crosshair vertical line options (LWC `crosshair.vertLine`).
    pub crosshair_vert_line: CrosshairLineStyle,
    /// Crosshair horizontal line options (LWC `crosshair.horzLine`).
    pub crosshair_horz_line: CrosshairLineStyle,
    /// Shared crosshair label text color.
    pub crosshair_label_text: [f32; 4],
    /// Live price line options for main/overlay series.
    pub last_price_line: LastPriceLineStyle,
    /// Font family — LWC default: `-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif`.
    pub font_family: String,
    /// Layout font size in CSS px (LWC default: 12).
    pub font_size: f32,
    /// Bar width as fraction of bar slot (0.0-1.0). 0.8 = 80%.
    pub bar_width_ratio: f32,

    // ── LWC price-axis renderer constants (in CSS px) ──
    /// Border width at the axis edge (LWC: 1).
    pub axis_border_size: f32,
    /// Tick line length perpendicular to axis (LWC: 5).
    pub axis_tick_length: f32,
    /// Price-scale tick mark density (LWC: `priceScale.tickMarkDensity`, default 2.5).
    pub price_scale_tick_mark_density: f32,
    /// Whether to draw axis tick marks (the small perpendicular lines).
    /// When false, layout stays the same but tick lines are not rendered.
    pub axis_ticks_visible: bool,
    /// Whether to draw the axis border line.
    /// When false, layout stays the same but the border line is not rendered.
    pub axis_border_visible: bool,
}

impl ChartStyle {
    // ── LWC-derived computed paddings (all in CSS px) ──

    /// Compact axis padding target in CSS px.
    /// Kept as a utility for future use; current axis paddings use LWC formulas.
    #[inline]
    #[allow(dead_code)]
    fn compact_axis_padding_css(&self) -> f64 {
        ((2.5 / 12.0) * self.font_size as f64).clamp(2.0, 3.0)
    }

    /// Price axis left padding (inside edge to text).
    /// LWC: `paddingInner = fontSize / 12 * tickLength` where tickLength = 5.
    #[inline]
    pub fn price_axis_padding_inner(&self) -> f64 {
        self.font_size as f64 / 12.0 * self.axis_tick_length as f64
    }
    /// Price axis right padding (text to outer edge).
    /// LWC: `paddingOuter = fontSize / 12 * tickLength`.
    #[inline]
    pub fn price_axis_padding_outer(&self) -> f64 {
        self.font_size as f64 / 12.0 * self.axis_tick_length as f64
    }
    /// Price axis paddingTop/Bottom: `2.5/12 * fontSize`.
    #[inline]
    pub fn price_axis_padding_tb(&self) -> f64 {
        2.5 / 12.0 * self.font_size as f64
    }
    /// Price axis label offset.
    /// LWC: `Constants.LabelOffset = 5` — extra spacing after text in optimal width.
    #[inline]
    pub fn price_axis_label_offset(&self) -> f64 {
        crate::core::constants::PRICE_AXIS_LABEL_OFFSET
    }
    /// Extra in-axis inset used when clamping Y-axis label boxes.
    /// Keeps crosshair/live-price labels fully inside at top/bottom edges.
    #[inline]
    pub fn price_axis_label_edge_inset(&self) -> f64 {
        self.price_axis_padding_tb().ceil() + self.axis_border_size as f64
    }
    /// Additional horizontal inset for full-width Y-axis labels (crosshair/live price).
    /// Keeps the label body visually inside the axis rather than riding the separator.
    #[inline]
    pub fn price_axis_full_label_inside_gap(&self) -> f64 {
        0.0
    }

    /// Computed optimal price axis width (CSS px) for a given max text width.
    /// LWC: `ceil(borderSize + tickLength + paddingInner + paddingOuter + LabelOffset + maxTextWidth)`,
    /// then rounded to the next even number.
    /// At default fontSize=12: `ceil(1 + 5 + 5 + 5 + 5 + textW)` = `ceil(21 + textW)`.
    #[inline]
    pub fn price_axis_width(&self, max_text_width: f64) -> f64 {
        let raw = self.axis_border_size as f64
            + self.axis_tick_length as f64
            + self.price_axis_padding_inner()
            + self.price_axis_padding_outer()
            + self.price_axis_label_offset()
            + max_text_width;
        // Keep a small practical floor so the axis never collapses on empty data.
        let raw = raw.max(12.0);
        let w = raw.ceil() as u32;
        (w + (w % 2)) as f64
    }

    /// Time axis optimal height (CSS px).
    /// LWC: `ceil(borderSize + tickLength + fontSize + paddingTop + paddingBottom + labelBottomOffset)`.
    /// At default fontSize=12: `ceil(1 + 5 + 12 + 3 + 3 + 4) = 28`.
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

    /// Time axis top padding.
    /// LWC: `paddingTop = 3 * fontSize / 12`.
    #[inline]
    pub fn time_axis_padding_top(&self) -> f64 {
        3.0 / 12.0 * self.font_size as f64
    }
    /// Time axis bottom padding.
    /// LWC: `paddingBottom = 3 * fontSize / 12`.
    #[inline]
    pub fn time_axis_padding_bottom(&self) -> f64 {
        3.0 / 12.0 * self.font_size as f64
    }
    /// Time axis paddingHorizontal: `9 * fontSize / 12`.
    #[inline]
    pub fn time_axis_padding_horizontal(&self) -> f64 {
        9.0 * self.font_size as f64 / 12.0
    }
    /// Additional bottom offset under X-axis labels.
    /// LWC: `labelBottomOffset = 4 * fontSize / 12`.
    #[inline]
    pub fn time_axis_label_bottom_offset(&self) -> f64 {
        4.0 / 12.0 * self.font_size as f64
    }

    /// Crosshair label additional padding (LWC: `2/12 * fontSize`).
    #[inline]
    pub fn crosshair_label_extra_padding(&self) -> f64 {
        2.0 / 12.0 * self.font_size as f64
    }
    /// Top inset for X-axis crosshair label box.
    /// LWC: label starts at y=0 (covers the border area).
    #[inline]
    pub fn time_axis_crosshair_label_top_inset(&self) -> f64 {
        0.0
    }

    /// Effective Y-axis row spacing for price ticks (CSS px).
    /// Matches LWC: `ceil(fontSize * tickMarkDensity)`.
    #[inline]
    pub fn price_scale_tick_mark_spacing_css(&self) -> f64 {
        (self.font_size as f64 * self.price_scale_tick_mark_density as f64)
            .ceil()
            .max(1.0)
    }

    /// Build the CSS font string for the axis: `"12px -apple-system, ..."`.
    #[inline]
    pub fn axis_font(&self, dpr: f64) -> String {
        format!(
            "{}px {}",
            (self.font_size as f64 * dpr).round(),
            self.font_family
        )
    }

    /// Build bold font string for time-axis bold labels.
    #[inline]
    pub fn axis_font_bold(&self, dpr: f64) -> String {
        format!(
            "bold {}px {}",
            (self.font_size as f64 * dpr).round(),
            self.font_family
        )
    }
}

impl Default for ChartStyle {
    fn default() -> Self {
        super::theme::default_style()
    }
}

/// Crosshair mode.
/// X line always snaps to bar centers (LWC behavior).
/// Y line behavior depends on mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrosshairMode {
    /// Normal mode — Y follows cursor exactly.
    #[default]
    Normal,
    /// Legacy magnet mode alias.
    /// Treated the same as `MagnetOHLC` in interaction logic.
    Magnet,
    /// Magnet OHLC mode — Y snaps to the nearest of O, H, L, C to the cursor Y.
    MagnetOHLC,
}

/// Crosshair state — the position of the crosshair in logical coordinates.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrosshairState {
    pub active: bool,
    pub x: f64,
    pub y: f64,
    pub bar_index: Option<usize>,
    pub price: f64,
    pub mode: CrosshairMode,
}

/// Information about the current render frame, passed to the renderer.
/// The renderer's canvas is sized to the pane (chart area) only.
pub struct RenderContext<'a> {
    pub bars: &'a BarArray,
    pub viewport: &'a Viewport,
    pub style: &'a ChartStyle,
    pub crosshair: &'a CrosshairState,
    pub dpr: f64,
    /// Horizontal pixel ratio: `bitmapWidth / cssWidth`.
    /// When using `device-pixel-content-box` this is the exact per-axis ratio;
    /// otherwise falls back to `dpr`.
    pub h_pixel_ratio: f64,
    /// Vertical pixel ratio: `bitmapHeight / cssHeight`.
    pub v_pixel_ratio: f64,
    /// Pre-computed tick marks for grid lines (price axis ticks).
    pub y_ticks: &'a [TickMark],
    /// Pre-computed tick marks for grid lines (time axis ticks).
    pub x_ticks: &'a [TickMark],
    /// Overlay series (line, area, etc.) — renderers iterate these in draw_lines().
    pub series: &'a SeriesCollection,
    /// Indicator runtime draw instruction stream for the current frame.
    pub indicator_draw_instructions: &'a [DrawInstruction],
    /// Main chart type (candlestick, OHLC bars, line, area, etc.).
    pub main_chart_type: crate::core::chart_type::MainChartType,
    /// Main chart rendering options.
    pub main_chart_options: &'a crate::core::chart_type::MainChartOptions,
    /// Drawings that should render below chart data (idle, non-hovered).
    pub bottom_drawings: &'a [DrawingGeometry],
    /// Footprint data for order-flow visualization (only used when main_chart_type is Footprint).
    pub footprint_data: &'a crate::core::footprint::FootprintData,
    /// Pre-computed footprint rects from geometry generator.
    /// Populated by ChartEngine::render() so both Canvas2D and WebGPU get identical geometry.
    pub footprint_rects: &'a [super::draw_list::ColoredRect],
    /// Pre-computed footprint text labels.
    /// Also stored on ChartEngine::footprint_texts for overlay rendering on WebGPU.
    pub footprint_texts: &'a [super::draw_list::DrawText],
}

/// Tick mark for axis rendering.
#[derive(Debug, Clone)]
pub struct TickMark {
    pub value: f64,
    pub pixel: f64,
    pub label: String,
    pub major: bool,
}

// ── The ChartRenderer trait ──────────────────────────────────────────────────

/// The phased rendering trait. Every rendering backend implements this.
///
/// The rendering pipeline is split into discrete phases so that the
/// engine can call individual phases for custom z-ordering.
///
/// WebGPU and Canvas2D are production backends on wasm32.
pub trait ChartRenderer {
    fn name(&self) -> &str;
    fn resize(&mut self, physical_width: u32, physical_height: u32, dpr: f64);
    fn is_valid(&self) -> bool {
        true
    }

    /// Acquire surface texture, create TextureView + CommandEncoder.
    /// Store them in `self` for the draw methods to use.
    fn begin_frame(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Draw background fill + grid lines.
    fn draw_grid(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Draw drawings that should live below series data.
    fn draw_bottom_drawings(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Draw candlesticks.
    /// Canvas2D: geometry_generator -> DrawList -> fill_rect loop.
    fn draw_candles(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Draw volume bars.
    /// Canvas2D: geometry_generator -> DrawList.
    fn draw_volume(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Draw indicator/study lines (SMA, EMA, etc).
    fn draw_lines(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Draw text labels (axis prices, timestamps).
    /// Canvas2D uses fillText.
    fn draw_text(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Draw crosshair overlay. Kept as a separate pass so that in the future
    /// it can render to a separate texture/overlay without re-rendering all
    /// candles when only the mouse moved.
    fn draw_crosshair(&mut self, ctx: &RenderContext) -> Result<(), String>;

    /// Submit command buffer, present surface texture.
    fn end_frame(&mut self) -> Result<(), String>;

    /// Default full-pipeline executor. Calls all phases in order.
    /// The engine can also call individual phases for custom z-indexing.
    fn render_frame(&mut self, ctx: &RenderContext) -> Result<(), String> {
        self.begin_frame(ctx)?;
        self.draw_grid(ctx)?;
        self.draw_bottom_drawings(ctx)?;
        self.draw_volume(ctx)?;
        self.draw_candles(ctx)?;
        self.draw_lines(ctx)?;
        self.draw_text(ctx)?;
        self.draw_crosshair(ctx)?;
        self.end_frame()
    }
}

/// Enum wrapper so ChartEngine can hold the renderer without dyn dispatch overhead.
/// WebGPU + Canvas2D are supported on wasm32; Noop exists for native compilation.
pub enum RendererBackend {
    #[cfg(target_arch = "wasm32")]
    WebGPU(super::wgpu_backend::WgpuRenderer),
    #[cfg(target_arch = "wasm32")]
    Canvas2D(super::canvas2d::Canvas2DRenderer),
    /// No-op backend for native builds (benchmarks, tests). Never renders anything.
    #[cfg(not(target_arch = "wasm32"))]
    Noop,
}

impl ChartRenderer for RendererBackend {
    fn name(&self) -> &str {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.name(),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.name(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => "noop",
        }
    }

    fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.resize(pw, ph, dpr),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.resize(pw, ph, dpr),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = (pw, ph, dpr);
            }
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.is_valid(),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.is_valid(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => true,
        }
    }

    fn begin_frame(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.begin_frame(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.begin_frame(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn draw_grid(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.draw_grid(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.draw_grid(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn draw_bottom_drawings(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.draw_bottom_drawings(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.draw_bottom_drawings(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn draw_candles(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.draw_candles(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.draw_candles(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn draw_volume(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.draw_volume(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.draw_volume(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn draw_lines(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.draw_lines(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.draw_lines(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn draw_text(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.draw_text(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.draw_text(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn draw_crosshair(&mut self, ctx: &RenderContext) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.draw_crosshair(ctx),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.draw_crosshair(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => {
                let _ = ctx;
                Ok(())
            }
        }
    }

    fn end_frame(&mut self) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WebGPU(r) => r.end_frame(),
            #[cfg(target_arch = "wasm32")]
            Self::Canvas2D(r) => r.end_frame(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Noop => Ok(()),
        }
    }
}

// Keep the old `Renderer` name as an alias during migration so downstream
// code (wasm/src/lib.rs) that imports `Renderer` doesn't break yet.
// TODO: remove once all consumers switch to `ChartRenderer`.
pub use ChartRenderer as Renderer;
