//! Draw-list IR consumed by rendering backends.
//!
//! Two coordinate flavors, mirroring the Canvas2D split lightweight-charts relies on
//! (see RENDERING_SPEC.md preamble):
//! - integer **bitmap** rects (`Rect`, `RectFrame`, `HLine`, `VLine`) — crisp, no AA;
//! - float bitmap-space geometry (`Polyline`, `AreaFill`, `RoundRect`, `Circle`, `Text`) — AA'd.

use crate::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineStyle {
    Solid,
    Dotted,
    Dashed,
    LargeDashed,
    SparseDotted,
}

impl LineStyle {
    /// Dash pattern in bitmap px for a given line width (RENDERING_SPEC.md §6).
    pub fn dash_pattern(&self, line_width: f32) -> Vec<f32> {
        let w = line_width;
        match self {
            LineStyle::Solid => vec![],
            LineStyle::Dotted => vec![w, w],
            LineStyle::Dashed => vec![2.0 * w, 2.0 * w],
            LineStyle::LargeDashed => vec![6.0 * w, 6.0 * w],
            LineStyle::SparseDotted => vec![w, 4.0 * w],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineType {
    Simple,
    WithSteps,
    Curved,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    pub top: Color,
    pub bottom: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    /// Integer bitmap-space filled rect (Canvas2D `fillRect` semantics).
    Rect { rect: IRect, color: Color },
    /// Hollow frame filled inside `rect` (Canvas2D `fillRectInnerBorder` semantics).
    RectFrame {
        rect: IRect,
        border: i32,
        color: Color,
    },
    /// Full-length 1px-class horizontal line at integer y (with half-pixel handling in backend).
    HLine {
        y: i32,
        x0: i32,
        x1: i32,
        width: i32,
        style: LineStyle,
        color: Color,
    },
    VLine {
        x: i32,
        y0: i32,
        y1: i32,
        width: i32,
        style: LineStyle,
        color: Color,
    },
    /// Anti-aliased polyline over `points[range]`, round joins / butt caps.
    Polyline {
        first_point: u32,
        point_count: u32,
        width: f32,
        style: LineStyle,
        line_type: LineType,
        color: Color,
    },
    /// Fill between polyline and a horizontal base with a vertical gradient.
    /// `line_type` matches the companion `Polyline` so stepped/curved areas trace the same edge.
    AreaFill {
        first_point: u32,
        point_count: u32,
        base_y: f32,
        line_type: LineType,
        gradient: Gradient,
    },
    RoundRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        /// left-top, right-top, right-bottom, left-bottom
        radii: [f32; 4],
        fill: Color,
        border_width: f32,
        border_color: Color,
    },
    Circle {
        cx: f32,
        cy: f32,
        radius: f32,
        fill: Color,
        stroke_width: f32,
        stroke: Color,
    },
    /// Filled triangle in bitmap space (markers/arrows and other small annotations).
    Triangle {
        a: [f32; 2],
        b: [f32; 2],
        c: [f32; 2],
        color: Color,
    },
    /// Vertical gradient over the full target (pane background).
    Background { gradient: Gradient },
    // Text runs come with the glyph engine in aion_render_wgpu; the IR slot is reserved so
    // layer ordering is stable.
    Text {
        run_id: u32,
        x: f32,
        y: f32,
        color: Color,
    },
}

/// One pane's frame output: `main` is redrawn on Light/Full invalidation; `top`
/// (crosshair + top primitives) is redrawn on every Cursor invalidation.
#[derive(Clone, Debug, Default)]
pub struct PaneLayers {
    pub main: Vec<Prim>,
    pub top: Vec<Prim>,
    /// Shared point pool referenced by Polyline/AreaFill ranges.
    pub points: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct DrawList {
    pub panes: Vec<PaneLayers>,
    pub time_axis: PaneLayers,
    pub left_price_axes: Vec<PaneLayers>,
    pub right_price_axes: Vec<PaneLayers>,
}
