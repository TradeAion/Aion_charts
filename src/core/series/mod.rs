//! Series module — multi-series abstraction for the chart engine.
//!
//! Each `Series` owns its data and rendering options. The engine holds
//! a `SeriesCollection` that renderers iterate over during `draw_lines()`.

pub mod area_options;
pub mod bar_data;
pub mod bar_options;
pub mod baseline_options;
pub mod histogram_data;
pub mod histogram_options;
pub mod line_data;
pub mod line_options;
pub mod validation;

pub use area_options::AreaSeriesOptions;
pub use bar_data::{OhlcDataArray, OhlcPoint};
pub use bar_options::BarSeriesOptions;
pub use baseline_options::BaselineSeriesOptions;
pub use histogram_data::{HistogramDataArray, HistogramPoint};
pub use histogram_options::HistogramSeriesOptions;
pub use line_data::{LineDataArray, LinePoint};
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
    /// Area series — line with gradient fill below (or above).
    Area,
    /// Histogram series — vertical bars from a base value.
    Histogram,
    /// Bar (OHLC) series — traditional open-high-low-close bars with ticks.
    Bar,
    /// Baseline series — line with two-tone fill above/below a base value.
    Baseline,
}

// ── Series ──────────────────────────────────────────────────────────────────

/// A single series instance. Owns its data and display options.
pub struct Series {
    id: SeriesId,
    series_type: SeriesType,
    /// Line data (used when series_type == Line or Area).
    pub line_data: LineDataArray,
    /// Line rendering options (used when series_type == Line).
    pub line_options: LineSeriesOptions,
    /// Area rendering options (used when series_type == Area).
    pub area_options: AreaSeriesOptions,
    /// Histogram rendering options (used when series_type == Histogram).
    pub histogram_options: HistogramSeriesOptions,
    /// Histogram data (used when series_type == Histogram).
    pub histogram_data: HistogramDataArray,
    /// Bar (OHLC) rendering options (used when series_type == Bar).
    pub bar_options: BarSeriesOptions,
    /// Bar (OHLC) data (used when series_type == Bar).
    pub bar_data: OhlcDataArray,
    /// Baseline rendering options (used when series_type == Baseline).
    pub baseline_options: BaselineSeriesOptions,
}

impl Series {
    pub fn new_line(id: SeriesId, options: LineSeriesOptions) -> Self {
        Self {
            id,
            series_type: SeriesType::Line,
            line_data: LineDataArray::new(),
            line_options: options,
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            histogram_data: HistogramDataArray::new(),
            bar_options: BarSeriesOptions::default(),
            bar_data: OhlcDataArray::new(),
            baseline_options: BaselineSeriesOptions::default(),
        }
    }

    pub fn new_area(id: SeriesId, options: AreaSeriesOptions) -> Self {
        Self {
            id,
            series_type: SeriesType::Area,
            line_data: LineDataArray::new(),
            line_options: LineSeriesOptions::default(),
            area_options: options,
            histogram_options: HistogramSeriesOptions::default(),
            histogram_data: HistogramDataArray::new(),
            bar_options: BarSeriesOptions::default(),
            bar_data: OhlcDataArray::new(),
            baseline_options: BaselineSeriesOptions::default(),
        }
    }

    pub fn new_histogram(id: SeriesId, options: HistogramSeriesOptions) -> Self {
        Self {
            id,
            series_type: SeriesType::Histogram,
            line_data: LineDataArray::new(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: options,
            histogram_data: HistogramDataArray::new(),
            bar_options: BarSeriesOptions::default(),
            bar_data: OhlcDataArray::new(),
            baseline_options: BaselineSeriesOptions::default(),
        }
    }

    pub fn new_bar(id: SeriesId, options: BarSeriesOptions) -> Self {
        Self {
            id,
            series_type: SeriesType::Bar,
            line_data: LineDataArray::new(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            histogram_data: HistogramDataArray::new(),
            bar_options: options,
            bar_data: OhlcDataArray::new(),
            baseline_options: BaselineSeriesOptions::default(),
        }
    }

    pub fn new_baseline(id: SeriesId, options: BaselineSeriesOptions) -> Self {
        Self {
            id,
            series_type: SeriesType::Baseline,
            line_data: LineDataArray::new(),
            line_options: LineSeriesOptions::default(),
            area_options: AreaSeriesOptions::default(),
            histogram_options: HistogramSeriesOptions::default(),
            histogram_data: HistogramDataArray::new(),
            bar_options: BarSeriesOptions::default(),
            bar_data: OhlcDataArray::new(),
            baseline_options: options,
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

    /// Get the last data value (close for OHLC, value for line/histogram).
    /// Returns None if the series has no data.
    pub fn last_value(&self) -> Option<f64> {
        match self.series_type {
            SeriesType::Line | SeriesType::Area | SeriesType::Baseline => {
                if self.line_data.is_empty() {
                    None
                } else {
                    Some(self.line_data.values[self.line_data.len() - 1])
                }
            }
            SeriesType::Histogram => {
                if self.histogram_data.is_empty() {
                    None
                } else {
                    Some(self.histogram_data.values[self.histogram_data.len() - 1])
                }
            }
            SeriesType::Bar => {
                if self.bar_data.is_empty() {
                    None
                } else {
                    Some(self.bar_data.close[self.bar_data.len() - 1])
                }
            }
            SeriesType::Candlestick => None, // Managed by engine.bars
        }
    }

    /// Get the primary line color for this series.
    pub fn series_color(&self) -> [f32; 4] {
        match self.series_type {
            SeriesType::Line => self.line_options.color,
            SeriesType::Area => self.area_options.line_color,
            SeriesType::Histogram => self.histogram_options.color,
            SeriesType::Bar => self.bar_options.up_color,
            SeriesType::Baseline => self.baseline_options.top_line_color,
            SeriesType::Candlestick => [1.0; 4],
        }
    }

    /// Whether this series is visible.
    pub fn is_visible(&self) -> bool {
        match self.series_type {
            SeriesType::Line => self.line_options.visible,
            SeriesType::Area => self.area_options.visible,
            SeriesType::Histogram => self.histogram_options.visible,
            SeriesType::Bar => self.bar_options.visible,
            SeriesType::Baseline => self.baseline_options.visible,
            SeriesType::Candlestick => true,
        }
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

    /// Add a new area series. Returns the assigned SeriesId.
    pub fn add_area(&mut self, options: AreaSeriesOptions) -> SeriesId {
        let id = SeriesId(self.next_id);
        self.next_id += 1;
        self.series.push(Series::new_area(id, options));
        id
    }

    /// Add a new histogram series. Returns the assigned SeriesId.
    pub fn add_histogram(&mut self, options: HistogramSeriesOptions) -> SeriesId {
        let id = SeriesId(self.next_id);
        self.next_id += 1;
        self.series.push(Series::new_histogram(id, options));
        id
    }

    /// Add a new bar (OHLC) series. Returns the assigned SeriesId.
    pub fn add_bar(&mut self, options: BarSeriesOptions) -> SeriesId {
        let id = SeriesId(self.next_id);
        self.next_id += 1;
        self.series.push(Series::new_bar(id, options));
        id
    }

    /// Add a new baseline series. Returns the assigned SeriesId.
    pub fn add_baseline(&mut self, options: BaselineSeriesOptions) -> SeriesId {
        let id = SeriesId(self.next_id);
        self.next_id += 1;
        self.series.push(Series::new_baseline(id, options));
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
