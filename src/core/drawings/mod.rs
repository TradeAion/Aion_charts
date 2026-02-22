//! Drawings subsystem — trend lines, fib retracements, rectangles, scale tools.
//!
//! Architecture:
//! - `types.rs`: shared types (DrawingPoint, AnchorPoint, DrawingState, etc.)
//! - `drawing.rs`: Drawing trait all tools implement
//! - `hit_test.rs`: geometric hit-test math
//! - `trend_line.rs`, `rectangle.rs`, `fibonacci.rs`, `scale.rs`: concrete tools
//! - `DrawingManager` (this file): owns all drawings, dispatches hit-tests, manages active tool

pub mod types;
pub mod drawing;
pub mod hit_test;
pub mod trend_line;
pub mod rectangle;
pub mod fibonacci;
pub mod scale;

use crate::core::viewport::Viewport;
use types::*;
use drawing::Drawing;

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
}

impl DrawingManager {
    pub fn new() -> Self {
        Self {
            drawings: Vec::new(),
            active_tool: DrawingTool::None,
            selected_id: None,
            creating_id: None,
        }
    }

    /// Add a drawing (already constructed).
    pub fn add(&mut self, drawing: Box<dyn Drawing>) {
        self.drawings.push(drawing);
    }

    /// Remove a drawing by ID.
    pub fn remove(&mut self, id: u64) {
        self.drawings.retain(|d| d.id() != id);
        if self.selected_id == Some(id) { self.selected_id = None; }
        if self.creating_id == Some(id) { self.creating_id = None; }
    }

    /// Remove the currently selected drawing.
    pub fn remove_selected(&mut self) {
        if let Some(id) = self.selected_id.take() {
            self.remove(id);
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
                    _ => { best = Some((d.id(), result)); }
                }
            }
        }

        best
    }

    /// Start creating a new drawing with the active tool.
    /// Returns the ID of the new drawing, or None if no tool is active.
    pub fn start_creating(&mut self, bar_index: f64, price: f64) -> Option<u64> {
        let tool = self.active_tool;
        if tool == DrawingTool::None { return None; }

        self.deselect_all();

        let drawing: Box<dyn Drawing> = match tool {
            DrawingTool::TrendLine => Box::new(trend_line::TrendLineDrawing::new(bar_index, price)),
            DrawingTool::Rectangle => Box::new(rectangle::RectangleDrawing::new(bar_index, price)),
            DrawingTool::Fibonacci => Box::new(fibonacci::FibonacciDrawing::new(bar_index, price)),
            DrawingTool::Scale => Box::new(scale::ScaleDrawing::new(bar_index, price)),
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
            self.creating_id = None;
            self.selected_id = Some(id);
            if let Some(d) = self.drawings.iter_mut().find(|d| d.id() == id) {
                d.set_state(DrawingState::Selected);
            }
            self.active_tool = DrawingTool::None;
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
                DrawingState::Dragging { anchor_index, start_bar, start_price } => {
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
    /// Splits into base-layer (Idle/Selected) and top-layer (Creating/Dragging).
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
            let show_anchors = matches!(d.state(), DrawingState::Selected | DrawingState::Dragging { .. });
            let geom = d.generate_geometry(vp, pane_css_w, pane_css_h, dpr, h_pixel_ratio, v_pixel_ratio, show_anchors);
            if geom.is_empty() { continue; }

            match d.z_order() {
                ZOrder::Top => top.push(geom),
                _ => base.push(geom),
            }
        }

        (base, top)
    }
}
