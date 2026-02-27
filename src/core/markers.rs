//! SeriesMarker — visual markers positioned at specific bar indices.
//!
//! Matches LWC's Series.setMarkers() API:
//! - Shapes: arrowUp, arrowDown, circle, square
//! - Positioned at bar time + price level (above/below/inBar)
//! - Optional text label below/above marker
//! - Two-pass rendering for circles (border ring, then fill)

use std::collections::HashMap;

/// Shape type for a series marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerShape {
    /// Upward pointing arrow (bullish signal).
    ArrowUp,
    /// Downward pointing arrow (bearish signal).
    ArrowDown,
    /// Filled circle.
    Circle,
    /// Filled square.
    Square,
}

impl MarkerShape {
    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "arrowup" | "arrow_up" => MarkerShape::ArrowUp,
            "arrowdown" | "arrow_down" => MarkerShape::ArrowDown,
            "circle" => MarkerShape::Circle,
            "square" => MarkerShape::Square,
            _ => MarkerShape::Circle, // default
        }
    }
}

/// Vertical position of the marker relative to the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerPosition {
    /// Above the bar's high price.
    AboveBar,
    /// Below the bar's low price.
    BelowBar,
    /// At a specific price level.
    AtPrice,
}

impl MarkerPosition {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "abovebar" | "above_bar" | "above" => MarkerPosition::AboveBar,
            "belowbar" | "below_bar" | "below" => MarkerPosition::BelowBar,
            "atprice" | "at_price" | "inbar" | "in_bar" => MarkerPosition::AtPrice,
            _ => MarkerPosition::AboveBar,
        }
    }
}

/// A single series marker instance.
#[derive(Debug, Clone)]
pub struct SeriesMarker {
    /// Bar index (0-based into bars array).
    pub bar_index: usize,
    /// Marker shape.
    pub shape: MarkerShape,
    /// Vertical position.
    pub position: MarkerPosition,
    /// Price level (used when position = AtPrice, otherwise ignored).
    pub price: f64,
    /// Marker color [R, G, B, A] in 0.0–1.0 range.
    pub color: [f32; 4],
    /// Marker size in CSS pixels (radius for circle, half-width for square/arrow).
    pub size: f64,
    /// Optional text label.
    pub text: String,
    /// Text color [R, G, B, A].
    pub text_color: [f32; 4],
    /// Unique identifier.
    pub id: u32,
}

impl Default for SeriesMarker {
    fn default() -> Self {
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        Self {
            bar_index: 0,
            shape: MarkerShape::Circle,
            position: MarkerPosition::AboveBar,
            price: 0.0,
            color: theme.series_defaults.marker_color,
            size: 6.0,
            text: String::new(),
            text_color: theme.series_defaults.marker_text_color,
            id: 0,
        }
    }
}

/// Manages markers for a single series.
#[derive(Debug, Clone)]
pub struct SeriesMarkers {
    markers: Vec<SeriesMarker>,
    next_id: u32,
}

impl Default for SeriesMarkers {
    fn default() -> Self {
        Self::new()
    }
}

impl SeriesMarkers {
    pub fn new() -> Self {
        Self {
            markers: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a marker. Returns the assigned ID.
    pub fn add(&mut self, mut marker: SeriesMarker) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        marker.id = id;
        self.markers.push(marker);
        id
    }

    /// Remove a marker by ID. Returns true if found.
    pub fn remove(&mut self, id: u32) -> bool {
        if let Some(pos) = self.markers.iter().position(|m| m.id == id) {
            self.markers.remove(pos);
            true
        } else {
            false
        }
    }

    /// Clear all markers.
    pub fn clear(&mut self) {
        self.markers.clear();
    }

    /// Set all markers at once (replaces existing).
    pub fn set(&mut self, markers: Vec<SeriesMarker>) {
        self.markers = markers;
        // Assign IDs
        for (i, m) in self.markers.iter_mut().enumerate() {
            m.id = (i + 1) as u32;
        }
        self.next_id = self.markers.len() as u32 + 1;
    }

    /// Iterate over all markers.
    pub fn iter(&self) -> impl Iterator<Item = &SeriesMarker> {
        self.markers.iter()
    }

    /// Number of markers.
    pub fn len(&self) -> usize {
        self.markers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }

    /// Get markers visible in a bar index range.
    pub fn in_range(&self, start_idx: usize, end_idx: usize) -> Vec<&SeriesMarker> {
        self.markers
            .iter()
            .filter(|m| m.bar_index >= start_idx && m.bar_index <= end_idx)
            .collect()
    }
}

/// Global marker manager that maps series IDs to their markers.
pub struct MarkerManager {
    /// Series ID -> markers for that series.
    series_markers: HashMap<u32, SeriesMarkers>,
}

impl Default for MarkerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkerManager {
    pub fn new() -> Self {
        Self {
            series_markers: HashMap::new(),
        }
    }

    /// Get or create markers collection for a series.
    pub fn for_series(&mut self, series_id: u32) -> &mut SeriesMarkers {
        self.series_markers
            .entry(series_id)
            .or_insert_with(SeriesMarkers::new)
    }

    /// Get markers for a series (immutable).
    pub fn get(&self, series_id: u32) -> Option<&SeriesMarkers> {
        self.series_markers.get(&series_id)
    }

    /// Clear markers for a specific series.
    pub fn clear_series(&mut self, series_id: u32) {
        if let Some(markers) = self.series_markers.get_mut(&series_id) {
            markers.clear();
        }
    }

    /// Clear all markers for all series.
    pub fn clear_all(&mut self) {
        self.series_markers.clear();
    }

    /// Remove a series and its markers.
    pub fn remove_series(&mut self, series_id: u32) {
        self.series_markers.remove(&series_id);
    }

    /// Iterate over all series and their markers.
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &SeriesMarkers)> {
        self.series_markers.iter()
    }
}
