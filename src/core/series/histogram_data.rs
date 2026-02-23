//! Histogram data storage — columnar arrays for timestamp, value, and optional per-bar color.
//!
//! Like LWC, each histogram bar can have an individual color override.
//! If no per-bar color is set, the series default color is used.

/// A single histogram data point.
#[derive(Debug, Clone, Copy)]
pub struct HistogramPoint {
    pub timestamp: u64,
    pub value: f32,
    /// Optional per-bar color override [R, G, B, A]. If all zeros, use series default.
    pub color: [f32; 4],
}

/// Columnar storage for histogram data.
#[derive(Debug, Clone, Default)]
pub struct HistogramDataArray {
    pub timestamps: Vec<u64>,
    pub values: Vec<f32>,
    /// Per-bar color overrides. Same length as `values`.
    /// `[0,0,0,0]` means "use series default color".
    pub colors: Vec<[f32; 4]>,
}

impl HistogramDataArray {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Set data from a vector of HistogramPoint.
    pub fn set_data(&mut self, data: Vec<HistogramPoint>) {
        self.timestamps.clear();
        self.values.clear();
        self.colors.clear();
        self.timestamps.reserve(data.len());
        self.values.reserve(data.len());
        self.colors.reserve(data.len());
        for p in data {
            self.timestamps.push(p.timestamp);
            self.values.push(p.value);
            self.colors.push(p.color);
        }
    }

    /// Set data from parallel arrays (no per-bar color — all default).
    pub fn set_from_arrays(&mut self, timestamps: &[u64], values: &[f32]) {
        let count = timestamps.len().min(values.len());
        self.timestamps = timestamps[..count].to_vec();
        self.values = values[..count].to_vec();
        self.colors = vec![[0.0; 4]; count]; // all zeros = use default
    }

    /// Set data from parallel arrays with per-bar colors.
    pub fn set_from_arrays_with_colors(
        &mut self,
        timestamps: &[u64],
        values: &[f32],
        colors: &[[f32; 4]],
    ) {
        let count = timestamps.len().min(values.len()).min(colors.len());
        self.timestamps = timestamps[..count].to_vec();
        self.values = values[..count].to_vec();
        self.colors = colors[..count].to_vec();
    }

    /// Returns true if the bar at index `i` has a per-bar color override.
    #[inline]
    pub fn has_color_override(&self, i: usize) -> bool {
        if i >= self.colors.len() {
            return false;
        }
        let c = self.colors[i];
        // If alpha > 0, it's a real override
        c[3] > 0.0
    }

    /// Get the effective color for bar `i`, falling back to `default_color`.
    #[inline]
    pub fn effective_color(&self, i: usize, default_color: [f32; 4]) -> [f32; 4] {
        if self.has_color_override(i) {
            self.colors[i]
        } else {
            default_color
        }
    }
}
