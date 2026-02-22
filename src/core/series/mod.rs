//! Series module — multi-series abstraction for the chart engine.
//!
//! Each `Series` owns its data and rendering options. The engine holds
//! a `SeriesCollection` that renderers iterate over during `draw_lines()`.

pub mod line_data;
pub mod line_options;

pub use line_data::{LinePoint, LineDataArray};
pub use line_options::{LineSeriesOptions, LineStyle};

// ── Series ID ───────────────────────────────────────────────────────────────

/// Unique identifier for a series. Monotonically increasing, never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeriesId(pub u32);

// ── Series type ─────────────────────────────────────────────────────────────

/// The kind of series — determines data format and rendering path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesType {
    /// OHLCV candlestick (rendered by the existing candle pipeline).
    /// Data lives in `ChartEngine.bars` — not managed by SeriesCollection.
    Candlestick,
    /// Single-value line series (SMA, EMA, or any indicator).
    Line,
}

// ── Series ──────────────────────────────────────────────────────────────────

/// A single series instance. Owns its data and display options.
pub struct Series {
    id: SeriesId,
    series_type: SeriesType,
    /// Line data (only used when series_type == Line).
    pub line_data: LineDataArray,
    /// Line rendering options.
    pub line_options: LineSeriesOptions,
}

impl Series {
    pub fn new_line(id: SeriesId, options: LineSeriesOptions) -> Self {
        Self {
            id,
            series_type: SeriesType::Line,
            line_data: LineDataArray::new(),
            line_options: options,
        }
    }

    #[inline]
    pub fn id(&self) -> SeriesId {
        self.id
    }

    #[inline]
    pub fn series_type(&self) -> SeriesType {
        self.series_type
    }
}

// ── Series collection ───────────────────────────────────────────────────────

/// Manages all overlay series on a pane. The builtin candlestick series
/// is NOT included here — it lives in `ChartEngine.bars`.
pub struct SeriesCollection {
    series: Vec<Series>,
    next_id: u32,
}

impl SeriesCollection {
    pub fn new() -> Self {
        Self {
            series: Vec::new(),
            next_id: 1, // 0 reserved for the implicit candlestick
        }
    }

    /// Add a new line series. Returns the assigned SeriesId.
    pub fn add_line(&mut self, options: LineSeriesOptions) -> SeriesId {
        let id = SeriesId(self.next_id);
        self.next_id += 1;
        self.series.push(Series::new_line(id, options));
        id
    }

    /// Remove a series by ID. Returns true if found and removed.
    pub fn remove(&mut self, id: SeriesId) -> bool {
        if let Some(pos) = self.series.iter().position(|s| s.id == id) {
            self.series.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get a mutable reference to a series by ID.
    pub fn get_mut(&mut self, id: SeriesId) -> Option<&mut Series> {
        self.series.iter_mut().find(|s| s.id == id)
    }

    /// Get an immutable reference to a series by ID.
    pub fn get(&self, id: SeriesId) -> Option<&Series> {
        self.series.iter().find(|s| s.id == id)
    }

    /// Iterate over all series.
    pub fn iter(&self) -> impl Iterator<Item = &Series> {
        self.series.iter()
    }

    /// Number of overlay series.
    pub fn len(&self) -> usize {
        self.series.len()
    }

    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }
}
