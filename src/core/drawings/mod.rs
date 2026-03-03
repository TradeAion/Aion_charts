//! Drawings subsystem — trend lines, fib retracements, rectangles, scale tools, brush.
//!
//! Architecture:
//! - `types.rs`: shared types (DrawingPoint, AnchorPoint, DrawingState, etc.)
//! - `drawing.rs`: Drawing trait all tools implement
//! - `hit_test.rs`: geometric hit-test math
//! - `trend_line.rs`, `rectangle.rs`, `fibonacci.rs`, `scale.rs`, `brush.rs`: concrete tools
//! - `horizontal_line.rs`, `vertical_line.rs`, `ray.rs`: additional line tools
//! - `DrawingManager` (this file): owns all drawings, dispatches hit-tests, manages active tool

pub mod brush;
pub mod drawing;
pub mod fibonacci;
pub mod hit_test;
pub mod horizontal_line;
pub mod persistence;
pub mod ray;
pub mod rectangle;
pub mod scale;
pub mod trend_line;
pub mod types;
pub mod vertical_line;

use crate::core::viewport::Viewport;
use drawing::{ensure_next_drawing_id_at_least, Drawing};
use persistence::{
    drawing_tool_from_key, drawing_tool_to_key, DrawingSnapshot, SerializedAnchorPoint,
    SerializedDrawing, SerializedDrawingPoint, DRAWINGS_SNAPSHOT_VERSION,
};
use types::*;

/// Returns the default anchor circle fill color from the theme.
/// Used by all drawing geometry methods for consistent anchor appearance.
pub fn default_anchor_color() -> [f32; 4] {
    crate::core::renderer::theme::ThemeConfig::default()
        .drawing_defaults
        .anchor_color
}

/// Manages all drawings on the chart.
pub struct DrawingManager {
    /// All drawings, ordered by creation time.
    drawings: Vec<Box<dyn Drawing>>,
    /// Currently active drawing tool (None = normal chart interaction).
    pub active_tool: DrawingTool,
    /// ID of the currently selected drawing (if any).
    pub selected_id: Option<u64>,
    /// ID of the drawing currently being created (if any).
    creating_id: Option<u64>,
    /// ID of the drawing currently hovered by pointer hit-test (transient).
    hovered_id: Option<u64>,
}

impl DrawingManager {
    pub fn new() -> Self {
        Self {
            drawings: Vec::new(),
            active_tool: DrawingTool::None,
            selected_id: None,
            creating_id: None,
            hovered_id: None,
        }
    }

    /// Add a drawing (already constructed).
    pub fn add(&mut self, drawing: Box<dyn Drawing>) {
        self.drawings.push(drawing);
    }

    /// Remove a drawing by ID.
    pub fn remove(&mut self, id: u64) {
        self.drawings.retain(|d| d.id() != id);
        if self.selected_id == Some(id) {
            self.selected_id = None;
        }
        if self.creating_id == Some(id) {
            self.creating_id = None;
        }
        if self.hovered_id == Some(id) {
            self.hovered_id = None;
        }
    }

    /// Remove the currently selected drawing.
    pub fn remove_selected(&mut self) {
        if let Some(id) = self.selected_id.take() {
            self.remove(id);
        }
    }

    /// Remove all scale drawings from the chart.
    pub fn remove_all_scale(&mut self) {
        self.drawings.retain(|d| d.tool() != DrawingTool::Scale);
        // Clear selection if it pointed to a scale drawing that was removed
        if let Some(id) = self.selected_id {
            if self.get(id).is_none() {
                self.selected_id = None;
            }
        }
        if let Some(id) = self.hovered_id {
            if self.get(id).is_none() {
                self.hovered_id = None;
            }
        }
    }

    /// Get a drawing by ID.
    pub fn get(&self, id: u64) -> Option<&Box<dyn Drawing>> {
        self.drawings.iter().find(|d| d.id() == id)
    }

    /// Get a mutable drawing by ID.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Box<dyn Drawing>> {
        self.drawings.iter_mut().find(|d| d.id() == id)
    }

    /// All drawings (for rendering).
    pub fn all(&self) -> &[Box<dyn Drawing>] {
        &self.drawings
    }

    /// Number of drawings.
    pub fn len(&self) -> usize {
        self.drawings.len()
    }

    /// Remove all drawings and reset interaction state.
    pub fn clear(&mut self) {
        self.drawings.clear();
        self.active_tool = DrawingTool::None;
        self.selected_id = None;
        self.creating_id = None;
        self.hovered_id = None;
    }

    /// Set the currently hovered drawing id. `None` clears hover.
    pub fn set_hovered(&mut self, id: Option<u64>) {
        self.hovered_id = id.filter(|hovered| self.get(*hovered).is_some());
    }

    /// Clear transient hover state.
    pub fn clear_hovered(&mut self) {
        self.hovered_id = None;
    }

    /// Current hovered drawing id.
    pub fn hovered_id(&self) -> Option<u64> {
        self.hovered_id
    }

    /// Is a drawing tool currently active?
    pub fn is_tool_active(&self) -> bool {
        self.active_tool != DrawingTool::None
    }

    /// Is a drawing currently being created?
    pub fn is_creating(&self) -> bool {
        self.creating_id.is_some()
    }

    /// Deselect all drawings.
    pub fn deselect_all(&mut self) {
        if let Some(id) = self.selected_id.take() {
            if let Some(d) = self.get_mut(id) {
                if d.state() == DrawingState::Selected {
                    d.set_state(DrawingState::Idle);
                }
            }
        }
    }

    /// Select a drawing by ID.
    pub fn select(&mut self, id: u64) {
        self.deselect_all();
        self.selected_id = Some(id);
        if let Some(d) = self.get_mut(id) {
            d.set_state(DrawingState::Selected);
        }
    }

    /// Hit-test all drawings at the given CSS cursor position.
    /// Returns the ID of the best (closest) hit, or None.
    pub fn hit_test(
        &self,
        cursor_css_x: f64,
        cursor_css_y: f64,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> Option<(u64, HitResult)> {
        let mut best: Option<(u64, HitResult)> = None;

        for d in &self.drawings {
            let result = d.hit_test(cursor_css_x, cursor_css_y, vp, pane_css_w, pane_css_h);
            if result.is_hit() {
                match &best {
                    Some((_, prev)) if prev.distance <= result.distance => {}
                    _ => {
                        best = Some((d.id(), result));
                    }
                }
            }
        }

        best
    }

    /// Start creating a new drawing with the active tool.
    /// Returns the ID of the new drawing, or None if no tool is active.
    pub fn start_creating(&mut self, bar_index: f64, price: f64) -> Option<u64> {
        let tool = self.active_tool;
        if tool == DrawingTool::None {
            return None;
        }

        self.deselect_all();

        let drawing: Box<dyn Drawing> = match tool {
            DrawingTool::TrendLine => Box::new(trend_line::TrendLineDrawing::new(bar_index, price)),
            DrawingTool::Rectangle => Box::new(rectangle::RectangleDrawing::new(bar_index, price)),
            DrawingTool::Fibonacci => Box::new(fibonacci::FibonacciDrawing::new(bar_index, price)),
            DrawingTool::Scale => Box::new(scale::ScaleDrawing::new(bar_index, price)),
            DrawingTool::Brush => Box::new(brush::BrushDrawing::new(bar_index, price)),
            DrawingTool::HorizontalLine => Box::new(horizontal_line::HorizontalLineDrawing::new(
                bar_index, price,
            )),
            DrawingTool::VerticalLine => {
                Box::new(vertical_line::VerticalLineDrawing::new(bar_index, price))
            }
            DrawingTool::Ray => Box::new(ray::RayDrawing::new(bar_index, price)),
            DrawingTool::None => unreachable!(),
        };

        let id = drawing.id();
        self.drawings.push(drawing);
        self.creating_id = Some(id);
        Some(id)
    }

    /// Update the creation preview (mouse move during creation).
    pub fn update_creation_preview(&mut self, bar_index: f64, price: f64) {
        if let Some(id) = self.creating_id {
            if let Some(d) = self.get_mut(id) {
                d.update_creation_preview(bar_index, price);
            }
        }
    }

    /// Get the first anchor point of the drawing being created (for snap calculations).
    /// Returns None if not creating or no anchors exist yet.
    pub fn creation_first_anchor(&self) -> Option<(f64, f64)> {
        let id = self.creating_id?;
        let d = self.get(id)?;
        let anchors = d.anchors();
        if anchors.is_empty() {
            return None;
        }
        Some((anchors[0].point.bar_index, anchors[0].point.price))
    }

    /// Get the tool type of the drawing being created.
    pub fn creation_tool(&self) -> Option<DrawingTool> {
        let id = self.creating_id?;
        self.get(id).map(|d| d.tool())
    }

    /// Get the "opposite" anchor for angle snapping during single-anchor drag.
    /// When dragging anchor N, returns anchor 0 if N > 0, else anchor 1.
    /// Returns None if not dragging a single anchor or drawing has < 2 anchors.
    pub fn drag_opposite_anchor(&self, id: u64) -> Option<(f64, f64)> {
        let d = self.get(id)?;
        let anchors = d.anchors();
        if anchors.len() < 2 {
            return None;
        }
        match d.state() {
            DrawingState::Dragging {
                anchor_index: Some(ai),
                ..
            } => {
                // Return the "other" anchor for angle reference
                let other_idx = if ai == 0 { 1 } else { 0 };
                if other_idx < anchors.len() {
                    Some((
                        anchors[other_idx].point.bar_index,
                        anchors[other_idx].point.price,
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get the tool type of a drawing by ID.
    pub fn tool_of(&self, id: u64) -> Option<DrawingTool> {
        self.get(id).map(|d| d.tool())
    }

    /// Finalize the current creation step (mouse release / click).
    /// Returns true if the drawing is now complete.
    pub fn finalize_creation_step(&mut self, bar_index: f64, price: f64) -> bool {
        let id = match self.creating_id {
            Some(id) => id,
            None => return true,
        };

        let complete = {
            match self.drawings.iter_mut().find(|d| d.id() == id) {
                Some(d) => d.add_creation_point(bar_index, price),
                None => return true,
            }
        };

        if complete {
            let tool = self
                .drawings
                .iter()
                .find(|d| d.id() == id)
                .map(|d| d.tool());
            self.creating_id = None;
            self.selected_id = Some(id);
            if let Some(d) = self.drawings.iter_mut().find(|d| d.id() == id) {
                d.set_state(DrawingState::Selected);
            }
            // Scale tool is hold-only: keep it active so user can immediately create another
            if tool != Some(DrawingTool::Scale) {
                self.active_tool = DrawingTool::None;
            }
        }
        complete
    }

    /// Cancel the current creation (e.g. Escape key).
    pub fn cancel_creation(&mut self) {
        if let Some(id) = self.creating_id.take() {
            self.remove(id);
        }
    }

    /// Start dragging a selected drawing (or one of its anchors).
    pub fn start_drag(&mut self, id: u64, anchor_index: Option<usize>, bar_index: f64, price: f64) {
        if let Some(d) = self.get_mut(id) {
            d.set_state(DrawingState::Dragging {
                anchor_index,
                start_bar: bar_index,
                start_price: price,
            });
        }
    }

    /// Update drag position.
    pub fn update_drag(&mut self, id: u64, bar_index: f64, price: f64) {
        if let Some(d) = self.get_mut(id) {
            match d.state() {
                DrawingState::Dragging {
                    anchor_index,
                    start_bar,
                    start_price,
                } => {
                    if let Some(ai) = anchor_index {
                        // Move single anchor
                        d.move_anchor(ai, bar_index, price);
                    } else {
                        // Move entire drawing
                        let delta_bar = bar_index - start_bar;
                        let delta_price = price - start_price;
                        d.move_by(delta_bar, delta_price);
                        d.set_state(DrawingState::Dragging {
                            anchor_index: None,
                            start_bar: bar_index,
                            start_price: price,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// End drag → back to Selected.
    pub fn end_drag(&mut self, id: u64) {
        if let Some(d) = self.get_mut(id) {
            d.set_state(DrawingState::Selected);
        }
    }

    /// Generate all drawing geometry for rendering.
    /// Splits into base-layer (idle, non-hovered) and top-layer (hovered/active).
    pub fn generate_all_geometry(
        &self,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        dpr: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
    ) -> (Vec<DrawingGeometry>, Vec<DrawingGeometry>) {
        let mut base = Vec::new();
        let mut top = Vec::new();

        for d in &self.drawings {
            let show_anchors = matches!(
                d.state(),
                DrawingState::Selected | DrawingState::Dragging { .. }
            );
            let geom = d.generate_geometry(
                vp,
                pane_css_w,
                pane_css_h,
                dpr,
                h_pixel_ratio,
                v_pixel_ratio,
                show_anchors,
            );
            if geom.is_empty() {
                continue;
            }

            let is_hovered = self.hovered_id == Some(d.id());
            let is_active = matches!(
                d.state(),
                DrawingState::Selected
                    | DrawingState::Creating { .. }
                    | DrawingState::Dragging { .. }
            );

            if is_hovered || is_active || d.z_order() == ZOrder::Top {
                top.push(geom);
            } else {
                base.push(geom);
            }
        }

        (base, top)
    }

    /// Export all drawings to a versioned snapshot.
    pub fn snapshot(&self) -> DrawingSnapshot {
        let drawings = self
            .drawings
            .iter()
            .map(|drawing| {
                let points = if drawing.tool() == DrawingTool::Brush {
                    drawing
                        .as_any()
                        .downcast_ref::<brush::BrushDrawing>()
                        .map(|brush| {
                            brush
                                .points()
                                .iter()
                                .copied()
                                .map(SerializedDrawingPoint::from)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

                SerializedDrawing {
                    id: drawing.id(),
                    tool: drawing_tool_to_key(drawing.tool()).to_string(),
                    style: drawing.style().into(),
                    anchors: drawing
                        .anchors()
                        .iter()
                        .map(SerializedAnchorPoint::from)
                        .collect(),
                    points,
                }
            })
            .collect();

        DrawingSnapshot {
            version: DRAWINGS_SNAPSHOT_VERSION,
            drawings,
        }
    }

    /// Export drawings as a JSON string.
    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.snapshot())
    }

    /// Replace current drawings from a versioned snapshot payload.
    pub fn replace_from_snapshot(&mut self, snapshot: DrawingSnapshot) -> Result<(), String> {
        if snapshot.version > DRAWINGS_SNAPSHOT_VERSION {
            return Err(format!(
                "Unsupported drawing snapshot version {} (max supported {})",
                snapshot.version, DRAWINGS_SNAPSHOT_VERSION
            ));
        }

        self.clear();

        let mut max_id = 0_u64;
        for item in snapshot.drawings {
            let mut drawing = Self::deserialize_one(item)?;
            max_id = max_id.max(drawing.id());
            drawing.set_state(DrawingState::Idle);
            self.drawings.push(drawing);
        }

        if max_id > 0 {
            ensure_next_drawing_id_at_least(max_id + 1);
        }

        Ok(())
    }

    /// Replace current drawings from a JSON snapshot.
    pub fn replace_from_json(&mut self, json: &str) -> Result<(), String> {
        let snapshot: DrawingSnapshot =
            serde_json::from_str(json).map_err(|e| format!("Invalid drawing JSON: {e}"))?;
        self.replace_from_snapshot(snapshot)
    }

    fn deserialize_one(item: SerializedDrawing) -> Result<Box<dyn Drawing>, String> {
        let tool = drawing_tool_from_key(item.tool.as_str())
            .ok_or_else(|| format!("Unknown drawing tool '{}'", item.tool))?;
        if tool == DrawingTool::None {
            return Err("Cannot deserialize drawing with tool 'none'".to_string());
        }

        let first = item
            .anchors
            .first()
            .ok_or_else(|| format!("Drawing '{}' has no anchors", item.tool.as_str()))?;
        let mut drawing: Box<dyn Drawing> = match tool {
            DrawingTool::TrendLine => Box::new(trend_line::TrendLineDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::Rectangle => Box::new(rectangle::RectangleDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::Fibonacci => Box::new(fibonacci::FibonacciDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::Scale => Box::new(scale::ScaleDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::Brush => Box::new(brush::BrushDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::HorizontalLine => Box::new(horizontal_line::HorizontalLineDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::VerticalLine => Box::new(vertical_line::VerticalLineDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::Ray => Box::new(ray::RayDrawing::new(
                first.point.bar_index,
                first.point.price,
            )),
            DrawingTool::None => unreachable!(),
        };

        let required_anchors = drawing.required_anchors();
        if item.anchors.len() < required_anchors {
            return Err(format!(
                "Drawing '{}' has {} anchors, expected at least {}",
                item.tool.as_str(),
                item.anchors.len(),
                required_anchors
            ));
        }

        *drawing.style_mut() = item.style.into();
        *drawing.anchors_mut() = item.anchors.into_iter().map(Into::into).collect();

        if tool == DrawingTool::Brush {
            let brush = drawing
                .as_any_mut()
                .downcast_mut::<brush::BrushDrawing>()
                .ok_or_else(|| "Brush type mismatch during restore".to_string())?;
            let points = item.points.into_iter().map(Into::into).collect();
            brush.set_points(points);
        }

        if item.id > 0 {
            drawing.set_id(item.id);
        }

        Ok(drawing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::viewport::Viewport;

    fn test_viewport() -> Viewport {
        let mut vp = Viewport::new(800, 600);
        vp.start_bar = 0.0;
        vp.end_bar = 100.0;
        vp.price_min = 0.0;
        vp.price_max = 200.0;
        vp.volume_height_ratio = 0.0;
        vp
    }

    fn complete_trend_line(manager: &mut DrawingManager) -> u64 {
        manager.active_tool = DrawingTool::TrendLine;
        let id = manager.start_creating(10.0, 100.0).expect("trend line id");
        manager.finalize_creation_step(20.0, 110.0);
        id
    }

    #[test]
    fn snapshot_roundtrip_preserves_brush_points() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Brush;

        let id = manager.start_creating(10.0, 100.0).expect("brush id");
        manager.update_creation_preview(10.5, 100.5);
        manager.update_creation_preview(11.0, 101.0);
        manager.finalize_creation_step(11.5, 101.5);

        let json = manager.snapshot_json().expect("snapshot json");
        let mut restored = DrawingManager::new();
        restored
            .replace_from_json(&json)
            .expect("restore brush snapshot");

        assert_eq!(restored.len(), 1);
        let drawing = restored.get(id).expect("drawing by id");
        let brush = drawing
            .as_any()
            .downcast_ref::<brush::BrushDrawing>()
            .expect("restored brush");
        assert_eq!(brush.points().len(), 2);
    }

    #[test]
    fn snapshot_restore_bumps_next_id() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::TrendLine;

        let first_id = manager.start_creating(1.0, 10.0).expect("first id");
        manager.finalize_creation_step(2.0, 11.0);

        let snapshot = manager.snapshot();

        let mut restored = DrawingManager::new();
        restored
            .replace_from_snapshot(snapshot)
            .expect("restore snapshot");

        restored.active_tool = DrawingTool::TrendLine;
        let next_id = restored.start_creating(3.0, 12.0).expect("next id");

        assert!(next_id > first_id);
    }

    #[test]
    fn idle_non_hovered_drawing_goes_to_bottom_bucket() {
        let mut manager = DrawingManager::new();
        let id = complete_trend_line(&mut manager);
        manager.deselect_all();
        assert_eq!(manager.selected_id, None);
        assert_eq!(manager.hovered_id(), None);
        assert!(manager.get(id).is_some());

        let vp = test_viewport();
        let (bottom, top) = manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        assert_eq!(bottom.len(), 1);
        assert_eq!(top.len(), 0);
    }

    #[test]
    fn hovered_idle_drawing_is_promoted_to_top_bucket() {
        let mut manager = DrawingManager::new();
        let id = complete_trend_line(&mut manager);
        manager.deselect_all();
        manager.set_hovered(Some(id));

        let vp = test_viewport();
        let (bottom, top) = manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        assert_eq!(bottom.len(), 0);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn selected_creating_and_dragging_drawings_stay_in_top_bucket() {
        let vp = test_viewport();

        // Selected
        let mut selected_mgr = DrawingManager::new();
        complete_trend_line(&mut selected_mgr);
        let (bottom, top) = selected_mgr.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        assert_eq!(bottom.len(), 0);
        assert_eq!(top.len(), 1);

        // Creating
        let mut creating_mgr = DrawingManager::new();
        creating_mgr.active_tool = DrawingTool::TrendLine;
        creating_mgr
            .start_creating(10.0, 100.0)
            .expect("creating trend line");
        creating_mgr.update_creation_preview(20.0, 110.0);
        let (bottom, top) = creating_mgr.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        assert_eq!(bottom.len(), 0);
        assert_eq!(top.len(), 1);

        // Dragging
        let mut dragging_mgr = DrawingManager::new();
        let id = complete_trend_line(&mut dragging_mgr);
        dragging_mgr.start_drag(id, None, 20.0, 110.0);
        let (bottom, top) = dragging_mgr.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        assert_eq!(bottom.len(), 0);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn hover_state_is_transient_and_not_serialized() {
        let mut manager = DrawingManager::new();
        let id = complete_trend_line(&mut manager);
        manager.deselect_all();
        manager.set_hovered(Some(id));
        assert_eq!(manager.hovered_id(), Some(id));

        let snapshot = manager.snapshot();
        let mut restored = DrawingManager::new();
        restored
            .replace_from_snapshot(snapshot)
            .expect("restore snapshot");

        assert_eq!(restored.hovered_id(), None);
    }
}
