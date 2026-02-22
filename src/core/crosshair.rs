//! Global crosshair model with per-pane views.
//!
//! Based on LWC's `crosshair.ts` - tracks a single logical position
//! (time index + price) that renders differently in each pane:
//! - Vertical line: same time index → same X in all panes
//! - Horizontal line: only in the active pane where cursor is
//! - Price labels: show in all pane price axes (converted to each pane's scale)

use crate::core::pane::PaneId;
use crate::core::viewport::Viewport;

/// Crosshair mode.
/// X line always snaps to bar centers (LWC behavior).
/// Y line behavior depends on mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrosshairMode {
    /// Normal mode — Y follows cursor exactly.
    #[default]
    Normal,
    /// Magnet OHLC mode — Y snaps to the nearest of O, H, L, C to the cursor Y.
    MagnetOHLC,
    /// Hidden mode — crosshair not rendered.
    Hidden,
}

/// Global crosshair state.
///
/// Unlike the old implementation that stored pixel coordinates,
/// this tracks the logical position (time index + price in active pane).
/// Each pane can then convert to its own coordinate system.
#[derive(Debug, Clone)]
pub struct Crosshair {
    /// Currently active pane (where cursor is hovering).
    active_pane: Option<PaneId>,
    /// Bar/time index (shared across all panes).
    /// This is the key insight from LWC - use time index not pixel X.
    time_index: Option<usize>,
    /// Price in the active pane's coordinate system.
    price: f64,
    /// Original X coordinate (CSS pixels) for drawing.
    origin_x: f64,
    /// Original Y coordinate (CSS pixels) in active pane.
    origin_y: f64,
    /// Whether crosshair is visible.
    visible: bool,
    /// Crosshair mode.
    mode: CrosshairMode,
}

impl Default for Crosshair {
    fn default() -> Self {
        Self {
            active_pane: None,
            time_index: None,
            price: 0.0,
            origin_x: 0.0,
            origin_y: 0.0,
            visible: false,
            mode: CrosshairMode::Normal,
        }
    }
}

impl Crosshair {
    /// Create a new crosshair.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the crosshair position.
    ///
    /// # Arguments
    /// * `time_index` - Bar index (from coordinate_to_index)
    /// * `price` - Price value in the active pane's coordinate system
    /// * `pane_id` - The pane where the cursor is
    pub fn set_position(&mut self, time_index: Option<usize>, price: f64, pane_id: PaneId) {
        self.time_index = time_index;
        self.price = price;
        self.active_pane = Some(pane_id);
        self.visible = true;
    }

    /// Save the original pixel coordinates (for drawing).
    pub fn save_origin_coord(&mut self, x: f64, y: f64) {
        self.origin_x = x;
        self.origin_y = y;
    }

    /// Clear the crosshair (cursor left chart area).
    pub fn clear(&mut self) {
        self.visible = false;
        self.active_pane = None;
        self.time_index = None;
    }

    /// Set the crosshair mode.
    pub fn set_mode(&mut self, mode: CrosshairMode) {
        self.mode = mode;
    }

    /// Get the crosshair mode.
    pub fn mode(&self) -> CrosshairMode {
        self.mode
    }

    /// Check if crosshair is visible.
    pub fn visible(&self) -> bool {
        self.visible && self.mode != CrosshairMode::Hidden
    }

    /// Get the active pane (where cursor is).
    pub fn active_pane(&self) -> Option<PaneId> {
        self.active_pane
    }

    /// Get the time/bar index.
    pub fn time_index(&self) -> Option<usize> {
        self.time_index
    }

    /// Get the price (in active pane's coordinate system).
    pub fn price(&self) -> f64 {
        self.price
    }

    /// Get the original X coordinate.
    pub fn applied_x(&self) -> f64 {
        self.origin_x
    }

    /// Get the original Y coordinate.
    pub fn applied_y(&self) -> f64 {
        self.origin_y
    }

    // ── Per-Pane Visibility ──────────────────────────────────────────

    /// Check if vertical crosshair line should be visible (all panes).
    pub fn vert_line_visible(&self) -> bool {
        self.visible() && self.time_index.is_some()
    }

    /// Check if horizontal crosshair line should be visible in given pane.
    /// Only visible in the active pane where cursor actually is.
    pub fn horz_line_visible(&self, pane_id: PaneId) -> bool {
        self.visible() && self.active_pane == Some(pane_id)
    }

    /// Check if price axis label should be visible in given pane.
    ///
    /// LWC shows the label on all panes' price axes, but we simplify
    /// to only show on the active pane for now.
    pub fn price_label_visible(&self, pane_id: PaneId) -> bool {
        self.visible() && self.active_pane == Some(pane_id)
    }

    // ── Coordinate Conversion ────────────────────────────────────────

    /// Get X coordinate for the crosshair vertical line.
    ///
    /// For the active pane, returns the original X.
    /// For other panes, computes from time_index using their viewport.
    pub fn x_for_pane(&self, _pane_id: PaneId, _viewport: &Viewport, _pane_css_w: f64) -> f64 {
        // All panes share the same time axis, so X is the same everywhere.
        // We just use the stored origin_x which was computed from the active pane.
        self.origin_x
    }

    /// Get Y coordinate for the crosshair horizontal line in given pane.
    ///
    /// Only valid for the active pane - returns original Y.
    /// For other panes, returns NaN (shouldn't draw horizontal line there).
    pub fn y_for_pane(&self, pane_id: PaneId) -> f64 {
        if self.active_pane == Some(pane_id) {
            self.origin_y
        } else {
            f64::NAN
        }
    }

    /// Get price value for the crosshair label in given pane.
    ///
    /// For the active pane, returns the stored price.
    /// For other panes, would need to convert from the active pane's
    /// Y coordinate to their price scale (not implemented yet).
    pub fn price_for_pane(
        &self,
        pane_id: PaneId,
        _pane_viewport: &Viewport,
        _pane_css_h: f64,
    ) -> f64 {
        if self.active_pane == Some(pane_id) {
            self.price
        } else {
            // For cross-pane price labels, we'd convert origin_y to the
            // target pane's price. For now, return NaN.
            f64::NAN
        }
    }
}

/// Per-pane crosshair view state.
///
/// This is what gets passed to pane rendering - pre-computed coordinates
/// specific to that pane.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrosshairPaneView {
    /// Whether crosshair is visible in this pane.
    pub visible: bool,
    /// X coordinate for vertical line (CSS pixels).
    pub x: f64,
    /// Y coordinate for horizontal line (CSS pixels, NaN if not in this pane).
    pub y: f64,
    /// Whether to draw horizontal line.
    pub show_horz_line: bool,
    /// Bar index for time axis label.
    pub bar_index: Option<usize>,
    /// Price for price axis label.
    pub price: f64,
}

impl CrosshairPaneView {
    /// Create a view for a specific pane from the global crosshair.
    pub fn from_crosshair(
        crosshair: &Crosshair,
        pane_id: PaneId,
        viewport: &Viewport,
        css_w: f64,
        css_h: f64,
    ) -> Self {
        if !crosshair.visible() {
            return Self::default();
        }

        let show_horz = crosshair.horz_line_visible(pane_id);
        let y = if show_horz {
            crosshair.y_for_pane(pane_id)
        } else {
            f64::NAN
        };
        let price = crosshair.price_for_pane(pane_id, viewport, css_h);

        Self {
            visible: true,
            x: crosshair.x_for_pane(pane_id, viewport, css_w),
            y,
            show_horz_line: show_horz,
            bar_index: crosshair.time_index(),
            price,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crosshair_visibility() {
        let mut ch = Crosshair::new();
        assert!(!ch.visible());

        ch.set_position(Some(10), 100.0, PaneId::MAIN);
        ch.save_origin_coord(50.0, 30.0);
        assert!(ch.visible());
        assert!(ch.vert_line_visible());
        assert!(ch.horz_line_visible(PaneId::MAIN));
        assert!(!ch.horz_line_visible(PaneId(1))); // Different pane

        ch.clear();
        assert!(!ch.visible());
    }

    #[test]
    fn test_crosshair_hidden_mode() {
        let mut ch = Crosshair::new();
        ch.set_position(Some(10), 100.0, PaneId::MAIN);
        assert!(ch.visible());

        ch.set_mode(CrosshairMode::Hidden);
        assert!(!ch.visible());
    }
}
