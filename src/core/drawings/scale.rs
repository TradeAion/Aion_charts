//! Scale (Price Range) drawing — 2-anchor measurement tool.
//!
//! Shows price difference, percentage change, and bar count between two points.

use super::drawing::{next_drawing_id, point_to_bitmap, Drawing};
use super::types::*;
use crate::core::renderer::draw_list::{ColoredLine, ColoredRect, DrawText};
use crate::core::viewport::Viewport;
use crate::impl_drawing_accessors;

#[derive(Debug)]
pub struct ScaleDrawing {
    id: u64,
    state: DrawingState,
    style: DrawingStyle,
    anchors: Vec<AnchorPoint>,
}

impl ScaleDrawing {
    pub fn new(bar_index: f64, price: f64) -> Self {
        let id = next_drawing_id();
        Self {
            id,
            state: DrawingState::Creating { step: 1 },
            style: DrawingStyle::scale_from_theme(
                &crate::core::renderer::theme::ThemeConfig::default(),
            ),
            anchors: vec![
                AnchorPoint::new(bar_index, price),
                AnchorPoint::new(bar_index, price),
            ],
        }
    }
}

impl Drawing for ScaleDrawing {
    impl_drawing_accessors!(DrawingTool::Scale);
    fn required_anchors(&self) -> usize {
        2
    }

    fn hit_test(&self, _cx: f64, _cy: f64, _vp: &Viewport, _pw: f64, _ph: f64) -> HitResult {
        // Scale tool is hold-only and ephemeral — never hit-testable
        HitResult::miss()
    }

    fn generate_geometry(
        &self,
        vp: &Viewport,
        pw: f64,
        ph: f64,
        _dpr: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
        _show_anchors: bool,
    ) -> DrawingGeometry {
        let mut geom = DrawingGeometry::new();
        if self.anchors.len() < 2 {
            return geom;
        }

        let (bx0, by0) = point_to_bitmap(
            &self.anchors[0].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
        );
        let (bx1, by1) = point_to_bitmap(
            &self.anchors[1].point,
            vp,
            pw,
            ph,
            h_pixel_ratio,
            v_pixel_ratio,
        );

        let c = &self.style.color;
        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let lw = (self.style.line_width * avg_ratio).floor().max(1.0) as f32;

        let px0 = bx0 as f32;
        let py0 = by0 as f32;
        let px1 = bx1 as f32;
        let py1 = by1 as f32;

        // Shaded area between the two price levels
        if let Some(fc) = self.style.fill_color {
            let rx = px0.min(px1);
            let ry = py0.min(py1);
            let rw = (px0 - px1).abs();
            let rh = (py0 - py1).abs();
            geom.rects.push(ColoredRect {
                x: rx,
                y: ry,
                w: rw,
                h: rh,
                r: fc[0],
                g: fc[1],
                b: fc[2],
                a: fc[3],
            });
        }

        // Top horizontal line
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py0,
            x1: px1,
            y1: py0,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });
        // Bottom horizontal line
        geom.lines.push(ColoredLine {
            x0: px0,
            y0: py1,
            x1: px1,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });
        // Vertical connector (middle)
        let mid_x = (px0 + px1) / 2.0;
        geom.lines.push(ColoredLine {
            x0: mid_x,
            y0: py0,
            x1: mid_x,
            y1: py1,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });

        // Arrow heads on the vertical connector
        let arrow_size = (4.0 * avg_ratio) as f32;
        let top_y = py0.min(py1);
        let bottom_y = py0.max(py1);
        // Up arrow
        geom.lines.push(ColoredLine {
            x0: mid_x - arrow_size,
            y0: top_y + arrow_size,
            x1: mid_x,
            y1: top_y,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });
        geom.lines.push(ColoredLine {
            x0: mid_x + arrow_size,
            y0: top_y + arrow_size,
            x1: mid_x,
            y1: top_y,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });
        // Down arrow
        geom.lines.push(ColoredLine {
            x0: mid_x - arrow_size,
            y0: bottom_y - arrow_size,
            x1: mid_x,
            y1: bottom_y,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });
        geom.lines.push(ColoredLine {
            x0: mid_x + arrow_size,
            y0: bottom_y - arrow_size,
            x1: mid_x,
            y1: bottom_y,
            width: lw,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            dash: 0.0,
            gap: 0.0,
        });

        // Label: price diff, pct change, bar count
        let p0 = self.anchors[0].point.price;
        let p1 = self.anchors[1].point.price;
        let diff = p1 - p0;
        let pct = if p0.abs() > 1e-10 {
            (diff / p0) * 100.0
        } else {
            0.0
        };
        let bars = (self.anchors[1].point.bar_index - self.anchors[0].point.bar_index)
            .abs()
            .round() as i64;

        let sign = if diff >= 0.0 { "+" } else { "" };
        let label = format!("{}{:.2} ({}{:.2}%) | {} bars", sign, diff, sign, pct, bars);
        let fs = (self.style.font_size * avg_ratio) as f32;

        geom.texts.push(DrawText {
            text: label,
            x: mid_x,
            y: (top_y + bottom_y) / 2.0 - fs * 0.6,
            font_size: fs,
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
            align: Default::default(),
        });

        // Scale tool is hold-only: never show anchor circles

        geom
    }
}
