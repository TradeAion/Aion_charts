//! Drawing type definitions — shared across all drawing tools.
//!
//! Coordinates are stored in logical space (bar_index + price) so drawings
//! survive scroll, zoom, price auto-fit, and window resize.

use crate::core::renderer::draw_list::{
    ColoredLine, ColoredRect, DrawText, TextAlign, TextVerticalAlign,
};
use serde::Serialize;

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
#[derive(Debug, Clone, PartialEq)]
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
        initial_bar: f64,
        initial_price: f64,
        fixed_bar: Option<f64>,
        fixed_price: Option<f64>,
    },
}

// ── Hit-test results ────────────────────────────────────────────────────────

/// What part of a drawing was hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitPart {
    /// Hit an anchor point (index into the drawing's anchor array).
    Anchor(usize),
    /// Hit the drawing's inline text label / placeholder.
    Label,
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
    locked: bool,
) -> &'static str {
    if locked && part != HitPart::None {
        return "not-allowed";
    }

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
        HitPart::Label => "text",
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
            line_width: 2.0,
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
            line_width: 2.0,
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

/// Shared inline text configuration for drawings that support labels/notes.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingTextStyle {
    pub font_size: Option<f64>,
    pub italic: bool,
    pub color: Option<[f32; 4]>,
}

impl Default for DrawingTextStyle {
    fn default() -> Self {
        Self {
            font_size: None,
            italic: false,
            color: None,
        }
    }
}

impl DrawingTextStyle {
    pub fn resolved_font_size(&self, fallback: f64) -> f64 {
        self.font_size
            .filter(|value| value.is_finite())
            .unwrap_or(fallback)
            .clamp(8.0, 48.0)
    }

    pub fn resolved_color(&self, fallback: [f32; 4]) -> [f32; 4] {
        self.color
            .unwrap_or(fallback)
            .map(|channel| channel.clamp(0.0, 1.0))
    }

    pub fn set_font_size(&mut self, value: f64) {
        if value.is_finite() {
            self.font_size = Some(value.clamp(8.0, 48.0));
        }
    }

    pub fn set_color_override(&mut self, color: Option<[f32; 4]>) {
        self.color = color.map(|rgba| rgba.map(|channel| channel.clamp(0.0, 1.0)));
    }
}

pub fn rgba_to_hex(color: [f32; 4]) -> String {
    let [r, g, b, _a] = color.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawingText {
    pub value: String,
    pub horizontal_align: TextAlign,
    pub vertical_align: TextVerticalAlign,
    pub style: DrawingTextStyle,
}

impl Default for DrawingText {
    fn default() -> Self {
        Self {
            value: String::new(),
            horizontal_align: TextAlign::Right,
            vertical_align: TextVerticalAlign::Top,
            style: DrawingTextStyle::default(),
        }
    }
}

impl DrawingText {
    pub fn rectangle_default() -> Self {
        Self {
            value: String::new(),
            horizontal_align: TextAlign::Right,
            vertical_align: TextVerticalAlign::Top,
            style: DrawingTextStyle::default(),
        }
    }
}

impl DrawingText {
    pub fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }
}

/// User-configurable Fibonacci level definition.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FibonacciLevel {
    pub ratio: f64,
    pub label: String,
}

impl FibonacciLevel {
    pub fn new(ratio: f64, label: impl Into<String>) -> Self {
        Self {
            ratio,
            label: label.into(),
        }
    }
}

/// Optional horizontal middle line for a Rectangle drawing (platform-style).
///
/// When `Some`, the rectangle renders an extra horizontal line through its
/// vertical midpoint, spanning the rectangle's full width. The line uses its
/// own color, width, and dash pattern, independent of the rectangle border.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiddleLineStyle {
    /// Line color [R, G, B, A] (0.0–1.0).
    pub color: [f32; 4],
    /// Line width in CSS pixels.
    pub line_width: f64,
    /// Optional dash pattern [dash, gap] in CSS px. None = solid.
    pub dash: Option<[f64; 2]>,
}

impl Default for MiddleLineStyle {
    fn default() -> Self {
        // Default to a neutral mid-gray, 1px solid line — matches the reference platform
        // default middle-line appearance once enabled.
        Self {
            color: [0.55, 0.55, 0.55, 1.0],
            line_width: 1.0,
            dash: None,
        }
    }
}

/// Rectangle in CSS pixels used by the demo/editor overlay.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DrawingTextEditorTarget {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub rotation_deg: f64,
}

/// UI-facing snapshot of the currently selected drawing's text capabilities.
#[derive(Debug, Clone, Serialize)]
pub struct SelectedDrawingInfo {
    pub id: u64,
    pub tool: String,
    pub locked: bool,
    pub supports_text: bool,
    pub supports_text_style: bool,
    pub placeholder: String,
    pub text: String,
    pub horizontal_align: String,
    pub vertical_align: String,
    pub text_font_size: f64,
    pub text_italic: bool,
    pub drawing_color: String,
    pub text_color: String,
    pub text_color_follows_drawing: bool,
    pub text_editing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_target: Option<DrawingTextEditorTarget>,
    pub supports_fibonacci_levels: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fibonacci_levels: Vec<FibonacciLevel>,
    /// True when the selected drawing supports a horizontal middle line
    /// (currently: Rectangle only).
    pub supports_middle_line: bool,
    /// True when the middle line is enabled on the selected drawing.
    pub middle_line_enabled: bool,
    /// Hex string ("#RRGGBB") of the middle-line color, when applicable.
    pub middle_line_color: String,
    /// Middle-line width in CSS px, when applicable.
    pub middle_line_width: f64,
    /// Optional dash pattern [dash, gap] for the middle line, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle_line_dash: Option<[f64; 2]>,
    /// True when the selected drawing supports an on/off toggle for its
    /// border (currently: Rectangle and Text drawings).
    pub supports_border: bool,
    /// True when the border is currently enabled on the selected drawing.
    pub border_enabled: bool,
    /// Hex string ("#RRGGBB") of the border color, when applicable. Mirrors
    /// `drawing_color` for tools whose border color == the drawing color.
    pub border_color: String,
    /// Border line width in CSS px, when applicable.
    pub border_width: f64,
    /// Optional dash pattern [dash, gap] for the border, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_dash: Option<[f64; 2]>,
    /// True when the selected drawing supports an on/off toggle for its
    /// background fill (currently: Text drawing only).
    pub supports_fill: bool,
    /// True when the fill is currently enabled on the selected drawing.
    pub fill_enabled: bool,
    /// Hex string ("#RRGGBB") of the fill color, when applicable.
    pub fill_color: String,
    /// Fill alpha in [0, 1], when applicable. Surfaced separately from
    /// `fill_color` so the UI can edit opacity without parsing #RRGGBBAA.
    pub fill_alpha: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingsLockSummary {
    pub total: usize,
    pub locked_count: usize,
    pub all_locked: bool,
}

#[cfg(test)]
mod tests {
    use super::{DrawingStyle, DrawingText};
    use crate::core::renderer::draw_list::{TextAlign, TextVerticalAlign};
    use crate::core::renderer::theme::ThemeConfig;

    #[test]
    fn default_line_based_drawing_style_uses_two_pixel_width() {
        let theme = ThemeConfig::default();
        let style = DrawingStyle::from_theme(&theme);

        assert_eq!(style.line_width, 2.0);
    }

    #[test]
    fn fibonacci_drawing_style_uses_two_pixel_width() {
        let theme = ThemeConfig::default();
        let style = DrawingStyle::fibonacci_from_theme(&theme);

        assert_eq!(style.line_width, 2.0);
    }

    #[test]
    fn drawing_text_defaults_to_right_top_alignment() {
        let text = DrawingText::default();
        assert_eq!(text.horizontal_align, TextAlign::Right);
        assert_eq!(text.vertical_align, TextVerticalAlign::Top);
    }

    #[test]
    fn rectangle_text_defaults_to_right_top_alignment() {
        let text = DrawingText::rectangle_default();
        assert_eq!(text.horizontal_align, TextAlign::Right);
        assert_eq!(text.vertical_align, TextVerticalAlign::Top);
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
    /// Multi-click polyline/path drawing (double-click or Enter to finish).
    Path,
    /// Brush / freehand (variable points, recorded from pointer drag).
    Brush,
    /// Text annotation (single anchor, auto-sized to text content).
    Text,
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
            DrawingTool::Path => "path",
            DrawingTool::Brush => "brush",
            DrawingTool::Text => "text",
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
            "path" => Some(DrawingTool::Path),
            "brush" => Some(DrawingTool::Brush),
            "text" => Some(DrawingTool::Text),
            _ => None,
        }
    }
}

// ── Z-order ─────────────────────────────────────────────────────────────────

/// Where in the visual stack a drawing renders.
/// Matches the reference implementation's PrimitivePaneViewZOrder.
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

#[derive(Debug, Clone, Copy)]
pub struct HorizontalLineAxisLabel {
    pub price: f64,
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct VerticalLineAxisLabel {
    pub bar_index: f64,
    pub timestamp: Option<u64>,
    pub color: [f32; 4],
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
