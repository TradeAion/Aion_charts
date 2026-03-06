use super::types::{AnchorPoint, DrawingPoint, DrawingStyle, DrawingTool};
use serde::{Deserialize, Serialize};

pub const DRAWINGS_SNAPSHOT_VERSION: u32 = 1;

fn snapshot_version() -> u32 {
    DRAWINGS_SNAPSHOT_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DrawingSnapshot {
    #[serde(default = "snapshot_version")]
    pub version: u32,
    #[serde(default)]
    pub drawings: Vec<SerializedDrawing>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedDrawing {
    #[serde(default)]
    pub id: u64,
    pub tool: String,
    #[serde(default)]
    pub style: SerializedDrawingStyle,
    #[serde(default)]
    pub anchors: Vec<SerializedAnchorPoint>,
    /// Brush intermediate points (excluding first/last anchors).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<SerializedDrawingPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedAnchorPoint {
    pub point: SerializedDrawingPoint,
    #[serde(default = "default_hit_radius")]
    pub hit_radius: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SerializedDrawingPoint {
    pub bar_index: f64,
    pub price: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedDrawingStyle {
    pub color: [f32; 4],
    pub line_width: f64,
    pub fill_color: Option<[f32; 4]>,
    pub dash: Option<[f64; 2]>,
    pub font_size: f64,
}

impl Default for SerializedDrawingStyle {
    fn default() -> Self {
        (&DrawingStyle::default()).into()
    }
}

fn default_hit_radius() -> f64 {
    5.0
}

impl From<&DrawingStyle> for SerializedDrawingStyle {
    fn from(value: &DrawingStyle) -> Self {
        Self {
            color: value.color,
            line_width: value.line_width,
            fill_color: value.fill_color,
            dash: value.dash,
            font_size: value.font_size,
        }
    }
}

impl From<SerializedDrawingStyle> for DrawingStyle {
    fn from(value: SerializedDrawingStyle) -> Self {
        Self {
            color: value.color,
            line_width: value.line_width,
            fill_color: value.fill_color,
            dash: value.dash,
            font_size: value.font_size,
        }
    }
}

impl From<DrawingPoint> for SerializedDrawingPoint {
    fn from(value: DrawingPoint) -> Self {
        Self {
            bar_index: value.bar_index,
            price: value.price,
            timestamp: value.timestamp,
        }
    }
}

impl From<&DrawingPoint> for SerializedDrawingPoint {
    fn from(value: &DrawingPoint) -> Self {
        Self {
            bar_index: value.bar_index,
            price: value.price,
            timestamp: value.timestamp,
        }
    }
}

impl From<SerializedDrawingPoint> for DrawingPoint {
    fn from(value: SerializedDrawingPoint) -> Self {
        Self {
            bar_index: value.bar_index,
            price: value.price,
            timestamp: value.timestamp,
        }
    }
}

impl From<&AnchorPoint> for SerializedAnchorPoint {
    fn from(value: &AnchorPoint) -> Self {
        Self {
            point: SerializedDrawingPoint::from(value.point),
            hit_radius: value.hit_radius,
        }
    }
}

impl From<SerializedAnchorPoint> for AnchorPoint {
    fn from(value: SerializedAnchorPoint) -> Self {
        AnchorPoint {
            point: value.point.into(),
            hit_radius: value.hit_radius,
        }
    }
}

pub fn drawing_tool_to_key(tool: DrawingTool) -> &'static str {
    tool.as_api_key()
}

pub fn drawing_tool_from_key(key: &str) -> Option<DrawingTool> {
    DrawingTool::from_api_key(key)
}
