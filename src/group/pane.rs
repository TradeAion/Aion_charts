//! Group pane state used by `ChartGroup`.

/// Stable pane handle inside a `ChartGroup`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChartPaneId(pub u64);

/// Crosshair magnet mode used for synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrosshairMagnetMode {
    Normal,
    Ohlc,
}

impl Default for CrosshairMagnetMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl CrosshairMagnetMode {
    pub fn from_key(key: &str) -> Self {
        match key {
            "ohlc" | "magnet_ohlc" | "magnet" => Self::Ohlc,
            _ => Self::Normal,
        }
    }
}

/// Synchronizable crosshair snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrosshairSnapshot {
    pub active: bool,
    pub x: f64,
    pub y: f64,
    pub bar_index: Option<f64>,
    pub price: Option<f64>,
    pub magnet: CrosshairMagnetMode,
}

impl Default for CrosshairSnapshot {
    fn default() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            bar_index: None,
            price: None,
            magnet: CrosshairMagnetMode::Normal,
        }
    }
}

/// Visible logical time range (`start_bar..end_bar`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start_bar: f64,
    pub end_bar: f64,
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            start_bar: 0.0,
            end_bar: 0.0,
        }
    }
}

/// Loaded data timestamp range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataRange {
    pub from_timestamp: Option<u64>,
    pub to_timestamp: Option<u64>,
}

impl Default for DataRange {
    fn default() -> Self {
        Self {
            from_timestamp: None,
            to_timestamp: None,
        }
    }
}

/// A pane registered in `ChartGroup`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartPane {
    pub id: ChartPaneId,
    pub symbol: String,
    pub interval: String,
    pub crosshair: CrosshairSnapshot,
    pub time_range: TimeRange,
    pub data_range: DataRange,
}

impl ChartPane {
    pub fn new(id: ChartPaneId, symbol: impl Into<String>, interval: impl Into<String>) -> Self {
        Self {
            id,
            symbol: symbol.into(),
            interval: interval.into(),
            crosshair: CrosshairSnapshot::default(),
            time_range: TimeRange::default(),
            data_range: DataRange::default(),
        }
    }

    pub fn set_symbol(&mut self, symbol: impl Into<String>) -> bool {
        let symbol = symbol.into();
        if self.symbol == symbol {
            return false;
        }
        self.symbol = symbol;
        true
    }

    pub fn set_interval(&mut self, interval: impl Into<String>) -> bool {
        let interval = interval.into();
        if self.interval == interval {
            return false;
        }
        self.interval = interval;
        true
    }

    pub fn set_crosshair(&mut self, crosshair: CrosshairSnapshot) -> bool {
        if self.crosshair == crosshair {
            return false;
        }
        self.crosshair = crosshair;
        true
    }

    pub fn set_time_range(&mut self, time_range: TimeRange) -> bool {
        if self.time_range == time_range {
            return false;
        }
        self.time_range = time_range;
        true
    }

    pub fn set_data_range(&mut self, data_range: DataRange) -> bool {
        if self.data_range == data_range {
            return false;
        }
        self.data_range = data_range;
        true
    }
}
