//! Drawing type definitions — shared across all drawing tools.
//!
//! Coordinates are stored in logical space (bar_index + price) so drawings
//! survive scroll, zoom, price auto-fit, and window resize.

use crate::core::renderer::draw_list::{ColoredRect, ColoredLine, DrawText};

// ── Logical coordinate ──────────────────────────────────────────────────────

/// A point in logical chart space (bar index + price).
/// Survives scroll/zoom/resize — converted to pixels at render time.
#[derive(Debug, Clone, Copy)]
pub struct DrawingPoint {
    /// Fractional bar index (e.g. 42.0 = center of bar 42).
    pub bar_index: f64,
    /// Price value.
    pub price: f64,
}

impl DrawingPoint {
    pub fn new(bar_index: f64, price: f64) -> Self {
        Self { bar_index, price }
    }
}

// ── Anchor points ───────────────────────────────────────────────────────────

/// An anchor point that the user can grab to edit a drawing.
#[derive(Debug, Clone, Copy)]
pub struct AnchorPoint {
    pub point: DrawingPoint,
    /// CSS-pixel radius for hit-testing (default 5.0).
    pub hit_radius: f64,
}

impl AnchorPoint {
    pub fn new(bar_index: f64, price: f64) -> Self {
        Self {
            point: DrawingPoint::new(bar_index, price),
            hit_radius: 5.0,
        }
    }

    pub fn with_radius(bar_index: f64, price: f64, radius: f64) -> Self {
        Self {
            point: DrawingPoint::new(bar_index, price),
            hit_radius: radius,
        }
    }
}

// ── Drawing state machine ───────────────────────────────────────────────────

/// State machine for drawing interaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawingState {
    /// User is placing anchor points (step 0 = first point, 1 = second, etc.)
    Creating { step: u8 },
    /// Finalized, not selected — renders on base canvas.
    Idle,
    /// Selected — anchors visible, waiting for drag or deselect.
    Selected,
    /// Being dragged. `anchor_index = None` means move entire drawing.
    Dragging {
        anchor_index: Option<usize>,
        start_bar: f64,
        start_price: f64,
    },
}

// ── Hit-test results ────────────────────────────────────────────────────────

/// What part of a drawing was hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitPart {
    /// Hit an anchor point (index into the drawing's anchor array).
    Anchor(usize),
    /// Hit the drawing body (line, rect fill, etc.)
    Body,
    /// No hit.
    None,
}

/// Full hit-test result.
#[derive(Debug, Clone, Copy)]
pub struct HitResult {
    pub part: HitPart,
    /// Distance from cursor to nearest edge (for ranking overlapping drawings).
    pub distance: f64,
}

impl HitResult {
    pub fn miss() -> Self {
        Self { part: HitPart::None, distance: f64::MAX }
    }

    pub fn hit(part: HitPart, distance: f64) -> Self {
        Self { part, distance }
    }

    pub fn is_hit(&self) -> bool {
        self.part != HitPart::None
    }
}

// ── Drawing style ───────────────────────────────────────────────────────────

/// Visual style for a drawing.
#[derive(Debug, Clone)]
pub struct DrawingStyle {
    /// Line color [R, G, B, A] (0.0–1.0).
    pub color: [f32; 4],
    /// Line width in CSS pixels.
    pub line_width: f64,
    /// Fill color (for rectangles, fib zones). None = no fill.
    pub fill_color: Option<[f32; 4]>,
    /// Dash pattern [dash, gap] in CSS px. None = solid.
    pub dash: Option<[f64; 2]>,
    /// Label font size in CSS px.
    pub font_size: f64,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        Self {
            color: [0.35, 0.55, 0.95, 1.0], // blue
            line_width: 1.0,
            fill_color: None,
            dash: None,
            font_size: 11.0,
        }
    }
}

// ── Drawing tool enum ───────────────────────────────────────────────────────

/// The type of drawing tool currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingTool {
    /// No drawing tool active — normal chart interaction.
    None,
    TrendLine,
    Rectangle,
    Fibonacci,
    Scale,
}

// ── Z-order ─────────────────────────────────────────────────────────────────

/// Where in the visual stack a drawing renders.
/// Matches LWC's PrimitivePaneViewZOrder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZOrder {
    /// Below series (behind candles).
    Bottom,
    /// Same level as series.
    Normal,
    /// Above everything (active/hovered drawings, above crosshair).
    Top,
}

// ── Rendered geometry output ────────────────────────────────────────────────

/// Pixel-space geometry produced by a drawing for one frame.
/// Consumed by the renderer (Canvas2D or WebGPU rect pipeline).
#[derive(Debug, Clone)]
pub struct DrawingGeometry {
    pub lines: Vec<ColoredLine>,
    pub rects: Vec<ColoredRect>,
    pub texts: Vec<DrawText>,
    /// Anchor circles for selected drawings (center_x, center_y, radius — all physical px).
    pub anchors: Vec<AnchorCircle>,
}

impl DrawingGeometry {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            rects: Vec::new(),
            texts: Vec::new(),
            anchors: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.rects.is_empty() && self.texts.is_empty() && self.anchors.is_empty()
    }
}

/// An anchor circle to render (for selected drawings).
#[derive(Debug, Clone, Copy)]
pub struct AnchorCircle {
    /// Center X in physical pixels.
    pub cx: f64,
    /// Center Y in physical pixels.
    pub cy: f64,
    /// Radius in physical pixels.
    pub radius: f64,
    /// Fill color.
    pub fill: [f32; 4],
    /// Border color.
    pub border: [f32; 4],
    /// Border width in physical pixels.
    pub border_width: f64,
}
