//! DrawList — unified geometry representation consumed by Canvas2D renderer.
//!
//! ALL visual elements are pre-computed into pixel-space primitives by
//! GeometryGenerator. The renderer is "dumb" — it just draws these primitives.

/// A filled rectangle in physical pixel coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredRect {
    /// Left edge X in physical pixels.
    pub x: f32,
    /// Top edge Y in physical pixels.
    pub y: f32,
    /// Width in physical pixels.
    pub w: f32,
    /// Height in physical pixels.
    pub h: f32,
    /// RGBA color (0.0–1.0).
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// A horizontally interpolated rectangle in physical pixel coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HorizontalGradientRect {
    /// Left edge X in physical pixels.
    pub x: f32,
    /// Top edge Y in physical pixels.
    pub y: f32,
    /// Width in physical pixels.
    pub w: f32,
    /// Height in physical pixels.
    pub h: f32,
    /// RGBA color at the left edge.
    pub left_r: f32,
    pub left_g: f32,
    pub left_b: f32,
    pub left_a: f32,
    /// RGBA color at the right edge.
    pub right_r: f32,
    pub right_g: f32,
    pub right_b: f32,
    pub right_a: f32,
}

/// A line segment in physical pixel coordinates for rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineSegment {
    /// Start X in physical pixels.
    pub x1: f32,
    /// Start Y in physical pixels.
    pub y1: f32,
    /// End X in physical pixels.
    pub x2: f32,
    /// End Y in physical pixels.
    pub y2: f32,
    /// Line width in physical pixels.
    pub width: f32,
    /// RGBA color (0.0–1.0).
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    /// Reserved slot to preserve the packed GPU instance layout.
    pub reserved: f32,
}

/// An area segment (trapezoid) for smooth area chart fills.
/// Top edge follows the line from (x1, y1) to (x2, y2).
/// Bottom edge is horizontal at y = bottom.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AreaSegment {
    /// Left X in physical pixels.
    pub x1: f32,
    /// Top-left Y (close price at bar i).
    pub y1: f32,
    /// Right X in physical pixels.
    pub x2: f32,
    /// Top-right Y (close price at bar i+1).
    pub y2: f32,
    /// Bottom Y (chart bottom, same for both sides).
    pub bottom: f32,
    /// Top RGBA color (0.0-1.0), used at the line edge.
    pub top_r: f32,
    pub top_g: f32,
    pub top_b: f32,
    pub top_a: f32,
    /// Bottom RGBA color (0.0-1.0), used at the baseline edge.
    pub bottom_r: f32,
    pub bottom_g: f32,
    pub bottom_b: f32,
    pub bottom_a: f32,
    /// Global gradient top Y (same across visible area segments).
    pub gradient_top: f32,
    /// Reserved slots to preserve the packed GPU instance layout.
    pub reserved1: f32,
    pub reserved2: f32,
}

/// A line segment in physical pixel coordinates (for future studies/drawings).
#[derive(Debug, Clone, Copy)]
pub struct ColoredLine {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub width: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    /// Dash pattern: [dash_len, gap_len]. Both 0.0 = solid.
    pub dash: f32,
    pub gap: f32,
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Center,
    Left,
    Right,
}

impl TextAlign {
    pub fn as_canvas_str(&self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// Serialize to a short key string for persistence.
    pub fn as_key(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }

    /// Deserialize from a key string. Returns `None` for unrecognised values.
    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// Vertical text alignment / canvas baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextVerticalAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

impl TextVerticalAlign {
    pub fn as_canvas_str(&self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Middle => "middle",
            Self::Bottom => "bottom",
        }
    }

    /// Serialize to a short key string for persistence.
    pub fn as_key(&self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Middle => "middle",
            Self::Bottom => "bottom",
        }
    }

    /// Deserialize from a key string. Returns `None` for unrecognised values.
    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "top" => Some(Self::Top),
            "middle" => Some(Self::Middle),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// Text element in physical pixel coordinates (for future overlay on main canvas).
#[derive(Debug, Clone)]
pub struct DrawText {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub font_weight: u16,
    pub italic: bool,
    pub rotation_rad: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub align: TextAlign,
    pub vertical_align: TextVerticalAlign,
}

/// The complete draw list for one frame. Generated by GeometryGenerator,
/// consumed by the Canvas2D renderer.
#[derive(Debug, Clone)]
pub struct DrawList {
    /// Filled rectangles (candle bodies, wicks, borders, volume bars).
    pub rects: Vec<ColoredRect>,
    /// Line segments (for future studies/drawings).
    pub lines: Vec<ColoredLine>,
    /// Text elements (for future main-canvas text).
    pub texts: Vec<DrawText>,
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            rects: Vec::with_capacity(4096),
            lines: Vec::with_capacity(256),
            texts: Vec::with_capacity(64),
        }
    }

    pub fn clear(&mut self) {
        self.rects.clear();
        self.lines.clear();
        self.texts.clear();
    }
}
