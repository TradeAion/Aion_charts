//! PriceLine — horizontal price level lines with labels.
//!
//! Matches LWC's createPriceLine() API:
//! - Horizontal line at a specified price, spanning the full pane width
//! - Optional label on the price axis
//! - Hit-testable (7px threshold) and optionally draggable
//! - Supports all LineStyle dash patterns

use crate::core::series::LineStyle;

/// Unique identifier for a price line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PriceLineId(pub u32);

/// Configuration options for a price line.
#[derive(Debug, Clone)]
pub struct PriceLineOptions {
    /// The price level where the line is drawn.
    pub price: f64,
    /// Line color [R, G, B, A] in 0.0–1.0 range.
    pub color: [f32; 4],
    /// Line width in CSS pixels.
    pub line_width: f64,
    /// Line dash style.
    pub line_style: LineStyle,
    /// Whether the line is visible.
    pub visible: bool,
    /// Whether the line can be dragged vertically.
    pub draggable: bool,
    /// Optional label text (shown on price axis). Empty = use formatted price.
    pub label_text: String,
    /// Whether to show the label on the price axis.
    pub show_label: bool,
    /// Label background color (defaults to line color if not set).
    pub label_bg_color: Option<[f32; 4]>,
    /// Label text color.
    pub label_text_color: [f32; 4],
}

impl Default for PriceLineOptions {
    fn default() -> Self {
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        Self {
            price: 0.0,
            color: theme.series_defaults.price_line_color,
            line_width: 1.0,
            line_style: LineStyle::Dashed,
            visible: true,
            draggable: false,
            label_text: String::new(),
            show_label: true,
            label_bg_color: None,
            label_text_color: theme.series_defaults.price_line_text_color,
        }
    }
}

/// A single price line instance.
#[derive(Debug, Clone)]
pub struct PriceLine {
    id: PriceLineId,
    /// Current options (includes mutable price for dragging).
    pub options: PriceLineOptions,
    /// Whether this line is currently being dragged.
    pub dragging: bool,
    /// Whether this line is hovered.
    pub hovered: bool,
}

impl PriceLine {
    pub fn new(id: PriceLineId, options: PriceLineOptions) -> Self {
        Self {
            id,
            options,
            dragging: false,
            hovered: false,
        }
    }

    #[inline]
    pub fn id(&self) -> PriceLineId {
        self.id
    }

    #[inline]
    pub fn price(&self) -> f64 {
        self.options.price
    }

    #[inline]
    pub fn set_price(&mut self, price: f64) {
        self.options.price = price;
    }

    #[inline]
    pub fn is_visible(&self) -> bool {
        self.options.visible
    }

    #[inline]
    pub fn is_draggable(&self) -> bool {
        self.options.draggable
    }
}

/// Hit-test result for price lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceLineHit {
    /// No hit.
    None,
    /// Hit the line body (for selection/hover).
    Line(PriceLineId),
}

/// Manages all price lines on a chart.
pub struct PriceLineManager {
    lines: Vec<PriceLine>,
    next_id: u32,
    /// Hit threshold in CSS pixels.
    hit_threshold: f64,
}

impl PriceLineManager {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            next_id: 1,
            hit_threshold: 7.0,
        }
    }

    /// Create a new price line. Returns the assigned ID.
    pub fn create(&mut self, options: PriceLineOptions) -> PriceLineId {
        let id = PriceLineId(self.next_id);
        self.next_id += 1;
        self.lines.push(PriceLine::new(id, options));
        id
    }

    /// Remove a price line by ID. Returns true if found and removed.
    pub fn remove(&mut self, id: PriceLineId) -> bool {
        if let Some(pos) = self.lines.iter().position(|l| l.id == id) {
            self.lines.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get a mutable reference to a price line by ID.
    pub fn get_mut(&mut self, id: PriceLineId) -> Option<&mut PriceLine> {
        self.lines.iter_mut().find(|l| l.id == id)
    }

    /// Get an immutable reference to a price line by ID.
    pub fn get(&self, id: PriceLineId) -> Option<&PriceLine> {
        self.lines.iter().find(|l| l.id == id)
    }

    /// Iterate over all price lines.
    pub fn iter(&self) -> impl Iterator<Item = &PriceLine> {
        self.lines.iter()
    }

    /// Number of price lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Hit-test a point against all price lines.
    /// `y_css` is the Y coordinate in CSS pixels.
    /// `viewport` is used to convert prices to Y coordinates.
    pub fn hit_test(
        &self,
        y_css: f64,
        viewport: &crate::core::viewport::Viewport,
        pane_css_h: f64,
    ) -> PriceLineHit {
        for line in &self.lines {
            if !line.is_visible() {
                continue;
            }
            let line_y = viewport.price_to_css_y(line.price(), pane_css_h);
            let dist = (y_css - line_y).abs();
            if dist <= self.hit_threshold {
                return PriceLineHit::Line(line.id);
            }
        }
        PriceLineHit::None
    }

    /// Clear hover state for all lines.
    pub fn clear_hover(&mut self) {
        for line in &mut self.lines {
            line.hovered = false;
        }
    }

    /// Set hover state for a specific line.
    pub fn set_hover(&mut self, id: PriceLineId, hovered: bool) {
        if let Some(line) = self.get_mut(id) {
            line.hovered = hovered;
        }
    }

    /// Start dragging a price line.
    pub fn start_drag(&mut self, id: PriceLineId) -> bool {
        if let Some(line) = self.get_mut(id) {
            if line.is_draggable() {
                line.dragging = true;
                return true;
            }
        }
        false
    }

    /// Update price during drag.
    pub fn drag_to(
        &mut self,
        id: PriceLineId,
        y_css: f64,
        viewport: &crate::core::viewport::Viewport,
        pane_css_h: f64,
    ) {
        if let Some(line) = self.get_mut(id) {
            if line.dragging {
                // Convert Y coordinate to price
                let candle_h = pane_css_h * viewport.candle_height_frac();
                let frac = 1.0 - (y_css / candle_h).clamp(0.0, 1.0);
                let internal =
                    viewport.price_min + frac * (viewport.price_max - viewport.price_min);
                let price = viewport.internal_to_price(internal);
                line.set_price(price);
            }
        }
    }

    /// End dragging.
    pub fn end_drag(&mut self, id: PriceLineId) {
        if let Some(line) = self.get_mut(id) {
            line.dragging = false;
        }
    }

    /// End all drags.
    pub fn end_all_drags(&mut self) {
        for line in &mut self.lines {
            line.dragging = false;
        }
    }
}
