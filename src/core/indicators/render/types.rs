use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub type ObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LayerBand {
    Background = 0,
    MainPriceVolume = 1,
    IndicatorFills = 2,
    IndicatorSeries = 3,
    IndicatorObjects = 4,
    Interaction = 5,
    AxisUi = 6,
}

impl Ord for LayerBand {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl PartialOrd for LayerBand {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOrderKey {
    pub layer_band: LayerBand,
    pub z: i16,
    pub declaration_order: u32,
    pub stable_id: u64,
}

impl Ord for RenderOrderKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.layer_band
            .cmp(&other.layer_band)
            .then_with(|| self.z.cmp(&other.z))
            .then_with(|| self.declaration_order.cmp(&other.declaration_order))
            .then_with(|| self.stable_id.cmp(&other.stable_id))
    }
}

impl PartialOrd for RenderOrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawInstruction {
    PlotLine {
        order: RenderOrderKey,
        series_id: String,
        points: Vec<(u64, f64)>,
        color: [f32; 4],
        width: f32,
    },
    PlotArea {
        order: RenderOrderKey,
        series_id: String,
        points: Vec<(u64, f64)>,
        top_color: [f32; 4],
        bottom_color: [f32; 4],
    },
    PlotHistogram {
        order: RenderOrderKey,
        series_id: String,
        points: Vec<(u64, f64)>,
        /// Default color used when per_point_colors is empty.
        color: [f32; 4],
        /// Optional per-point colors for dynamic styling (same length as points if used).
        per_point_colors: Vec<[f32; 4]>,
        base: f64,
    },
    PlotBar {
        order: RenderOrderKey,
        series_id: String,
        points: Vec<(u64, f64, f64, f64, f64)>,
        up_color: [f32; 4],
        down_color: [f32; 4],
    },
    PlotCandle {
        order: RenderOrderKey,
        series_id: String,
        points: Vec<(u64, f64, f64, f64, f64)>,
        up_color: [f32; 4],
        down_color: [f32; 4],
    },
    PlotShape {
        order: RenderOrderKey,
        shape: String,
        timestamp: u64,
        value: f64,
        color: [f32; 4],
        size: f32,
    },
    DrawLabel {
        order: RenderOrderKey,
        id: ObjectId,
        timestamp: u64,
        value: f64,
        text: String,
        color: [f32; 4],
    },
    DrawBox {
        order: RenderOrderKey,
        id: ObjectId,
        x1: u64,
        y1: f64,
        x2: u64,
        y2: f64,
        line_color: [f32; 4],
        fill_color: [f32; 4],
    },
    DrawLine {
        order: RenderOrderKey,
        id: ObjectId,
        x1: u64,
        y1: f64,
        x2: u64,
        y2: f64,
        color: [f32; 4],
        width: f32,
        style: String,
        extend: String,
    },
    DrawPolyline {
        order: RenderOrderKey,
        id: ObjectId,
        points: Vec<(u64, f64)>,
        color: [f32; 4],
        width: f32,
    },
    FillBetween {
        order: RenderOrderKey,
        upper_series_id: String,
        lower_series_id: String,
        color: [f32; 4],
    },
    DrawTable {
        order: RenderOrderKey,
        id: ObjectId,
        position: String, // position.top_left, position.top_center, etc.
        columns: u32,
        rows: u32,
        bgcolor: [f32; 4],
        frame_color: [f32; 4],
        frame_width: f32,
        border_color: [f32; 4],
        border_width: f32,
        cells: Vec<TableCell>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCell {
    pub column: u32,
    pub row: u32,
    pub text: String,
    pub text_color: [f32; 4],
    pub text_halign: String,
    pub text_valign: String,
    pub text_size: f32,
    pub bgcolor: [f32; 4],
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub tooltip: Option<String>,
}

impl DrawInstruction {
    pub fn order_key(&self) -> RenderOrderKey {
        match self {
            DrawInstruction::PlotLine { order, .. }
            | DrawInstruction::PlotArea { order, .. }
            | DrawInstruction::PlotHistogram { order, .. }
            | DrawInstruction::PlotBar { order, .. }
            | DrawInstruction::PlotCandle { order, .. }
            | DrawInstruction::PlotShape { order, .. }
            | DrawInstruction::DrawLabel { order, .. }
            | DrawInstruction::DrawBox { order, .. }
            | DrawInstruction::DrawLine { order, .. }
            | DrawInstruction::DrawPolyline { order, .. }
            | DrawInstruction::DrawTable { order, .. }
            | DrawInstruction::FillBetween { order, .. } => *order,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectMutation {
    Create {
        id: ObjectId,
        object_type: String,
        layer_band: LayerBand,
        z: i16,
        props: serde_json::Value,
    },
    Update {
        id: ObjectId,
        props: serde_json::Value,
    },
    Delete {
        id: ObjectId,
    },
}
