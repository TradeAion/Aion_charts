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

use crate::core::data::BarArray;
use crate::core::renderer::draw_list::{DrawText, TextAlign, TextVerticalAlign};
use crate::core::renderer::value_projection::TimeScaleIndex;
use crate::core::viewport::Viewport;
use drawing::{
    ensure_next_drawing_id_at_least, line_label_placement, point_to_css, prepare_text_block,
    rect_text_anchor, rotated_text_box_top_left, Drawing, PreparedTextBlock, TEXT_DRAWING_GAP_CSS,
};
use persistence::{
    drawing_tool_from_key, drawing_tool_to_key, migrate_snapshot, DrawingSnapshot,
    SerializedAnchorPoint, SerializedDrawing, SerializedDrawingPoint, DRAWINGS_SNAPSHOT_VERSION,
};
use types::*;

/// Returns the default anchor circle fill color from the theme.
/// Used by all drawing geometry methods for consistent anchor appearance.
pub fn default_anchor_color() -> [f32; 4] {
    crate::core::renderer::theme::ThemeConfig::default()
        .drawing_defaults
        .anchor_color
}

#[derive(Debug, Clone)]
struct DrawingTextEditState {
    drawing_id: u64,
    caret: usize,
    original_value: String,
    /// Whether the caret is currently rendered (drives blink phase).
    caret_visible: bool,
    /// Timestamp (ms, monotonic-ish) of the last blink toggle or keystroke reset.
    last_blink_ms: f64,
}

impl DrawingTextEditState {
    /// Caret blink half-period in milliseconds (TradingView ~530ms).
    const BLINK_HALF_PERIOD_MS: f64 = 530.0;
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
    /// Native inline text editing state for the selected drawing.
    text_edit: Option<DrawingTextEditState>,
}

impl DrawingManager {
    const DRAWING_PLACEHOLDER: &'static str = "+ Add text";
    const LINE_LABEL_SIDE_GAP_CSS: f64 = TEXT_DRAWING_GAP_CSS;
    const RECT_OUTSIDE_GAP_CSS: f64 = TEXT_DRAWING_GAP_CSS;

    pub fn new() -> Self {
        Self {
            drawings: Vec::new(),
            active_tool: DrawingTool::None,
            selected_id: None,
            creating_id: None,
            hovered_id: None,
            text_edit: None,
        }
    }

    /// Add a drawing (already constructed).
    pub fn add(&mut self, drawing: Box<dyn Drawing>) {
        self.drawings.push(drawing);
    }

    /// Remove a drawing by ID.
    pub fn remove(&mut self, id: u64) {
        self.drawings.retain(|d| d.id() != id);
        if self
            .text_edit
            .as_ref()
            .map(|state| state.drawing_id == id)
            .unwrap_or(false)
        {
            self.text_edit = None;
        }
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

    fn drawing_text_ref(drawing: &dyn Drawing) -> Option<&DrawingText> {
        match drawing.tool() {
            DrawingTool::TrendLine => drawing
                .as_any()
                .downcast_ref::<trend_line::TrendLineDrawing>()
                .map(|d| d.text()),
            DrawingTool::Rectangle => drawing
                .as_any()
                .downcast_ref::<rectangle::RectangleDrawing>()
                .map(|d| d.text()),
            DrawingTool::HorizontalLine => drawing
                .as_any()
                .downcast_ref::<horizontal_line::HorizontalLineDrawing>()
                .map(|d| d.text()),
            DrawingTool::VerticalLine => drawing
                .as_any()
                .downcast_ref::<vertical_line::VerticalLineDrawing>()
                .map(|d| d.text()),
            DrawingTool::Ray => drawing
                .as_any()
                .downcast_ref::<ray::RayDrawing>()
                .map(|d| d.text()),
            _ => None,
        }
    }

    fn drawing_text_mut(drawing: &mut dyn Drawing) -> Option<&mut DrawingText> {
        match drawing.tool() {
            DrawingTool::TrendLine => drawing
                .as_any_mut()
                .downcast_mut::<trend_line::TrendLineDrawing>()
                .map(|d| d.text_mut()),
            DrawingTool::Rectangle => drawing
                .as_any_mut()
                .downcast_mut::<rectangle::RectangleDrawing>()
                .map(|d| d.text_mut()),
            DrawingTool::HorizontalLine => drawing
                .as_any_mut()
                .downcast_mut::<horizontal_line::HorizontalLineDrawing>()
                .map(|d| d.text_mut()),
            DrawingTool::VerticalLine => drawing
                .as_any_mut()
                .downcast_mut::<vertical_line::VerticalLineDrawing>()
                .map(|d| d.text_mut()),
            DrawingTool::Ray => drawing
                .as_any_mut()
                .downcast_mut::<ray::RayDrawing>()
                .map(|d| d.text_mut()),
            _ => None,
        }
    }

    fn drawing_text_style_ref(drawing: &dyn Drawing) -> Option<&DrawingTextStyle> {
        match drawing.tool() {
            DrawingTool::TrendLine
            | DrawingTool::Rectangle
            | DrawingTool::HorizontalLine
            | DrawingTool::VerticalLine
            | DrawingTool::Ray => Self::drawing_text_ref(drawing).map(|text| &text.style),
            DrawingTool::Fibonacci => drawing
                .as_any()
                .downcast_ref::<fibonacci::FibonacciDrawing>()
                .map(|fib| fib.label_style()),
            _ => None,
        }
    }

    fn drawing_text_style_mut(drawing: &mut dyn Drawing) -> Option<&mut DrawingTextStyle> {
        match drawing.tool() {
            DrawingTool::TrendLine
            | DrawingTool::Rectangle
            | DrawingTool::HorizontalLine
            | DrawingTool::VerticalLine
            | DrawingTool::Ray => Self::drawing_text_mut(drawing).map(|text| &mut text.style),
            DrawingTool::Fibonacci => drawing
                .as_any_mut()
                .downcast_mut::<fibonacci::FibonacciDrawing>()
                .map(|fib| fib.label_style_mut()),
            _ => None,
        }
    }

    fn drawing_supports_text(drawing: &dyn Drawing) -> bool {
        Self::drawing_text_ref(drawing).is_some()
    }

    pub fn commit_text_edit(&mut self) -> bool {
        self.text_edit.take().is_some()
    }

    fn prev_char_boundary(text: &str, caret: usize) -> usize {
        if caret == 0 {
            return 0;
        }
        text[..caret]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    fn next_char_boundary(text: &str, caret: usize) -> usize {
        if caret >= text.len() {
            return text.len();
        }
        let mut iter = text[caret..].char_indices();
        let _ = iter.next();
        iter.next()
            .map(|(offset, _)| caret + offset)
            .unwrap_or(text.len())
    }

    fn text_target_hit(target: DrawingTextEditorTarget, x: f64, y: f64) -> bool {
        let theta = target.rotation_deg.to_radians();
        let (sin_theta, cos_theta) = theta.sin_cos();
        let dx = x - target.left;
        let dy = y - target.top;
        let local_x = dx * cos_theta + dy * sin_theta;
        let local_y = -dx * sin_theta + dy * cos_theta;
        local_x >= 0.0
            && local_y >= 0.0
            && local_x <= target.width.max(0.0)
            && local_y <= target.height.max(0.0)
    }

    fn css_to_bitmap_x(x: f64, h_pixel_ratio: f64) -> f64 {
        (x + 1.0) * h_pixel_ratio - 1.0
    }

    fn css_to_bitmap_y(y: f64, v_pixel_ratio: f64) -> f64 {
        y * v_pixel_ratio
    }

    fn estimate_text_line_width(line: &str, font_size: f64) -> f64 {
        line.chars()
            .map(|ch| match ch {
                'i' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 0.32,
                ' ' => 0.33,
                'm' | 'w' | 'M' | 'W' | '@' | '#' | '%' | '&' => 0.9,
                '0'..='9' => 0.62,
                'A'..='Z' => 0.7,
                _ => 0.58,
            })
            .sum::<f64>()
            * font_size
    }

    fn text_edit_caret_local_metrics(text: &str, caret: usize, font_size: f64) -> (f64, f64, f64) {
        let mut caret = caret.min(text.len());
        while caret > 0 && !text.is_char_boundary(caret) {
            caret -= 1;
        }
        let prefix = &text[..caret];
        let mut line_count = 0usize;
        let mut current_line = "";
        for segment in prefix.split('\n') {
            current_line = segment;
            line_count += 1;
        }
        if line_count == 0 {
            line_count = 1;
        }
        let line_idx = line_count.saturating_sub(1) as f64;
        let line_height = (font_size * 1.2).max(font_size);
        let caret_x = 1.0 + Self::estimate_text_line_width(current_line, font_size);
        let top = line_idx * line_height;
        let bottom = top + line_height;
        (caret_x, top, bottom)
    }

    fn append_native_text_edit_feedback(
        &self,
        geom: &mut DrawingGeometry,
        drawing: &dyn Drawing,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
    ) {
        let Some(edit_state) = self
            .text_edit_ref()
            .filter(|state| state.drawing_id == drawing.id())
        else {
            return;
        };
        // Blink: when the caret is in its "off" phase, render nothing.
        // The host calls `tick_caret_blink` from its animation loop, which
        // toggles `caret_visible` and triggers a repaint.
        if !edit_state.caret_visible {
            return;
        }
        let Some(target) =
            Self::editor_target_for_drawing_sized(drawing, vp, pane_css_w, pane_css_h, false)
        else {
            return;
        };
        if target.width <= 0.0 || target.height <= 0.0 {
            return;
        }

        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let mut color = drawing.style().color;
        color[3] = color[3].max(0.8);
        let theta = target.rotation_deg.to_radians();
        let (sin_theta, cos_theta) = theta.sin_cos();

        let local_to_css = |lx: f64, ly: f64| -> (f64, f64) {
            (
                target.left + lx * cos_theta - ly * sin_theta,
                target.top + lx * sin_theta + ly * cos_theta,
            )
        };
        let css_to_bitmap = |x: f64, y: f64| -> (f64, f64) {
            (
                Self::css_to_bitmap_x(x, h_pixel_ratio),
                Self::css_to_bitmap_y(y, v_pixel_ratio),
            )
        };

        let Some(text) = Self::drawing_text_ref(drawing) else {
            return;
        };
        let font_size = Self::drawing_text_style_ref(drawing)
            .map(|style| style.resolved_font_size(drawing.style().font_size))
            .unwrap_or(drawing.style().font_size);
        let (caret_x, caret_top, caret_bottom) =
            Self::text_edit_caret_local_metrics(&text.value, edit_state.caret, font_size);
        let clamped_caret_x = caret_x.clamp(1.0, (target.width - 1.0).max(1.0));
        let clamped_caret_top = caret_top.clamp(0.0, target.height.max(1.0));
        let clamped_caret_bottom = caret_bottom.clamp(
            clamped_caret_top + 1.0,
            target.height.max(clamped_caret_top + 1.0),
        );
        let (caret_top_x, caret_top_y) = local_to_css(clamped_caret_x, clamped_caret_top);
        let (caret_bottom_x, caret_bottom_y) = local_to_css(clamped_caret_x, clamped_caret_bottom);
        let (caret_top_x, caret_top_y) = css_to_bitmap(caret_top_x, caret_top_y);
        let (caret_bottom_x, caret_bottom_y) = css_to_bitmap(caret_bottom_x, caret_bottom_y);
        geom.lines
            .push(crate::core::renderer::draw_list::ColoredLine {
                x0: caret_top_x as f32,
                y0: caret_top_y as f32,
                x1: caret_bottom_x as f32,
                y1: caret_bottom_y as f32,
                width: avg_ratio.max(1.0) as f32,
                r: color[0],
                g: color[1],
                b: color[2],
                a: color[3],
                dash: 0.0,
                gap: 0.0,
            });
    }

    fn append_native_placeholder(
        &self,
        geom: &mut DrawingGeometry,
        drawing: &dyn Drawing,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
    ) {
        let Some(text) = Self::drawing_text_ref(drawing) else {
            return;
        };
        if !text.is_empty() {
            return;
        }

        // While editing this drawing with no text yet, the placeholder stays
        // visible but rendered faded — it's a hint, not an obstruction. The
        // blinking caret renders on top. The moment the user types anything,
        // `text.is_empty()` becomes false and the placeholder disappears
        // (handled by the early return above).
        let is_editing_this = self
            .text_edit
            .as_ref()
            .map(|state| state.drawing_id == drawing.id())
            .unwrap_or(false);

        // When NOT editing, only show the placeholder if the drawing is the
        // user's current focus (selected or hovered). When editing, always
        // show it (faded) regardless of hover state.
        if !is_editing_this {
            let should_show =
                self.selected_id == Some(drawing.id()) || self.hovered_id == Some(drawing.id());
            if !should_show {
                return;
            }
        }

        let Some(target) = Self::editor_target_for_drawing(drawing, vp, pane_css_w, pane_css_h)
        else {
            return;
        };

        let avg_ratio = (h_pixel_ratio + v_pixel_ratio) * 0.5;
        let style = Self::drawing_text_style_ref(drawing);
        let font_size = style
            .map(|text_style| text_style.resolved_font_size(drawing.style().font_size))
            .unwrap_or(drawing.style().font_size);
        // Drawing labels are always italic — placeholder included so it visually
        // matches the typed text the user is about to enter.
        let italic = true;
        let x = ((target.left + 1.0) * h_pixel_ratio - 1.0) as f32;
        let y = (target.top * v_pixel_ratio) as f32;

        // Editing-with-empty-text: render at ~35% opacity so it reads as a
        // ghost hint behind the caret. Otherwise full color (modulo any
        // existing alpha on the drawing's color).
        let base_alpha = drawing.style().color[3];
        let alpha = if is_editing_this {
            base_alpha * 0.35
        } else {
            base_alpha
        };

        geom.texts.push(DrawText {
            text: Self::DRAWING_PLACEHOLDER.to_string(),
            x,
            y,
            font_size: (font_size * avg_ratio) as f32,
            font_weight: 600,
            italic,
            rotation_rad: target.rotation_deg.to_radians() as f32,
            r: drawing.style().color[0],
            g: drawing.style().color[1],
            b: drawing.style().color[2],
            a: alpha,
            align: TextAlign::Left,
            vertical_align: TextVerticalAlign::Top,
        });
    }

    fn text_edit_ref(&self) -> Option<&DrawingTextEditState> {
        self.text_edit.as_ref().and_then(|state| {
            (self.selected_id == Some(state.drawing_id) && self.get(state.drawing_id).is_some())
                .then_some(state)
        })
    }

    pub fn is_text_editing_selected(&self) -> bool {
        self.text_edit_ref().is_some()
    }

    pub fn begin_text_edit_selected(&mut self) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        self.begin_text_edit(id)
    }

    pub fn begin_text_edit(&mut self, id: u64) -> bool {
        let Some(original_value) = self.get(id).and_then(|drawing| {
            Self::drawing_text_ref(drawing.as_ref()).map(|text| text.value.clone())
        }) else {
            return false;
        };
        self.select(id);
        self.text_edit = Some(DrawingTextEditState {
            drawing_id: id,
            caret: original_value.len(),
            original_value,
            caret_visible: true,
            last_blink_ms: 0.0,
        });
        true
    }

    pub fn cancel_text_edit(&mut self) -> bool {
        let Some(edit_state) = self.text_edit.take() else {
            return false;
        };
        let Some(drawing) = self.get_mut(edit_state.drawing_id) else {
            return true;
        };
        let Some(text) = Self::drawing_text_mut(drawing.as_mut()) else {
            return true;
        };
        text.value = edit_state.original_value;
        true
    }

    /// Advance the caret blink phase. Call once per animation frame from the host.
    ///
    /// Returns `true` when the visible state of the caret changed (host should
    /// schedule a repaint). When no text edit is active this is a no-op returning
    /// `false`.
    pub fn tick_caret_blink(&mut self, now_ms: f64) -> bool {
        let Some(state) = self.text_edit.as_mut() else {
            return false;
        };
        // First tick after begin_text_edit / reset: prime the timer.
        if state.last_blink_ms <= 0.0 {
            state.last_blink_ms = now_ms;
            state.caret_visible = true;
            return false;
        }
        if now_ms - state.last_blink_ms >= DrawingTextEditState::BLINK_HALF_PERIOD_MS {
            state.caret_visible = !state.caret_visible;
            state.last_blink_ms = now_ms;
            return true;
        }
        false
    }

    /// Reset the blink phase so the caret is immediately visible. Called on
    /// every keystroke / caret movement so typing never hides the caret.
    fn reset_caret_blink(&mut self) {
        if let Some(state) = self.text_edit.as_mut() {
            state.caret_visible = true;
            // Setting to 0 makes the next tick re-prime against the host clock,
            // which avoids needing to thread `now_ms` into the key handler.
            state.last_blink_ms = 0.0;
        }
    }

    pub fn handle_text_key(&mut self, key: &str, ctrl: bool, alt: bool, shift: bool) -> bool {
        let is_printable = !ctrl
            && !alt
            && key.chars().count() == 1
            && !key.chars().next().is_some_and(char::is_control);

        if self.is_text_editing_selected() {
            // Any keystroke during text edit resets the blink so the caret
            // stays visible while the user is actively typing/navigating.
            self.reset_caret_blink();
            match key {
                "Escape" => return self.cancel_text_edit(),
                "Enter" if !shift => return self.commit_text_edit(),
                "Enter" => {
                    let selected_id = self.selected_id;
                    let (drawings, text_edit) = (&mut self.drawings, &mut self.text_edit);
                    if let (Some(id), Some(edit)) = (selected_id, text_edit.as_mut()) {
                        if edit.drawing_id == id {
                            if let Some(drawing) =
                                drawings.iter_mut().find(|drawing| drawing.id() == id)
                            {
                                if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
                                    text.value.insert(edit.caret, '\n');
                                    edit.caret += '\n'.len_utf8();
                                    return true;
                                }
                            }
                        }
                    }
                }
                "Backspace" => {
                    let selected_id = self.selected_id;
                    let (drawings, text_edit) = (&mut self.drawings, &mut self.text_edit);
                    if let (Some(id), Some(edit)) = (selected_id, text_edit.as_mut()) {
                        if edit.drawing_id == id {
                            if let Some(drawing) =
                                drawings.iter_mut().find(|drawing| drawing.id() == id)
                            {
                                if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
                                    if edit.caret > 0 {
                                        let prev =
                                            Self::prev_char_boundary(&text.value, edit.caret);
                                        text.value.replace_range(prev..edit.caret, "");
                                        edit.caret = prev;
                                    }
                                    return true;
                                }
                            }
                        }
                    }
                }
                "Delete" => {
                    let selected_id = self.selected_id;
                    let (drawings, text_edit) = (&mut self.drawings, &mut self.text_edit);
                    if let (Some(id), Some(edit)) = (selected_id, text_edit.as_mut()) {
                        if edit.drawing_id == id {
                            if let Some(drawing) =
                                drawings.iter_mut().find(|drawing| drawing.id() == id)
                            {
                                if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
                                    if edit.caret < text.value.len() {
                                        let next =
                                            Self::next_char_boundary(&text.value, edit.caret);
                                        text.value.replace_range(edit.caret..next, "");
                                    }
                                    return true;
                                }
                            }
                        }
                    }
                }
                "ArrowLeft" => {
                    let selected_id = self.selected_id;
                    let (drawings, text_edit) = (&mut self.drawings, &mut self.text_edit);
                    if let (Some(id), Some(edit)) = (selected_id, text_edit.as_mut()) {
                        if edit.drawing_id == id {
                            if let Some(drawing) =
                                drawings.iter_mut().find(|drawing| drawing.id() == id)
                            {
                                if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
                                    edit.caret = Self::prev_char_boundary(&text.value, edit.caret);
                                    return true;
                                }
                            }
                        }
                    }
                }
                "ArrowRight" => {
                    let selected_id = self.selected_id;
                    let (drawings, text_edit) = (&mut self.drawings, &mut self.text_edit);
                    if let (Some(id), Some(edit)) = (selected_id, text_edit.as_mut()) {
                        if edit.drawing_id == id {
                            if let Some(drawing) =
                                drawings.iter_mut().find(|drawing| drawing.id() == id)
                            {
                                if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
                                    edit.caret = Self::next_char_boundary(&text.value, edit.caret);
                                    return true;
                                }
                            }
                        }
                    }
                }
                "Home" => {
                    if let Some(edit) = self.text_edit.as_mut() {
                        edit.caret = 0;
                        return true;
                    }
                }
                "End" => {
                    let selected_id = self.selected_id;
                    let (drawings, text_edit) = (&mut self.drawings, &mut self.text_edit);
                    if let (Some(id), Some(edit)) = (selected_id, text_edit.as_mut()) {
                        if edit.drawing_id == id {
                            if let Some(drawing) =
                                drawings.iter_mut().find(|drawing| drawing.id() == id)
                            {
                                if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
                                    edit.caret = text.value.len();
                                    return true;
                                }
                            }
                        }
                    }
                }
                _ if is_printable => {
                    let selected_id = self.selected_id;
                    let (drawings, text_edit) = (&mut self.drawings, &mut self.text_edit);
                    if let (Some(id), Some(edit)) = (selected_id, text_edit.as_mut()) {
                        if edit.drawing_id == id {
                            if let Some(drawing) =
                                drawings.iter_mut().find(|drawing| drawing.id() == id)
                            {
                                if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
                                    text.value.insert_str(edit.caret, key);
                                    edit.caret += key.len();
                                    return true;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            return false;
        }

        if is_printable {
            if self.begin_text_edit_selected() {
                return self.handle_text_key(key, ctrl, alt, shift);
            }
        } else if key == "Enter" {
            return self.begin_text_edit_selected();
        }

        false
    }

    fn clamp_editor_target(
        mut target: DrawingTextEditorTarget,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> DrawingTextEditorTarget {
        target.width = target.width.max(1.0).min(pane_css_w.max(1.0));
        target.height = target.height.max(1.0).min(pane_css_h.max(1.0));
        if target.rotation_deg.abs() <= f64::EPSILON {
            target.left = target.left.clamp(0.0, (pane_css_w - target.width).max(0.0));
            target.top = target.top.clamp(0.0, (pane_css_h - target.height).max(0.0));
        }
        target
    }

    fn line_editor_target(
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        text_value: &str,
        horizontal_align: TextAlign,
        vertical_align: crate::core::renderer::draw_list::TextVerticalAlign,
        font_size: f64,
        pane_css_w: f64,
        pane_css_h: f64,
        use_placeholder_when_empty: bool,
    ) -> DrawingTextEditorTarget {
        let is_empty = text_value.trim().is_empty();
        let display_text = if is_empty && use_placeholder_when_empty {
            Self::DRAWING_PLACEHOLDER
        } else {
            text_value
        };
        let block =
            prepare_text_block(display_text, font_size as f32).unwrap_or(PreparedTextBlock {
                lines: vec![display_text.to_string()],
                line_height: (font_size as f32 * 1.2).max(font_size as f32),
                total_height: font_size as f32,
                max_width: ((display_text.len() as f64) * font_size * 0.6) as f32,
            });
        // When the actual text is empty AND we're sizing for the caret (not the
        // placeholder), collapse the editor target to a 1px-wide anchor. This
        // makes the editor target's `left` coincide exactly with where the first
        // typed character will render under any TextAlign, so the caret renders
        // at the true text-insertion point instead of being offset by the
        // placeholder's width.
        let width = if is_empty && !use_placeholder_when_empty {
            1.0
        } else {
            (block.max_width as f64 + 2.0).clamp(1.0, 320.0)
        };
        let height = (block.total_height as f64 + 2.0).clamp(1.0, 120.0);
        let placement = line_label_placement(
            start_x,
            start_y,
            end_x,
            end_y,
            horizontal_align,
            vertical_align,
            &block,
            font_size as f32,
            TEXT_DRAWING_GAP_CSS,
            Self::LINE_LABEL_SIDE_GAP_CSS,
        );
        let (left, top) = rotated_text_box_top_left(&placement, width);

        Self::clamp_editor_target(
            DrawingTextEditorTarget {
                left,
                top,
                width,
                height,
                rotation_deg: placement.rotation_rad.to_degrees() as f64,
            },
            pane_css_w,
            pane_css_h,
        )
    }

    fn editor_target_for_drawing(
        drawing: &dyn Drawing,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> Option<DrawingTextEditorTarget> {
        Self::editor_target_for_drawing_sized(drawing, vp, pane_css_w, pane_css_h, true)
    }

    /// Same as `editor_target_for_drawing` but lets the caller control whether
    /// the placeholder string is used to size the target when the actual text
    /// value is empty.
    ///
    /// - `use_placeholder_when_empty = true` (default): editor target is sized
    ///   to fit the "+ Add text" placeholder when text is empty. Used by hover
    ///   bounds, hit-testing, selection rectangles, and the placeholder render
    ///   itself.
    /// - `use_placeholder_when_empty = false`: editor target collapses to a
    ///   1px-wide anchor when text is empty so its `left` coincides exactly
    ///   with the text-render anchor for the chosen alignment. Used by the
    ///   blinking caret so it sits where the first typed character will land.
    fn editor_target_for_drawing_sized(
        drawing: &dyn Drawing,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        use_placeholder_when_empty: bool,
    ) -> Option<DrawingTextEditorTarget> {
        let text = Self::drawing_text_ref(drawing)?;
        let anchors = drawing.anchors();
        let effective_font_size = Self::drawing_text_style_ref(drawing)
            .map(|style| style.resolved_font_size(drawing.style().font_size))
            .unwrap_or(drawing.style().font_size);
        match drawing.tool() {
            DrawingTool::TrendLine | DrawingTool::Ray => {
                if anchors.len() < 2 {
                    return None;
                }
                let (x0, y0) = point_to_css(&anchors[0].point, vp, pane_css_w, pane_css_h);
                let (x1, y1) = point_to_css(&anchors[1].point, vp, pane_css_w, pane_css_h);
                Some(Self::line_editor_target(
                    x0,
                    y0,
                    x1,
                    y1,
                    &text.value,
                    text.horizontal_align,
                    text.vertical_align,
                    effective_font_size,
                    pane_css_w,
                    pane_css_h,
                    use_placeholder_when_empty,
                ))
            }
            DrawingTool::HorizontalLine => {
                if anchors.is_empty() {
                    return None;
                }
                let y = vp.price_to_css_y(anchors[0].point.price, pane_css_h);
                Some(Self::line_editor_target(
                    0.0,
                    y,
                    pane_css_w,
                    y,
                    &text.value,
                    text.horizontal_align,
                    text.vertical_align,
                    effective_font_size,
                    pane_css_w,
                    pane_css_h,
                    use_placeholder_when_empty,
                ))
            }
            DrawingTool::VerticalLine => {
                if anchors.is_empty() {
                    return None;
                }
                let (x, _) = point_to_css(&anchors[0].point, vp, pane_css_w, pane_css_h);
                let is_empty = text.value.trim().is_empty();
                let display_text = if is_empty && use_placeholder_when_empty {
                    Self::DRAWING_PLACEHOLDER
                } else {
                    text.value.as_str()
                };
                let block = prepare_text_block(display_text, effective_font_size as f32).unwrap_or(
                    PreparedTextBlock {
                        lines: vec![display_text.to_string()],
                        line_height: (effective_font_size as f32 * 1.2)
                            .max(effective_font_size as f32),
                        total_height: effective_font_size as f32,
                        max_width: ((display_text.len() as f64) * effective_font_size * 0.6) as f32,
                    },
                );
                let width = if is_empty && !use_placeholder_when_empty {
                    1.0
                } else {
                    (block.max_width as f64 + 2.0).clamp(1.0, 240.0)
                };
                let height = (block.total_height as f64 + 2.0).clamp(1.0, 120.0);
                let gap = Self::LINE_LABEL_SIDE_GAP_CSS;
                let left = match text.horizontal_align {
                    TextAlign::Left => x - width - gap,
                    TextAlign::Center => x - width * 0.5,
                    TextAlign::Right => x + gap,
                };
                let top = match text.vertical_align {
                    crate::core::renderer::draw_list::TextVerticalAlign::Top => {
                        TEXT_DRAWING_GAP_CSS
                    }
                    crate::core::renderer::draw_list::TextVerticalAlign::Middle => {
                        pane_css_h * 0.5 - height * 0.5
                    }
                    crate::core::renderer::draw_list::TextVerticalAlign::Bottom => {
                        pane_css_h - height - TEXT_DRAWING_GAP_CSS
                    }
                };
                Some(Self::clamp_editor_target(
                    DrawingTextEditorTarget {
                        left,
                        top,
                        width,
                        height,
                        rotation_deg: 0.0,
                    },
                    pane_css_w,
                    pane_css_h,
                ))
            }
            DrawingTool::Rectangle => {
                if anchors.len() < 2 {
                    return None;
                }
                let (x0, y0) = point_to_css(&anchors[0].point, vp, pane_css_w, pane_css_h);
                let (x1, y1) = point_to_css(&anchors[1].point, vp, pane_css_w, pane_css_h);
                let left = x0.min(x1);
                let top = y0.min(y1);
                let right = x0.max(x1);
                let bottom = y0.max(y1);
                let is_empty = text.value.trim().is_empty();
                let display_text = if is_empty && use_placeholder_when_empty {
                    Self::DRAWING_PLACEHOLDER
                } else {
                    text.value.as_str()
                };
                let block = prepare_text_block(display_text, effective_font_size as f32).unwrap_or(
                    PreparedTextBlock {
                        lines: vec![display_text.to_string()],
                        line_height: (effective_font_size as f32 * 1.2)
                            .max(effective_font_size as f32),
                        total_height: effective_font_size as f32,
                        max_width: ((display_text.len() as f64) * effective_font_size * 0.6) as f32,
                    },
                );
                let editor_width = if is_empty && !use_placeholder_when_empty {
                    1.0
                } else {
                    (block.max_width as f64 + 2.0).clamp(1.0, 320.0)
                };
                let editor_height = (block.total_height as f64 + 2.0).clamp(1.0, 120.0);
                let (anchor_x, anchor_y, _, vertical_align) = rect_text_anchor(
                    left,
                    top,
                    right,
                    bottom,
                    text.horizontal_align,
                    text.vertical_align,
                    TEXT_DRAWING_GAP_CSS,
                    Self::RECT_OUTSIDE_GAP_CSS,
                );
                let editor_left = match text.horizontal_align {
                    TextAlign::Left => anchor_x,
                    TextAlign::Center => anchor_x - editor_width * 0.5,
                    TextAlign::Right => anchor_x - editor_width,
                };
                let editor_top = match vertical_align {
                    crate::core::renderer::draw_list::TextVerticalAlign::Top => {
                        anchor_y - editor_height
                    }
                    crate::core::renderer::draw_list::TextVerticalAlign::Middle => {
                        anchor_y - editor_height * 0.5
                    }
                    crate::core::renderer::draw_list::TextVerticalAlign::Bottom => anchor_y,
                };
                Some(Self::clamp_editor_target(
                    DrawingTextEditorTarget {
                        left: editor_left,
                        top: editor_top,
                        width: editor_width,
                        height: editor_height,
                        rotation_deg: 0.0,
                    },
                    pane_css_w,
                    pane_css_h,
                ))
            }
            _ => None,
        }
    }

    pub fn selected_drawing_info(
        &self,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
    ) -> Option<SelectedDrawingInfo> {
        let id = self.selected_id?;
        let drawing = self.get(id)?;
        let tool = drawing.tool();
        let text = Self::drawing_text_ref(drawing.as_ref());

        let (supports_text, text_value, horizontal_align, vertical_align, editor_target) =
            if let Some(text) = text {
                (
                    true,
                    text.value.clone(),
                    text.horizontal_align.as_key().to_string(),
                    text.vertical_align.as_key().to_string(),
                    Self::editor_target_for_drawing(drawing.as_ref(), vp, pane_css_w, pane_css_h),
                )
            } else if tool == DrawingTool::Fibonacci {
                let fib = drawing
                    .as_any()
                    .downcast_ref::<fibonacci::FibonacciDrawing>()?;
                (
                    false,
                    String::new(),
                    fib.label_align().as_key().to_string(),
                    fib.label_vertical_align().as_key().to_string(),
                    None,
                )
            } else {
                (
                    false,
                    String::new(),
                    TextAlign::Center.as_key().to_string(),
                    crate::core::renderer::draw_list::TextVerticalAlign::Middle
                        .as_key()
                        .to_string(),
                    None,
                )
            };

        let (
            supports_text_style,
            text_font_size,
            text_italic,
            drawing_color,
            text_color,
            text_color_follows_drawing,
        ) = if let Some(style) = Self::drawing_text_style_ref(drawing.as_ref()) {
            (
                true,
                style.resolved_font_size(drawing.style().font_size),
                style.italic,
                rgba_to_hex(drawing.style().color),
                rgba_to_hex(style.resolved_color(drawing.style().color)),
                style.color.is_none(),
            )
        } else {
            (
                false,
                drawing.style().font_size,
                false,
                rgba_to_hex(drawing.style().color),
                rgba_to_hex(drawing.style().color),
                true,
            )
        };

        let (supports_fibonacci_levels, fibonacci_levels) = if tool == DrawingTool::Fibonacci {
            let fib = drawing
                .as_any()
                .downcast_ref::<fibonacci::FibonacciDrawing>()?;
            (true, fib.levels().to_vec())
        } else {
            (false, Vec::new())
        };

        Some(SelectedDrawingInfo {
            id,
            tool: drawing_tool_to_key(tool).to_string(),
            supports_text,
            supports_text_style,
            placeholder: "+ Add text".to_string(),
            text: text_value,
            horizontal_align,
            vertical_align,
            text_font_size,
            text_italic,
            drawing_color,
            text_color,
            text_color_follows_drawing,
            text_editing: self
                .text_edit
                .as_ref()
                .map(|state| state.drawing_id == id)
                .unwrap_or(false),
            editor_target,
            supports_fibonacci_levels,
            fibonacci_levels,
        })
    }

    pub fn set_selected_drawing_text(&mut self, text: String) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        let Some(drawing) = self.get_mut(id) else {
            return false;
        };
        let Some(drawing_text) = Self::drawing_text_mut(drawing.as_mut()) else {
            return false;
        };
        drawing_text.value = text;
        let new_len = drawing_text.value.len();
        let _ = drawing_text;
        if let Some(edit) = self.text_edit.as_mut().filter(|edit| edit.drawing_id == id) {
            edit.caret = new_len;
        }
        true
    }

    pub fn set_selected_text_alignment(
        &mut self,
        horizontal_align: TextAlign,
        vertical_align: crate::core::renderer::draw_list::TextVerticalAlign,
    ) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        let Some(drawing) = self.get_mut(id) else {
            return false;
        };

        if let Some(text) = Self::drawing_text_mut(drawing.as_mut()) {
            text.horizontal_align = horizontal_align;
            text.vertical_align = vertical_align;
            return true;
        }

        if drawing.tool() == DrawingTool::Fibonacci {
            if let Some(fib) = drawing
                .as_any_mut()
                .downcast_mut::<fibonacci::FibonacciDrawing>()
            {
                fib.set_label_align(horizontal_align);
                fib.set_label_vertical_align(vertical_align);
                return true;
            }
        }

        false
    }

    pub fn set_selected_text_style(
        &mut self,
        font_size: f64,
        italic: bool,
        color: Option<[f32; 4]>,
    ) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        let Some(drawing) = self.get_mut(id) else {
            return false;
        };
        let Some(style) = Self::drawing_text_style_mut(drawing.as_mut()) else {
            return false;
        };
        style.set_font_size(font_size);
        style.italic = italic;
        style.set_color_override(color);
        true
    }

    pub fn set_selected_fibonacci_levels(&mut self, levels: Vec<FibonacciLevel>) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        let Some(drawing) = self.get_mut(id) else {
            return false;
        };
        let Some(fib) = drawing
            .as_any_mut()
            .downcast_mut::<fibonacci::FibonacciDrawing>()
        else {
            return false;
        };
        fib.set_levels(levels);
        true
    }

    #[inline]
    fn drawing_has_hit_test_priority(&self, drawing: &dyn Drawing) -> bool {
        let is_hovered = self.hovered_id == Some(drawing.id());
        let is_active = matches!(
            drawing.state(),
            DrawingState::Selected | DrawingState::Creating { .. } | DrawingState::Dragging { .. }
        );
        is_hovered || is_active || drawing.z_order() == ZOrder::Top
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
        self.text_edit = None;
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
        self.commit_text_edit();
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
        if self.selected_id != Some(id) {
            self.commit_text_edit();
        }
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

        for top_bucket in [false, true] {
            for d in &self.drawings {
                if self.drawing_has_hit_test_priority(d.as_ref()) != top_bucket {
                    continue;
                }
                let mut result = d.hit_test(cursor_css_x, cursor_css_y, vp, pane_css_w, pane_css_h);
                if !matches!(result.part, HitPart::Anchor(_))
                    && Self::drawing_supports_text(d.as_ref())
                    && Self::drawing_text_ref(d.as_ref())
                        .map(|text| {
                            !text.is_empty()
                                || self.selected_id == Some(d.id())
                                || self.hovered_id == Some(d.id())
                                || self
                                    .text_edit
                                    .as_ref()
                                    .map(|state| state.drawing_id == d.id())
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false)
                    && Self::editor_target_for_drawing(d.as_ref(), vp, pane_css_w, pane_css_h)
                        .map(|target| Self::text_target_hit(target, cursor_css_x, cursor_css_y))
                        .unwrap_or(false)
                    && (!result.is_hit() || result.distance > 1.0)
                {
                    let label_distance = if result.is_hit() {
                        result.distance + hit_test::HIT_THRESHOLD_CSS
                    } else {
                        hit_test::HIT_THRESHOLD_CSS * 0.5
                    };
                    result = HitResult::hit(HitPart::Label, label_distance);
                }
                if result.is_hit() {
                    match &best {
                        Some((_, prev)) if prev.distance < result.distance => {}
                        _ => {
                            best = Some((d.id(), result));
                        }
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

        self.commit_text_edit();
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

    /// Get the fixed reference point for angle snapping during single-anchor drag.
    /// Returns None if not dragging a single anchor or the tool has no stable
    /// opposite point for the current handle.
    pub fn drag_opposite_anchor(&self, id: u64) -> Option<(f64, f64)> {
        let d = self.get(id)?;
        match d.state() {
            DrawingState::Dragging {
                anchor_index: Some(ai),
                ..
            } => {
                if d.tool() == DrawingTool::Rectangle {
                    let rect = d.as_any().downcast_ref::<rectangle::RectangleDrawing>()?;
                    return if rectangle::RectangleDrawing::is_corner_anchor(ai) {
                        rect.opposite_corner(ai)
                    } else {
                        None
                    };
                }

                let anchors = d.anchors();
                if anchors.len() < 2 {
                    return None;
                }
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
            // Note: we intentionally do NOT auto-enter text edit on finalize.
            // The "+ Add text" placeholder only appears when the user later
            // re-selects (or hovers) the drawing — never as a hard interruption
            // immediately after drawing. This matches what users expect: draw
            // first, decide to label later.
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
        self.commit_text_edit();
        if let Some(d) = self.get_mut(id) {
            let (initial_bar, initial_price) = (bar_index, price);

            // For rectangle corner/edge drag, pin the opposite reference for the
            // entire gesture so crossing over flips naturally instead of "pushing" sides.
            let (fixed_bar, fixed_price) = if d.tool() == DrawingTool::Rectangle {
                if let Some(ai) = anchor_index {
                    if let Some(rect) = d.as_any().downcast_ref::<rectangle::RectangleDrawing>() {
                        match rect.opposite_reference_for_anchor(ai) {
                            Some((fixed_bar, fixed_price)) => (Some(fixed_bar), Some(fixed_price)),
                            None => (None, None),
                        }
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            d.set_state(DrawingState::Dragging {
                anchor_index,
                start_bar: bar_index,
                start_price: price,
                initial_bar,
                initial_price,
                fixed_bar,
                fixed_price,
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
                    initial_bar: _initial_bar,
                    initial_price: _initial_price,
                    fixed_bar,
                    fixed_price,
                } => {
                    if let Some(ai) = anchor_index {
                        let target_bar = bar_index;
                        let target_price = price;
                        // Move single anchor.
                        if d.tool() == DrawingTool::Rectangle {
                            if rectangle::RectangleDrawing::is_corner_anchor(ai)
                                || rectangle::RectangleDrawing::is_edge_anchor(ai)
                            {
                                if let Some(rect) =
                                    d.as_any_mut().downcast_mut::<rectangle::RectangleDrawing>()
                                {
                                    rect.move_corner_with_fixed_opposite(
                                        ai,
                                        target_bar,
                                        target_price,
                                        fixed_bar.unwrap_or(target_bar),
                                        fixed_price.unwrap_or(target_price),
                                    );
                                } else {
                                    d.move_anchor(ai, target_bar, target_price);
                                }
                            } else {
                                d.move_anchor(ai, target_bar, target_price);
                            }
                        } else {
                            d.move_anchor(ai, target_bar, target_price);
                        }
                    } else {
                        // Move entire drawing
                        let delta_bar = bar_index - start_bar;
                        let delta_price = price - start_price;
                        d.move_by(delta_bar, delta_price);
                        d.set_state(DrawingState::Dragging {
                            anchor_index: None,
                            start_bar: bar_index,
                            start_price: price,
                            initial_bar: bar_index,
                            initial_price: price,
                            fixed_bar: None,
                            fixed_price: None,
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
    /// Returns `(base, overlay)` buckets; drawings now stay on the overlay bucket
    /// by default so idle drawings do not fall behind the series layer.
    pub fn generate_all_geometry(
        &self,
        vp: &Viewport,
        pane_css_w: f64,
        pane_css_h: f64,
        dpr: f64,
        h_pixel_ratio: f64,
        v_pixel_ratio: f64,
    ) -> (Vec<DrawingGeometry>, Vec<DrawingGeometry>) {
        let base = Vec::new();
        let mut top = Vec::new();

        for d in &self.drawings {
            let show_anchors = matches!(
                d.state(),
                DrawingState::Selected | DrawingState::Dragging { .. }
            );
            let mut geom = d.generate_geometry(
                vp,
                pane_css_w,
                pane_css_h,
                dpr,
                h_pixel_ratio,
                v_pixel_ratio,
                show_anchors,
            );
            self.append_native_placeholder(
                &mut geom,
                d.as_ref(),
                vp,
                pane_css_w,
                pane_css_h,
                h_pixel_ratio,
                v_pixel_ratio,
            );
            self.append_native_text_edit_feedback(
                &mut geom,
                d.as_ref(),
                vp,
                pane_css_w,
                pane_css_h,
                h_pixel_ratio,
                v_pixel_ratio,
            );
            if geom.is_empty() {
                continue;
            }

            top.push(geom);
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

                let drawing_text = Self::drawing_text_ref(drawing.as_ref());
                let drawing_text_style = Self::drawing_text_style_ref(drawing.as_ref());
                let text =
                    drawing_text.and_then(|text| (!text.is_empty()).then(|| text.value.clone()));
                let (horizontal_align, vertical_align) = if drawing.tool() == DrawingTool::Fibonacci
                {
                    drawing
                        .as_any()
                        .downcast_ref::<fibonacci::FibonacciDrawing>()
                        .map(|fib| {
                            (
                                Some(fib.label_align().as_key().to_string()),
                                Some(fib.label_vertical_align().as_key().to_string()),
                            )
                        })
                        .unwrap_or((None, None))
                } else if let Some(text) = drawing_text {
                    (
                        Some(text.horizontal_align.as_key().to_string()),
                        Some(text.vertical_align.as_key().to_string()),
                    )
                } else {
                    (None, None)
                };
                let fibonacci_levels = if drawing.tool() == DrawingTool::Fibonacci {
                    drawing
                        .as_any()
                        .downcast_ref::<fibonacci::FibonacciDrawing>()
                        .map(|fib| {
                            fib.levels()
                                .iter()
                                .map(persistence::SerializedFibonacciLevel::from)
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
                    text,
                    horizontal_align,
                    vertical_align,
                    text_font_size: drawing_text_style.and_then(|style| style.font_size),
                    text_italic: drawing_text_style.and_then(|style| style.italic.then_some(true)),
                    text_color: drawing_text_style.and_then(|style| style.color),
                    fibonacci_levels,
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

        let mut restored: Vec<Box<dyn Drawing>> = Vec::with_capacity(snapshot.drawings.len());
        let mut max_id = 0_u64;
        for item in snapshot.drawings {
            let mut drawing = Self::deserialize_one(item)?;
            max_id = max_id.max(drawing.id());
            drawing.set_state(DrawingState::Idle);
            restored.push(drawing);
        }

        self.clear();
        self.drawings = restored;

        if max_id > 0 {
            ensure_next_drawing_id_at_least(max_id + 1);
        }

        Ok(())
    }

    /// Replace current drawings from a JSON snapshot.
    pub fn replace_from_json(&mut self, json: &str) -> Result<(), String> {
        let payload: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("Invalid drawing JSON: {e}"))?;
        let snapshot = migrate_snapshot(&payload)
            .map_err(|e| format!("Invalid drawing snapshot migration: {e}"))?;
        self.replace_from_snapshot(snapshot)
    }

    /// Resolve a timestamp for a fractional logical index using the current
    /// merged time-scale.
    fn resolve_timestamp(bar_index: f64, time_scale: &TimeScaleIndex) -> Option<u64> {
        time_scale.resolve_rounded_timestamp(bar_index)
    }

    /// Fill in missing `timestamp` fields on all drawing anchor points
    /// (and brush intermediate points) using the current bar data.
    pub fn stamp_timestamps(&mut self, bars: &BarArray) {
        let time_scale = TimeScaleIndex::from_bars(bars);
        self.stamp_timestamps_with_time_scale(&time_scale);
    }

    /// Fill in missing `timestamp` fields on all drawing anchor points
    /// (and brush intermediate points) using the current merged time scale.
    pub fn stamp_timestamps_with_time_scale(&mut self, time_scale: &TimeScaleIndex) {
        for drawing in &mut self.drawings {
            for anchor in drawing.anchors_mut().iter_mut() {
                if anchor.point.timestamp.is_none() {
                    anchor.point.timestamp =
                        Self::resolve_timestamp(anchor.point.bar_index, time_scale);
                }
            }
            if drawing.tool() == DrawingTool::Brush {
                if let Some(brush) = drawing.as_any_mut().downcast_mut::<brush::BrushDrawing>() {
                    for pt in brush.points_mut().iter_mut() {
                        if pt.timestamp.is_none() {
                            pt.timestamp = Self::resolve_timestamp(pt.bar_index, time_scale);
                        }
                    }
                }
            }
        }
    }

    /// Remap all drawing positions from stored timestamps to new bar indices
    /// in the given (potentially different-timeframe) bar data.
    pub fn remap_to_new_data(&mut self, bars: &BarArray) {
        let time_scale = TimeScaleIndex::from_bars(bars);
        self.remap_to_time_scale(&time_scale);
    }

    /// Remap all drawing positions from stored timestamps to the current merged
    /// logical time scale.
    pub fn remap_to_time_scale(&mut self, time_scale: &TimeScaleIndex) {
        if time_scale.is_empty() {
            return;
        }
        for drawing in &mut self.drawings {
            // HorizontalLine only depends on price, not bar_index — skip X remap
            if drawing.tool() == DrawingTool::HorizontalLine {
                continue;
            }
            for anchor in drawing.anchors_mut().iter_mut() {
                if let Some(ts) = anchor.point.timestamp {
                    if let Some(new_idx) = time_scale.logical_index_for_timestamp(ts) {
                        anchor.point.bar_index = new_idx;
                    }
                }
            }
            if drawing.tool() == DrawingTool::Brush {
                if let Some(brush) = drawing.as_any_mut().downcast_mut::<brush::BrushDrawing>() {
                    for pt in brush.points_mut().iter_mut() {
                        if let Some(ts) = pt.timestamp {
                            if let Some(new_idx) = time_scale.logical_index_for_timestamp(ts) {
                                pt.bar_index = new_idx;
                            }
                        }
                    }
                }
            }
        }
    }

    fn deserialize_one(item: SerializedDrawing) -> Result<Box<dyn Drawing>, String> {
        let SerializedDrawing {
            id,
            tool,
            style,
            anchors,
            points,
            text,
            horizontal_align,
            vertical_align,
            text_font_size,
            text_italic,
            text_color,
            fibonacci_levels,
        } = item;

        let tool = drawing_tool_from_key(tool.as_str())
            .ok_or_else(|| format!("Unknown drawing tool '{}'", tool))?;
        if tool == DrawingTool::None {
            return Err("Cannot deserialize drawing with tool 'none'".to_string());
        }

        let first = anchors
            .first()
            .ok_or_else(|| format!("Drawing '{}' has no anchors", tool.as_api_key()))?;
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
        if anchors.len() < required_anchors {
            return Err(format!(
                "Drawing '{}' has {} anchors, expected at least {}",
                tool.as_api_key(),
                anchors.len(),
                required_anchors
            ));
        }

        *drawing.style_mut() = style.into();
        *drawing.anchors_mut() = anchors.into_iter().map(Into::into).collect();

        if tool == DrawingTool::Brush {
            let brush = drawing
                .as_any_mut()
                .downcast_mut::<brush::BrushDrawing>()
                .ok_or_else(|| "Brush type mismatch during restore".to_string())?;
            let points = points.into_iter().map(Into::into).collect();
            brush.set_points(points);
        }

        if let Some(text_style) = Self::drawing_text_style_mut(drawing.as_mut()) {
            if let Some(font_size) = text_font_size {
                text_style.set_font_size(font_size);
            }
            text_style.italic = text_italic.unwrap_or(false);
            text_style.set_color_override(text_color);
        }

        if let Some(drawing_text) = Self::drawing_text_mut(drawing.as_mut()) {
            if let Some(text) = text {
                drawing_text.value = text;
            }
            if let Some(key) = horizontal_align.as_deref().and_then(TextAlign::from_key) {
                drawing_text.horizontal_align = key;
            }
            if let Some(key) = vertical_align
                .as_deref()
                .and_then(crate::core::renderer::draw_list::TextVerticalAlign::from_key)
            {
                drawing_text.vertical_align = key;
            }
        } else if tool == DrawingTool::Fibonacci {
            let fib = drawing
                .as_any_mut()
                .downcast_mut::<fibonacci::FibonacciDrawing>()
                .ok_or_else(|| "Fibonacci type mismatch during restore".to_string())?;
            if let Some(align) = horizontal_align.as_deref().and_then(TextAlign::from_key) {
                fib.set_label_align(align);
            }
            if let Some(align) = vertical_align
                .as_deref()
                .and_then(crate::core::renderer::draw_list::TextVerticalAlign::from_key)
            {
                fib.set_label_vertical_align(align);
            }
            if !fibonacci_levels.is_empty() {
                fib.set_levels(fibonacci_levels.into_iter().map(Into::into).collect());
            }
        }

        if id > 0 {
            drawing.set_id(id);
        }

        Ok(drawing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::renderer::draw_list::TextVerticalAlign;
    use crate::core::viewport::Viewport;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9
    }

    fn contains_point(anchors: &[AnchorPoint], bar_index: f64, price: f64) -> bool {
        anchors.iter().any(|anchor| {
            approx_eq(anchor.point.bar_index, bar_index) && approx_eq(anchor.point.price, price)
        })
    }

    fn bounds(anchors: &[AnchorPoint]) -> (f64, f64, f64, f64) {
        let a = &anchors[0].point;
        let b = &anchors[1].point;
        let left = a.bar_index.min(b.bar_index);
        let right = a.bar_index.max(b.bar_index);
        let top = a.price.max(b.price);
        let bottom = a.price.min(b.price);
        (left, right, top, bottom)
    }

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
    fn native_text_editing_updates_selected_drawing_without_overlay_state() {
        let mut manager = DrawingManager::new();
        let id = complete_trend_line(&mut manager);
        assert_eq!(manager.selected_id, Some(id));

        assert!(manager.handle_text_key("D", false, false, false));
        assert!(manager.is_text_editing_selected());
        assert_eq!(
            manager
                .selected_drawing_info(&test_viewport(), 800.0, 600.0)
                .expect("selected drawing info")
                .text,
            "D"
        );

        assert!(manager.handle_text_key("e", false, false, false));
        assert!(manager.handle_text_key("v", false, false, false));
        assert!(manager.handle_text_key("Backspace", false, false, false));
        assert!(manager.handle_text_key("Enter", false, false, false));

        let info = manager
            .selected_drawing_info(&test_viewport(), 800.0, 600.0)
            .expect("selected drawing info");
        assert_eq!(info.text, "De");
        assert!(!info.text_editing);
    }

    #[test]
    fn escape_restores_original_text_during_native_text_edit() {
        let mut manager = DrawingManager::new();
        complete_trend_line(&mut manager);
        assert!(manager.set_selected_drawing_text("Dev".to_string()));
        assert!(manager.begin_text_edit_selected());
        assert!(manager.handle_text_key("X", false, false, false));
        assert!(manager.handle_text_key("Escape", false, false, false));

        let info = manager
            .selected_drawing_info(&test_viewport(), 800.0, 600.0)
            .expect("selected drawing info");
        assert_eq!(info.text, "Dev");
        assert!(!info.text_editing);
    }

    #[test]
    fn empty_middle_aligned_line_label_target_matches_typed_target_vertical_position() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::HorizontalLine;
        let id = manager
            .start_creating(12.0, 100.0)
            .expect("horizontal line id");
        manager.finalize_creation_step(12.0, 100.0);
        manager.select(id);
        assert!(manager.set_selected_text_alignment(TextAlign::Right, TextVerticalAlign::Middle));

        let vp = test_viewport();
        let empty_target = manager
            .selected_drawing_info(&vp, 800.0, 600.0)
            .and_then(|info| info.editor_target)
            .expect("empty editor target");
        assert!(manager.set_selected_drawing_text("A".to_string()));
        let typed_target = manager
            .selected_drawing_info(&vp, 800.0, 600.0)
            .and_then(|info| info.editor_target)
            .expect("typed editor target");

        assert!(
            (empty_target.top - typed_target.top).abs() <= 1.0,
            "empty target top={} typed top={}",
            empty_target.top,
            typed_target.top
        );
    }

    #[test]
    fn empty_middle_aligned_label_hit_test_prefers_label_region() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::HorizontalLine;
        manager
            .start_creating(12.0, 100.0)
            .expect("horizontal line id");
        manager.finalize_creation_step(12.0, 100.0);
        assert!(manager.set_selected_text_alignment(TextAlign::Right, TextVerticalAlign::Middle));

        let vp = test_viewport();
        let target = manager
            .selected_drawing_info(&vp, 800.0, 600.0)
            .and_then(|info| info.editor_target)
            .expect("editor target");
        let hit = manager
            .hit_test(
                target.left + target.width * 0.5,
                target.top + target.height * 0.5,
                &vp,
                800.0,
                600.0,
            )
            .expect("label hit");

        assert_eq!(hit.1.part, HitPart::Label);
    }

    #[test]
    fn text_editing_feedback_adds_native_geometry() {
        let mut manager = DrawingManager::new();
        complete_trend_line(&mut manager);
        assert!(!manager.is_text_editing_selected());
        assert!(manager.set_selected_drawing_text("Dev".to_string()));

        let vp = test_viewport();
        let (_before_bottom, before_top) =
            manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        let before_lines = before_top.first().map(|geom| geom.lines.len()).unwrap_or(0);

        assert!(manager.begin_text_edit_selected());
        let (_after_bottom, after_top) =
            manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        let after_lines = after_top.first().map(|geom| geom.lines.len()).unwrap_or(0);

        assert!(after_lines > before_lines);
    }

    #[test]
    fn finalizing_text_capable_drawing_does_not_auto_enter_text_edit_mode() {
        // Finalizing a fresh shape must NOT hijack focus into text-edit mode.
        // Users want to draw first and decide to label later. The "+ Add text"
        // affordance only shows up when the user later re-selects (or hovers)
        // the drawing — never as an immediate post-creation interruption.
        let mut manager = DrawingManager::new();
        let id = complete_trend_line(&mut manager);
        assert_eq!(manager.selected_id, Some(id));
        assert!(
            !manager.is_text_editing_selected(),
            "finalize must not auto-enter text-edit mode"
        );
    }

    #[test]
    fn caret_blink_toggles_visibility_after_half_period() {
        let mut manager = DrawingManager::new();
        complete_trend_line(&mut manager);
        assert!(manager.begin_text_edit_selected());
        assert!(manager.is_text_editing_selected());

        // Initial tick primes the timer (no flip).
        assert!(!manager.tick_caret_blink(1000.0));
        // Same frame: nothing happens.
        assert!(!manager.tick_caret_blink(1100.0));
        // Past the half-period (530ms): caret toggles off.
        assert!(manager.tick_caret_blink(1600.0));
        // Past another half-period: caret toggles back on.
        assert!(manager.tick_caret_blink(2200.0));

        // No active edit -> tick is a cheap no-op.
        manager.cancel_text_edit();
        assert!(!manager.tick_caret_blink(9999.0));
    }

    #[test]
    fn entering_text_edit_renders_caret_and_fades_placeholder() {
        // While editing with empty text: the caret renders, AND the "+ Add
        // text" placeholder stays visible but faded (alpha reduced) as a
        // ghost hint. The placeholder vanishes the instant the user types
        // anything (covered by `text.is_empty()` early return in
        // `append_native_placeholder`).
        let mut manager = DrawingManager::new();
        complete_trend_line(&mut manager);
        assert!(manager.begin_text_edit_selected());
        assert!(manager.is_text_editing_selected());

        let vp = test_viewport();
        let (_bot, top) = manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        let pane = top.first().expect("top pane geometry");

        let placeholder = pane
            .texts
            .iter()
            .find(|t| t.text == DrawingManager::DRAWING_PLACEHOLDER)
            .expect("faded placeholder must still render while editing empty text");
        assert!(
            placeholder.a < 0.6,
            "placeholder must be faded while editing (got alpha={})",
            placeholder.a
        );
        assert!(
            !pane.lines.is_empty(),
            "caret line must be rendered on the first frame of edit mode"
        );

        // Type a character -> placeholder must vanish.
        assert!(manager.handle_text_key("X", false, false, false));
        let (_b2, t2) = manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        let pane2 = t2.first().expect("top pane geometry");
        let still_present = pane2
            .texts
            .iter()
            .any(|t| t.text == DrawingManager::DRAWING_PLACEHOLDER);
        assert!(
            !still_present,
            "placeholder must disappear as soon as the user types any character"
        );
    }

    #[test]
    fn caret_position_matches_text_anchor_for_empty_rectangle_under_each_align() {
        // Regression: when text is empty and the user is editing, the caret
        // must render where the first typed character will appear, NOT where
        // the (much wider) "+ Add text" placeholder would have been centered
        // or right-aligned. Previously the editor target was sized using the
        // placeholder string, so for Center/Right alignments the caret drifted
        // tens of pixels away from the actual text-render anchor.
        let vp = test_viewport();
        let pane_w = 800.0;
        let pane_h = 600.0;

        for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
            let mut manager = DrawingManager::new();
            manager.active_tool = DrawingTool::Rectangle;
            let rect_id = manager.start_creating(10.0, 120.0).expect("rectangle id");
            manager.finalize_creation_step(20.0, 100.0);
            manager.select(rect_id);
            assert!(manager.set_selected_text_alignment(align, TextVerticalAlign::Middle));
            assert!(manager.begin_text_edit_selected());

            let drawing = manager.get(rect_id).expect("rect drawing");

            // Caret-mode editor target (no placeholder fallback): when text is
            // empty this collapses to ~1px wide so `left` is the true text
            // anchor for the chosen alignment.
            let caret_target = DrawingManager::editor_target_for_drawing_sized(
                drawing.as_ref(),
                &vp,
                pane_w,
                pane_h,
                false,
            )
            .expect("caret target");

            // Placeholder-mode target: sized using "+ Add text" so the
            // visible placeholder is centered/right-aligned correctly. Its
            // anchor (left for Left, mid for Center, right edge for Right)
            // must match the caret target's anchor.
            let placeholder_target = DrawingManager::editor_target_for_drawing_sized(
                drawing.as_ref(),
                &vp,
                pane_w,
                pane_h,
                true,
            )
            .expect("placeholder target");

            let caret_anchor = match align {
                TextAlign::Left => caret_target.left,
                TextAlign::Center => caret_target.left + caret_target.width * 0.5,
                TextAlign::Right => caret_target.left + caret_target.width,
            };
            let placeholder_anchor = match align {
                TextAlign::Left => placeholder_target.left,
                TextAlign::Center => placeholder_target.left + placeholder_target.width * 0.5,
                TextAlign::Right => placeholder_target.left + placeholder_target.width,
            };
            let drift = (caret_anchor - placeholder_anchor).abs();
            assert!(
                drift <= 2.0,
                "caret anchor must coincide with placeholder/text anchor for {:?} align (drift={})",
                align,
                drift
            );

            // The caret target must be narrow when text is empty, otherwise
            // the caret will be offset from where the first typed character
            // lands. (The placeholder target is intentionally wide.)
            assert!(
                caret_target.width <= 4.0,
                "caret-mode target should collapse to a thin anchor when text is empty for {:?} align (got width={})",
                align,
                caret_target.width
            );
        }
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
    fn snapshot_restore_is_atomic_on_error() {
        let mut manager = DrawingManager::new();
        let existing_id = complete_trend_line(&mut manager);
        manager.deselect_all();
        assert_eq!(manager.len(), 1);

        let mut invalid = manager.snapshot();
        invalid.drawings[0].tool = "not_a_real_tool".to_string();

        let err = manager
            .replace_from_snapshot(invalid)
            .expect_err("invalid snapshot should fail");
        assert!(err.contains("Unknown drawing tool"));
        assert_eq!(manager.len(), 1);
        assert!(manager.get(existing_id).is_some());
    }

    #[test]
    fn idle_non_hovered_drawing_stays_in_overlay_bucket() {
        let mut manager = DrawingManager::new();
        let id = complete_trend_line(&mut manager);
        manager.deselect_all();
        assert_eq!(manager.selected_id, None);
        assert_eq!(manager.hovered_id(), None);
        assert!(manager.get(id).is_some());

        let vp = test_viewport();
        let (bottom, top) = manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        assert_eq!(bottom.len(), 0);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn hovered_idle_drawing_stays_in_overlay_bucket() {
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
    fn selected_creating_and_dragging_drawings_stay_in_overlay_bucket() {
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
    fn hit_test_prefers_later_idle_drawing_when_identical_drawings_overlap() {
        let mut manager = DrawingManager::new();
        let first = complete_trend_line(&mut manager);
        let second = complete_trend_line(&mut manager);
        manager.deselect_all();

        let vp = test_viewport();
        let pw = 800.0;
        let ph = 600.0;
        let hit = manager
            .hit_test(
                vp.bar_to_frac(15.0) * pw,
                vp.price_to_css_y(105.0, ph),
                &vp,
                pw,
                ph,
            )
            .expect("overlap hit");

        assert_ne!(first, second);
        assert_eq!(hit.0, second);
    }

    #[test]
    fn hit_test_prefers_selected_top_bucket_drawing_over_later_idle_overlap() {
        let mut manager = DrawingManager::new();
        let first = complete_trend_line(&mut manager);
        let second = complete_trend_line(&mut manager);
        manager.deselect_all();
        manager.select(first);

        let vp = test_viewport();
        let pw = 800.0;
        let ph = 600.0;
        let hit = manager
            .hit_test(
                vp.bar_to_frac(15.0) * pw,
                vp.price_to_css_y(105.0, ph),
                &vp,
                pw,
                ph,
            )
            .expect("overlap hit");

        assert_eq!(hit.0, first);
        assert_ne!(hit.0, second);
    }

    #[test]
    fn hit_test_still_prefers_closer_drawing_when_top_bucket_is_farther() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::TrendLine;
        let lower = manager.start_creating(10.0, 100.0).expect("lower line");
        manager.finalize_creation_step(20.0, 110.0);
        manager.deselect_all();

        manager.active_tool = DrawingTool::TrendLine;
        let upper = manager.start_creating(10.0, 101.5).expect("upper line");
        manager.finalize_creation_step(20.0, 111.5);
        manager.deselect_all();
        manager.select(upper);

        let vp = test_viewport();
        let pw = 800.0;
        let ph = 600.0;
        let hit = manager
            .hit_test(
                vp.bar_to_frac(15.0) * pw,
                vp.price_to_css_y(105.0, ph),
                &vp,
                pw,
                ph,
            )
            .expect("nearby overlap hit");

        assert_eq!(hit.0, lower);
        assert_ne!(hit.0, upper);
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

    #[test]
    fn rectangle_corner_drag_keeps_opposite_corner_fixed_when_crossing() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        let id = manager.start_creating(10.0, 110.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 100.0);

        // Cross horizontal axis only: opposite corner must remain pinned exactly.
        manager.start_drag(id, Some(0), 10.0, 110.0);
        manager.update_drag(id, 25.0, 105.0);

        let drawing = manager.get(id).expect("rectangle drawing");
        let anchors = drawing.anchors();
        assert!(contains_point(anchors, 20.0, 100.0));

        // Drag top-left corner and cross over both axes.
        manager.update_drag(id, 25.0, 95.0);

        let drawing = manager.get(id).expect("rectangle drawing");
        let anchors = drawing.anchors();
        let (left, right, top, bottom) = bounds(anchors);
        assert!(approx_eq(left, 20.0));
        assert!(approx_eq(right, 25.0));
        assert!(approx_eq(top, 100.0));
        assert!(approx_eq(bottom, 95.0));
        assert!(contains_point(anchors, 20.0, 100.0));

        // Continue dragging back across to the opposite side.
        manager.update_drag(id, 5.0, 120.0);

        let drawing = manager.get(id).expect("rectangle drawing");
        let anchors = drawing.anchors();
        let (left, right, top, bottom) = bounds(anchors);
        assert!(approx_eq(left, 5.0));
        assert!(approx_eq(right, 20.0));
        assert!(approx_eq(top, 120.0));
        assert!(approx_eq(bottom, 100.0));
        assert!(contains_point(anchors, 20.0, 100.0));
    }

    #[test]
    fn rectangle_selected_geometry_renders_eight_resize_handles() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        manager.start_creating(10.0, 110.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 100.0);

        let vp = test_viewport();
        let (_bottom, top) = manager.generate_all_geometry(&vp, 800.0, 600.0, 1.0, 1.0, 1.0);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].anchors.len(), 8);
    }

    #[test]
    fn rectangle_left_edge_drag_keeps_opposite_edge_fixed_after_crossing() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        let id = manager.start_creating(10.0, 110.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 100.0);

        // Left midpoint handle index = 7.
        manager.start_drag(id, Some(7), 10.0, 105.0);
        manager.update_drag(id, 25.0, 105.0);

        let drawing = manager.get(id).expect("rectangle drawing");
        let anchors = drawing.anchors();
        let (left, _right, _top, _bottom) = bounds(anchors);
        assert!(approx_eq(left, 20.0));

        // Continue further to the right; right side must remain pinned.
        manager.update_drag(id, 26.0, 105.0);
        let drawing = manager.get(id).expect("rectangle drawing");
        let anchors = drawing.anchors();
        let (left, _right, _top, _bottom) = bounds(anchors);
        assert!(approx_eq(left, 20.0));
    }

    #[test]
    fn rectangle_top_edge_hit_and_drag_flips_with_bottom_fixed() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        let id = manager.start_creating(10.0, 110.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 100.0);

        let vp = test_viewport();
        let pw = 800.0;
        let ph = 600.0;

        // Cursor on top border center (not on top-mid anchor point).
        let edge_x = vp.bar_to_frac(14.0) * pw;
        let edge_y = vp.price_to_css_y(110.0, ph);
        let (_hit_id, hit) = manager
            .hit_test(edge_x, edge_y, &vp, pw, ph)
            .expect("edge hit");

        // Edge should map to top-mid anchor drag path.
        assert_eq!(hit.part, HitPart::Anchor(4));

        manager.start_drag(id, Some(4), 14.0, 110.0);
        manager.update_drag(id, 14.0, 95.0);

        let drawing = manager.get(id).expect("rectangle drawing");
        let anchors = drawing.anchors();
        let (_left, _right, top, bottom) = bounds(anchors);
        // Original bottom (100) stays fixed while dragged top crosses below and flips.
        assert!(approx_eq(top, 100.0));
        assert!(approx_eq(bottom, 95.0));
    }

    #[test]
    fn anchor_drag_tracks_cursor_exactly_for_existing_drawings() {
        let mut manager = DrawingManager::new();
        let id = complete_trend_line(&mut manager);

        // Grab the first anchor with an intentional offset from its true position.
        manager.start_drag(id, Some(0), 12.0, 102.0);
        manager.update_drag(id, 14.0, 105.0);

        let drawing = manager.get(id).expect("trend line drawing");
        let first_anchor = drawing.anchors()[0].point;
        assert!(approx_eq(first_anchor.bar_index, 14.0));
        assert!(approx_eq(first_anchor.price, 105.0));
    }

    #[test]
    fn rectangle_corner_drag_uses_normalized_handle_position() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        let id = manager.start_creating(10.0, 100.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 110.0);

        // Drag the normalized top-left handle. The stored logical anchors are
        // bottom-left + top-right here, so using raw anchor[0] would jump.
        manager.start_drag(id, Some(0), 10.0, 110.0);
        manager.update_drag(id, 12.0, 108.0);

        let drawing = manager.get(id).expect("rectangle drawing");
        let anchors = drawing.anchors();
        let (left, right, top, bottom) = bounds(anchors);
        assert!(approx_eq(left, 12.0));
        assert!(approx_eq(right, 20.0));
        assert!(approx_eq(top, 108.0));
        assert!(approx_eq(bottom, 100.0));
    }

    #[test]
    fn rectangle_drag_opposite_anchor_uses_normalized_opposite_corner() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        let id = manager.start_creating(10.0, 100.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 110.0);

        manager.start_drag(id, Some(0), 10.0, 110.0);

        assert_eq!(manager.drag_opposite_anchor(id), Some((20.0, 100.0)));
    }

    #[test]
    fn snapshot_roundtrip_preserves_drawing_text_and_fibonacci_metadata() {
        let mut manager = DrawingManager::new();

        manager.active_tool = DrawingTool::TrendLine;
        let line_id = manager.start_creating(10.0, 100.0).expect("line id");
        manager.finalize_creation_step(20.0, 110.0);
        manager.select(line_id);
        assert!(manager.set_selected_drawing_text("Dev".to_string()));
        assert!(manager.set_selected_text_alignment(TextAlign::Right, TextVerticalAlign::Bottom));
        assert!(manager.set_selected_text_style(17.0, true, Some([0.9, 0.4, 0.1, 1.0]),));

        manager.active_tool = DrawingTool::Fibonacci;
        let fib_id = manager.start_creating(12.0, 120.0).expect("fib id");
        manager.finalize_creation_step(22.0, 90.0);
        manager.select(fib_id);
        assert!(manager.set_selected_text_alignment(TextAlign::Left, TextVerticalAlign::Middle));
        assert!(manager.set_selected_text_style(14.0, true, Some([0.25, 0.75, 0.5, 1.0]),));
        assert!(manager.set_selected_fibonacci_levels(vec![
            FibonacciLevel::new(0.0, "Start"),
            FibonacciLevel::new(0.5, "Mid"),
            FibonacciLevel::new(1.0, "End"),
        ]));

        let snapshot = manager.snapshot();
        let mut restored = DrawingManager::new();
        restored
            .replace_from_snapshot(snapshot)
            .expect("restore snapshot with text");

        let line = restored.get(line_id).expect("restored line");
        let line = line
            .as_any()
            .downcast_ref::<trend_line::TrendLineDrawing>()
            .expect("trend line");
        assert_eq!(line.text().value, "Dev");
        assert_eq!(line.text().horizontal_align, TextAlign::Right);
        assert_eq!(line.text().vertical_align, TextVerticalAlign::Bottom);
        assert_eq!(line.text().style.font_size, Some(17.0));
        assert!(line.text().style.italic);
        assert_eq!(line.text().style.color, Some([0.9, 0.4, 0.1, 1.0]));

        let fib = restored.get(fib_id).expect("restored fib");
        let fib = fib
            .as_any()
            .downcast_ref::<fibonacci::FibonacciDrawing>()
            .expect("fibonacci");
        assert_eq!(fib.label_align(), TextAlign::Left);
        assert_eq!(fib.label_vertical_align(), TextVerticalAlign::Middle);
        assert_eq!(fib.label_style().font_size, Some(14.0));
        assert!(fib.label_style().italic);
        assert_eq!(fib.label_style().color, Some([0.25, 0.75, 0.5, 1.0]));
        assert_eq!(fib.levels().len(), 3);
        assert_eq!(fib.levels()[1].ratio, 0.5);
        assert_eq!(fib.levels()[1].label, "Mid");
    }

    #[test]
    fn selected_rectangle_info_exposes_text_editor_target_and_alignment() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        let rect_id = manager.start_creating(10.0, 120.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 100.0);
        manager.select(rect_id);
        assert!(manager.set_selected_drawing_text("Note".to_string()));
        assert!(manager.set_selected_text_alignment(TextAlign::Left, TextVerticalAlign::Top));

        let info = manager
            .selected_drawing_info(&test_viewport(), 800.0, 600.0)
            .expect("selected drawing info");

        assert_eq!(info.id, rect_id);
        assert_eq!(info.tool, "rectangle");
        assert!(info.supports_text);
        assert!(info.supports_text_style);
        assert_eq!(info.text, "Note");
        assert_eq!(info.horizontal_align, "left");
        assert_eq!(info.vertical_align, "top");
        assert!(info.text_color_follows_drawing);
        let editor = info.editor_target.expect("editor target");
        assert!(editor.width >= 20.0);
        assert!(editor.height >= 10.0);
    }

    #[test]
    fn rectangle_text_vertical_alignment_top_bottom_outside_middle_inside() {
        let mut manager = DrawingManager::new();
        manager.active_tool = DrawingTool::Rectangle;
        let rect_id = manager.start_creating(10.0, 120.0).expect("rectangle id");
        manager.finalize_creation_step(20.0, 100.0);
        manager.select(rect_id);
        assert!(manager.set_selected_drawing_text("Note".to_string()));

        let vp = test_viewport();
        let drawing = manager.get(rect_id).expect("rectangle");
        let anchors = drawing.anchors();
        let (_x0, y0) = point_to_css(&anchors[0].point, &vp, 800.0, 600.0);
        let (_x1, y1) = point_to_css(&anchors[1].point, &vp, 800.0, 600.0);
        let rect_top = y0.min(y1);
        let rect_bottom = y0.max(y1);

        assert!(manager.set_selected_text_alignment(TextAlign::Center, TextVerticalAlign::Top));
        let top_target = manager
            .selected_drawing_info(&vp, 800.0, 600.0)
            .and_then(|info| info.editor_target)
            .expect("top target");
        assert!(
            top_target.top + top_target.height <= rect_top + 1.0,
            "top target should stay outside top edge"
        );

        assert!(manager.set_selected_text_alignment(TextAlign::Center, TextVerticalAlign::Middle));
        let middle_target = manager
            .selected_drawing_info(&vp, 800.0, 600.0)
            .and_then(|info| info.editor_target)
            .expect("middle target");
        assert!(
            middle_target.top >= rect_top - 1.0
                && middle_target.top + middle_target.height <= rect_bottom + 1.0,
            "middle target should stay inside rectangle"
        );

        assert!(manager.set_selected_text_alignment(TextAlign::Center, TextVerticalAlign::Bottom));
        let bottom_target = manager
            .selected_drawing_info(&vp, 800.0, 600.0)
            .and_then(|info| info.editor_target)
            .expect("bottom target");
        assert!(
            bottom_target.top >= rect_bottom - 1.0,
            "bottom target should stay outside bottom edge"
        );
    }
}
