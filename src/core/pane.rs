//! Pane — independent chart pane with its own price scale.
//!
//! Multi-pane architecture matching LWC:
//! - Each pane has its own viewport (price range), series, and price axis
//! - Panes share a common time axis (horizontal scroll is synchronized)
//! - Stretch factors control proportional vertical sizing
//! - Main pane (index 0) contains the primary candlestick data
//! - Sub-panes (index 1+) contain indicators like RSI, MACD

use crate::core::constants::{
    MAIN_PANE_STRETCH_FACTOR, MIN_INDICATOR_PANE_HEIGHT_CSS, MIN_MAIN_PANE_HEIGHT_CSS,
    PANE_SEPARATOR_HEIGHT_CSS,
};
use crate::core::series::SeriesCollection;
use crate::core::viewport::{PriceScaleMode, Viewport};

/// Unique identifier for a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub u32);

impl PaneId {
    /// The main chart pane (always exists).
    pub const MAIN: PaneId = PaneId(0);
}

/// Configuration for a pane.
#[derive(Debug, Clone)]
pub struct PaneOptions {
    /// Vertical stretch factor (relative to other panes).
    /// Default is 1.0. Main pane typically has higher factor (e.g., 3.0).
    pub stretch_factor: f64,
    /// Whether this pane shows volume bars.
    pub show_volume: bool,
    /// Price scale mode for this pane.
    pub price_scale_mode: PriceScaleMode,
    /// Minimum height in CSS pixels (prevents pane from being too small).
    pub min_height: f64,
}

impl Default for PaneOptions {
    fn default() -> Self {
        Self {
            stretch_factor: 1.0,
            show_volume: false,
            price_scale_mode: PriceScaleMode::Normal,
            min_height: MIN_INDICATOR_PANE_HEIGHT_CSS,
        }
    }
}

impl PaneOptions {
    /// Options for the main chart pane.
    pub fn main() -> Self {
        Self {
            stretch_factor: MAIN_PANE_STRETCH_FACTOR,
            show_volume: true,
            price_scale_mode: PriceScaleMode::Normal,
            min_height: MIN_MAIN_PANE_HEIGHT_CSS,
        }
    }

    /// Options for an indicator sub-pane.
    pub fn indicator() -> Self {
        Self {
            stretch_factor: 1.0,
            show_volume: false,
            price_scale_mode: PriceScaleMode::Normal,
            min_height: MIN_INDICATOR_PANE_HEIGHT_CSS,
        }
    }
}

/// A single chart pane with independent price scale.
pub struct Pane {
    pub id: PaneId,
    pub options: PaneOptions,
    /// Viewport for this pane (owns price_min/max, shares time range via sync).
    pub viewport: Viewport,
    /// Series displayed in this pane.
    pub series: SeriesCollection,
    /// Height in CSS pixels (computed from stretch factor).
    pub height_css: f64,
    /// Study IDs attached to this pane.
    pub study_ids: Vec<u32>,
}

impl Pane {
    pub fn new(id: PaneId, options: PaneOptions, width: u32, height: u32) -> Self {
        let mut viewport = Viewport::new(width, height);
        viewport.price_scale_mode = options.price_scale_mode;

        Self {
            id,
            options,
            viewport,
            series: SeriesCollection::new(),
            height_css: 0.0,
            study_ids: Vec::new(),
        }
    }

    /// Check if this is the main pane.
    #[inline]
    pub fn is_main(&self) -> bool {
        self.id == PaneId::MAIN
    }

    /// Sync time range from another viewport (for shared horizontal scroll).
    pub fn sync_time_range(&mut self, source: &Viewport) {
        self.viewport.start_bar = source.start_bar;
        self.viewport.end_bar = source.end_bar;
    }

    /// Update the price range based on visible series data.
    pub fn auto_scale_price(&mut self) {
        // This will be called to auto-fit price range to visible data
        // Implementation depends on what series are in this pane
        self.viewport.price_invalidated = true;
    }
}

/// Manages multiple panes in a chart.
pub struct PaneManager {
    panes: Vec<Pane>,
    next_id: u32,
    /// Total available height in CSS pixels.
    total_height: f64,
}

impl PaneManager {
    pub fn new() -> Self {
        Self {
            panes: Vec::new(),
            next_id: 1, // 0 is reserved for main
            total_height: 0.0,
        }
    }

    /// Initialize with main pane.
    pub fn init_main(&mut self, width: u32, height: u32) -> PaneId {
        let main = Pane::new(PaneId::MAIN, PaneOptions::main(), width, height);
        self.panes.push(main);
        PaneId::MAIN
    }

    /// Add a new sub-pane. Returns the pane ID.
    pub fn add_pane(&mut self, options: PaneOptions, width: u32, height: u32) -> PaneId {
        let id = PaneId(self.next_id);
        self.next_id += 1;
        let pane = Pane::new(id, options, width, height);
        self.panes.push(pane);
        self.recompute_heights();
        id
    }

    /// Remove a pane by ID. Cannot remove the main pane.
    pub fn remove_pane(&mut self, id: PaneId) -> bool {
        if id == PaneId::MAIN {
            return false; // Cannot remove main pane
        }
        if let Some(pos) = self.panes.iter().position(|p| p.id == id) {
            self.panes.remove(pos);
            self.recompute_heights();
            true
        } else {
            false
        }
    }

    /// Get a pane by ID.
    pub fn get(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Get a mutable pane by ID.
    pub fn get_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// Get the main pane.
    pub fn main(&self) -> Option<&Pane> {
        self.get(PaneId::MAIN)
    }

    /// Get the main pane mutably.
    pub fn main_mut(&mut self) -> Option<&mut Pane> {
        self.get_mut(PaneId::MAIN)
    }

    /// Iterate over all panes (top to bottom).
    pub fn iter(&self) -> impl Iterator<Item = &Pane> {
        self.panes.iter()
    }

    /// Iterate mutably over all panes.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pane> {
        self.panes.iter_mut()
    }

    /// Number of panes.
    pub fn len(&self) -> usize {
        self.panes.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Set total available height and recompute pane heights.
    pub fn set_total_height(&mut self, height: f64) {
        self.total_height = height;
        self.recompute_heights();
    }

    /// Recompute pane heights based on stretch factors.
    fn recompute_heights(&mut self) {
        if self.panes.is_empty() || self.total_height <= 0.0 {
            return;
        }

        // Account for separators (N-1 separators for N panes)
        let num_separators = self.panes.len().saturating_sub(1);
        let available = self.total_height - (num_separators as f64 * PANE_SEPARATOR_HEIGHT_CSS);

        // Sum of all stretch factors
        let total_stretch: f64 = self.panes.iter().map(|p| p.options.stretch_factor).sum();

        if total_stretch <= 0.0 {
            return;
        }

        // Distribute height proportionally
        for pane in &mut self.panes {
            let frac = pane.options.stretch_factor / total_stretch;
            let height = (available * frac).max(pane.options.min_height);
            pane.height_css = height;
        }
    }

    /// Sync time range from main pane to all sub-panes.
    pub fn sync_time_range(&mut self) {
        if self.panes.len() < 2 {
            return;
        }

        // Get main pane's time range
        let (start, end) = if let Some(main) = self.main() {
            (main.viewport.start_bar, main.viewport.end_bar)
        } else {
            return;
        };

        // Apply to all sub-panes
        for pane in &mut self.panes {
            if pane.id != PaneId::MAIN {
                pane.viewport.start_bar = start;
                pane.viewport.end_bar = end;
            }
        }
    }

    /// Get cumulative Y offset for a pane (for hit-testing).
    pub fn pane_y_offset(&self, id: PaneId) -> f64 {
        let mut offset = 0.0;

        for pane in &self.panes {
            if pane.id == id {
                return offset;
            }
            offset += pane.height_css + PANE_SEPARATOR_HEIGHT_CSS;
        }

        offset
    }

    /// Find which pane contains a given Y coordinate.
    pub fn pane_at_y(&self, y: f64) -> Option<PaneId> {
        let mut cumulative = 0.0;

        for pane in &self.panes {
            let top = cumulative;
            let bottom = cumulative + pane.height_css;

            if y >= top && y < bottom {
                return Some(pane.id);
            }

            cumulative = bottom + PANE_SEPARATOR_HEIGHT_CSS;
        }

        None
    }

    /// Check if Y coordinate is on a separator (for resize dragging).
    /// Returns the index of the separator (0 = between pane 0 and 1).
    pub fn separator_at_y(&self, y: f64) -> Option<usize> {
        let mut cumulative = 0.0;

        for (i, pane) in self.panes.iter().enumerate() {
            cumulative += pane.height_css;

            // Check if in separator zone
            if y >= cumulative && y < cumulative + PANE_SEPARATOR_HEIGHT_CSS {
                return Some(i);
            }

            cumulative += PANE_SEPARATOR_HEIGHT_CSS;
        }

        None
    }

    /// Resize panes by dragging a separator.
    /// `separator_idx`: which separator (0 = between pane 0 and 1)
    /// `delta_y`: how much to move the separator (positive = down)
    pub fn drag_separator(&mut self, separator_idx: usize, delta_y: f64) {
        if separator_idx >= self.panes.len().saturating_sub(1) {
            return;
        }

        let top_pane = separator_idx;
        let bottom_pane = separator_idx + 1;

        // Get current heights
        let top_height = self.panes[top_pane].height_css;
        let bottom_height = self.panes[bottom_pane].height_css;
        let top_min = self.panes[top_pane].options.min_height;
        let bottom_min = self.panes[bottom_pane].options.min_height;

        // Compute new heights respecting minimums
        let new_top = (top_height + delta_y).max(top_min);
        let _new_bottom = (bottom_height - delta_y).max(bottom_min);

        // Only apply if both are valid
        let actual_delta = new_top - top_height;
        if (bottom_height - actual_delta) >= bottom_min {
            self.panes[top_pane].height_css = new_top;
            self.panes[bottom_pane].height_css = bottom_height - actual_delta;

            // Update stretch factors to match new proportions
            let total = new_top + (bottom_height - actual_delta);
            if total > 0.0 {
                self.panes[top_pane].options.stretch_factor = new_top / total * 2.0;
                self.panes[bottom_pane].options.stretch_factor =
                    (bottom_height - actual_delta) / total * 2.0;
            }
        }
    }
}

impl Default for PaneManager {
    fn default() -> Self {
        Self::new()
    }
}
