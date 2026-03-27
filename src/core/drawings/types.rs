//! Drawing type definitions — shared across all drawing tools.
//!
//! Coordinates are stored in logical space (bar_index + price) so drawings
//! survive scroll, zoom, price auto-fit, and window resize.

use crate::core::renderer::draw_list::{ColoredLine, ColoredRect, DrawText};

// ── Logical coordinate ──────────────────────────────────────────────────────

/// A point in logical chart space (bar index + price).
/// Survives scroll/zoom/resize — converted to pixels at render time.
/// An optional `timestamp` anchors the point in absolute time so drawings
/// can be remapped when bar data changes (e.g. timeframe switch).
#[derive(Debug, Clone, Copy)]
pub struct DrawingPoint {
    /// Fractional bar index (e.g. 42.0 = center of bar 42).
    pub bar_index: f64,
    /// Price value.
    pub price: f64,
    /// Absolute timestamp (epoch millis) corresponding to `bar_index`.
    /// Used to remap `bar_index` when the underlying bar data changes.
    pub timestamp: Option<u64>,
}

impl DrawingPoint {
    pub fn new(bar_index: f64, price: f64) -> Self {
        Self {
            bar_index,
            price,
            timestamp: None,
        }
    }

    pub fn with_timestamp(bar_index: f64, price: f64, timestamp: u64) -> Self {
        Self {
            bar_index,
            price,
            timestamp: Some(timestamp),
        }
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
    /// Hit an edge of the drawing (e.g. rectangle border, distinct from interior).
    Edge,
    /// No hit.
    None,
}

/// Determine the CSS cursor string for a drawing hit result.
///
/// For rectangles:
///   - Anchor corners → resize cursors (nwse-resize / nesw-resize)
///   - Edge → move
///   - Body (interior) → move
///
/// For trend lines / fib / scale:
///   - Anchor → pointer (clickable feel)
///   - Body → move
pub fn cursor_for_drawing_hit(
    tool: DrawingTool,
    part: HitPart,
    _anchor_index: Option<usize>,
) -> &'static str {
    match part {
        HitPart::None => "crosshair",
        HitPart::Anchor(idx) => {
            match tool {
                DrawingTool::Rectangle => {
                    // 8-handle rectangle:
                    // TL=0, TR=1, BR=2, BL=3, TM=4, RM=5, BM=6, LM=7
                    match idx {
                        0 => "nwse-resize", // top-left
                        1 => "nesw-resize", // top-right
                        2 => "nwse-resize", // bottom-right
                        3 => "nesw-resize", // bottom-left
                        4 => "ns-resize",   // top-mid
                        5 => "ew-resize",   // right-mid
                        6 => "ns-resize",   // bottom-mid
                        7 => "ew-resize",   // left-mid
                        _ => "move",
                    }
                }
                DrawingTool::HorizontalLine => "ns-resize", // vertical drag
                DrawingTool::VerticalLine => "ew-resize",   // horizontal drag
                _ => "pointer",                             // trend line, fib, scale, ray anchors
            }
        }
        HitPart::Edge => {
            match tool {
                DrawingTool::Rectangle => "move", // edge drag moves the whole rectangle
                DrawingTool::HorizontalLine => "ns-resize",
                DrawingTool::VerticalLine => "ew-resize",
                _ => "pointer",
            }
        }
        HitPart::Body => {
            match tool {
                // Rectangle body: move the whole shape.
                DrawingTool::Rectangle => "move",
                // Horizontal/vertical lines: move cursor
                DrawingTool::HorizontalLine => "ns-resize",
                DrawingTool::VerticalLine => "ew-resize",
                // Other drawings: move cursor on body
                _ => "move",
            }
        }
    }
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
        Self {
            part: HitPart::None,
            distance: f64::MAX,
        }
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
        // Delegate to theme defaults for the standard drawing color.
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        Self::from_theme(&theme)
    }
}

impl DrawingStyle {
    /// Create a DrawingStyle from theme drawing defaults.
    pub fn from_theme(theme: &crate::core::renderer::theme::ThemeConfig) -> Self {
        Self {
            color: theme.drawing_defaults.color,
            line_width: 1.0,
            fill_color: None,
            dash: None,
            font_size: theme.drawing_defaults.font_size,
        }
    }

    /// Create a DrawingStyle for rectangle drawings from theme.
    pub fn rectangle_from_theme(theme: &crate::core::renderer::theme::ThemeConfig) -> Self {
        let mut c = theme.drawing_defaults.color;
        c[3] = 0.15; // fill alpha
        Self {
            color: theme.drawing_defaults.color,
            line_width: 1.0,
            fill_color: Some(c),
            dash: None,
            font_size: theme.drawing_defaults.font_size,
        }
    }

    /// Create a DrawingStyle for Fibonacci drawings from theme.
    pub fn fibonacci_from_theme(theme: &crate::core::renderer::theme::ThemeConfig) -> Self {
        Self {
            color: theme.drawing_defaults.fibonacci_color,
            line_width: 1.0,
            fill_color: None,
            dash: None,
            font_size: theme.drawing_defaults.fibonacci_font_size,
        }
    }

    /// Create a DrawingStyle for Scale/measurement drawings from theme.
    pub fn scale_from_theme(theme: &crate::core::renderer::theme::ThemeConfig) -> Self {
        Self {
            color: theme.drawing_defaults.scale_color,
            line_width: 1.0,
            fill_color: Some(theme.drawing_defaults.scale_fill),
            dash: None,
            font_size: theme.drawing_defaults.font_size,
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
    /// Horizontal line at a price level (single anchor, extends full pane width).
    HorizontalLine,
    /// Vertical line at a bar index (single anchor, extends full pane height).
    VerticalLine,
    /// Ray / extended line (two anchors, extends to visible edges).
    Ray,
    /// Brush / freehand (variable points, recorded from pointer drag).
    Brush,
}

impl DrawingTool {
    pub fn as_api_key(self) -> &'static str {
        match self {
            DrawingTool::None => "none",
            DrawingTool::TrendLine => "trend_line",
            DrawingTool::Rectangle => "rectangle",
            DrawingTool::Fibonacci => "fibonacci",
            DrawingTool::Scale => "scale",
            DrawingTool::HorizontalLine => "horizontal_line",
            DrawingTool::VerticalLine => "vertical_line",
            DrawingTool::Ray => "ray",
            DrawingTool::Brush => "brush",
        }
    }

    pub fn from_api_key(value: &str) -> Option<Self> {
        match value {
            "none" => Some(DrawingTool::None),
            "trend_line" => Some(DrawingTool::TrendLine),
            "rectangle" => Some(DrawingTool::Rectangle),
            "fibonacci" => Some(DrawingTool::Fibonacci),
            "scale" => Some(DrawingTool::Scale),
            "horizontal_line" => Some(DrawingTool::HorizontalLine),
            "vertical_line" => Some(DrawingTool::VerticalLine),
            "ray" => Some(DrawingTool::Ray),
            "brush" => Some(DrawingTool::Brush),
            _ => None,
        }
    }
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
    /// Above series on the overlay layer (active/hovered drawings).
    Top,
}

// ── Rendered geometry output ────────────────────────────────────────────────

/// Pixel-space geometry produced by a drawing for one frame.
/// Consumed by the Canvas2D renderer.
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
        self.lines.is_empty()
            && self.rects.is_empty()
            && self.texts.is_empty()
            && self.anchors.is_empty()
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
