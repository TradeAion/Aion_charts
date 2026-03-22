//! ExecutionMark — first-class trade execution markers for trading workflows.
//!
//! Unlike generic SeriesMarker, ExecutionMark is timestamp-based (not bar-index-based)
//! and designed specifically for trade execution visualization:
//! - Entry / Exit / Scale-in / Scale-out semantics
//! - Buy / Sell side distinction
//! - Quantity, realized P&L, order type metadata
//! - Grouped fills via group_id
//!
//! The engine resolves timestamps to bar indices internally, so host apps can
//! work directly with execution data without manual bar-index conversion.

use std::collections::HashMap;

use crate::core::data::BarArray;

/// Execution side: buy or sell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionSide {
    /// Buy execution (long entry, short exit/cover).
    Buy,
    /// Sell execution (short entry, long exit).
    Sell,
}

impl ExecutionSide {
    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "buy" | "long" | "b" => ExecutionSide::Buy,
            "sell" | "short" | "s" => ExecutionSide::Sell,
            _ => ExecutionSide::Buy, // default
        }
    }

    /// Convert to string key.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionSide::Buy => "buy",
            ExecutionSide::Sell => "sell",
        }
    }
}

/// Execution role in a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionRole {
    /// Initial position entry.
    Entry,
    /// Adding to existing position.
    ScaleIn,
    /// Partial position reduction.
    ScaleOut,
    /// Full position exit.
    Exit,
}

impl ExecutionRole {
    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "entry" | "open" | "start" => ExecutionRole::Entry,
            "scale_in" | "scalein" | "add" | "pyramid" => ExecutionRole::ScaleIn,
            "scale_out" | "scaleout" | "partial" | "reduce" => ExecutionRole::ScaleOut,
            "exit" | "close" | "end" => ExecutionRole::Exit,
            _ => ExecutionRole::Entry, // default
        }
    }

    /// Convert to string key.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionRole::Entry => "entry",
            ExecutionRole::ScaleIn => "scale_in",
            ExecutionRole::ScaleOut => "scale_out",
            ExecutionRole::Exit => "exit",
        }
    }
}

/// A single trade execution mark.
#[derive(Debug, Clone)]
pub struct ExecutionMark {
    /// Unique identifier for this execution.
    pub id: String,
    /// Unix timestamp in milliseconds when the execution occurred.
    pub timestamp_ms: u64,
    /// Execution price.
    pub price: f64,
    /// Execution quantity (positive).
    pub quantity: f64,
    /// Buy or sell.
    pub side: ExecutionSide,
    /// Role in the trade (entry, scale-in, scale-out, exit).
    pub role: ExecutionRole,
    /// Order type (optional, e.g., "market", "limit", "stop").
    pub order_type: Option<String>,
    /// Realized P&L from this execution (optional, for exits/partials).
    pub realized_pnl: Option<f64>,
    /// Custom label text (optional, overrides default).
    pub label: Option<String>,
    /// Custom color override [R, G, B, A] in 0.0–1.0 range (optional).
    pub color: Option<[f32; 4]>,
    /// Group ID for related fills (optional, e.g., same trade).
    pub group_id: Option<String>,

    // Internal cached bar index (populated by resolve_bar_indices)
    pub(crate) resolved_bar_index: Option<usize>,
}

impl ExecutionMark {
    /// Create a new execution mark with required fields.
    pub fn new(
        id: impl Into<String>,
        timestamp_ms: u64,
        price: f64,
        quantity: f64,
        side: ExecutionSide,
        role: ExecutionRole,
    ) -> Self {
        Self {
            id: id.into(),
            timestamp_ms,
            price,
            quantity: quantity.abs(),
            side,
            role,
            order_type: None,
            realized_pnl: None,
            label: None,
            color: None,
            group_id: None,
            resolved_bar_index: None,
        }
    }

    /// Builder: set order type.
    pub fn with_order_type(mut self, order_type: impl Into<String>) -> Self {
        self.order_type = Some(order_type.into());
        self
    }

    /// Builder: set realized P&L.
    pub fn with_realized_pnl(mut self, pnl: f64) -> Self {
        self.realized_pnl = Some(pnl);
        self
    }

    /// Builder: set custom label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Builder: set color override.
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    /// Builder: set group ID.
    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }

    /// Get the display label for this execution.
    pub fn display_label(&self) -> String {
        if let Some(ref label) = self.label {
            return label.clone();
        }
        // Default label format: SIDE ROLE @ PRICE
        format!(
            "{} {} @ {:.2}",
            self.side.as_str().to_uppercase(),
            self.role.as_str().to_uppercase(),
            self.price
        )
    }
}

/// Manager for execution marks on a chart.
///
/// Stores execution marks by ID and provides bulk operations.
/// Resolves timestamps to bar indices for rendering.
pub struct ExecutionMarkManager {
    /// All execution marks, keyed by ID.
    marks: HashMap<String, ExecutionMark>,
    /// Marks sorted by timestamp for efficient range queries.
    sorted_ids: Vec<String>,
    /// Whether sorted_ids needs rebuilding.
    dirty_sort: bool,
}

impl Default for ExecutionMarkManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionMarkManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            marks: HashMap::new(),
            sorted_ids: Vec::new(),
            dirty_sort: false,
        }
    }

    /// Add a single execution mark.
    pub fn add(&mut self, mark: ExecutionMark) {
        self.marks.insert(mark.id.clone(), mark);
        self.dirty_sort = true;
    }

    /// Remove an execution mark by ID. Returns true if found.
    pub fn remove(&mut self, id: &str) -> bool {
        if self.marks.remove(id).is_some() {
            self.dirty_sort = true;
            true
        } else {
            false
        }
    }

    /// Clear all execution marks.
    pub fn clear(&mut self) {
        self.marks.clear();
        self.sorted_ids.clear();
        self.dirty_sort = false;
    }

    /// Replace all execution marks with a new set (bulk set).
    pub fn set(&mut self, marks: Vec<ExecutionMark>) {
        self.marks.clear();
        for mark in marks {
            self.marks.insert(mark.id.clone(), mark);
        }
        self.dirty_sort = true;
    }

    /// Number of execution marks.
    pub fn len(&self) -> usize {
        self.marks.len()
    }

    /// Whether there are no execution marks.
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// Get an execution mark by ID.
    pub fn get(&self, id: &str) -> Option<&ExecutionMark> {
        self.marks.get(id)
    }

    /// Get a mutable execution mark by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ExecutionMark> {
        self.marks.get_mut(id)
    }

    /// Iterate over all marks.
    pub fn iter(&self) -> impl Iterator<Item = &ExecutionMark> {
        self.marks.values()
    }

    /// Iterate over all marks mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ExecutionMark> {
        self.marks.values_mut()
    }

    /// Ensure sorted_ids is up to date.
    fn ensure_sorted(&mut self) {
        if self.dirty_sort {
            self.sorted_ids = self.marks.keys().cloned().collect();
            self.sorted_ids
                .sort_by_key(|id| self.marks.get(id).map(|m| m.timestamp_ms).unwrap_or(0));
            self.dirty_sort = false;
        }
    }

    /// Get marks in a timestamp range [start_ms, end_ms] (inclusive).
    pub fn in_time_range(&mut self, start_ms: u64, end_ms: u64) -> Vec<&ExecutionMark> {
        self.ensure_sorted();
        self.sorted_ids
            .iter()
            .filter_map(|id| self.marks.get(id))
            .filter(|m| m.timestamp_ms >= start_ms && m.timestamp_ms <= end_ms)
            .collect()
    }

    /// Get marks visible in a bar index range [start_idx, end_idx].
    /// Requires resolve_bar_indices to have been called first.
    pub fn in_bar_range(&self, start_idx: usize, end_idx: usize) -> Vec<&ExecutionMark> {
        self.marks
            .values()
            .filter(|m| {
                m.resolved_bar_index
                    .map(|idx| idx >= start_idx && idx <= end_idx)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Resolve timestamps to bar indices using the provided bar data.
    ///
    /// This should be called after setting bars or execution marks, before rendering.
    /// Uses binary search for efficiency.
    pub fn resolve_bar_indices(&mut self, bars: &BarArray) {
        if bars.is_empty() {
            for mark in self.marks.values_mut() {
                mark.resolved_bar_index = None;
            }
            return;
        }

        for mark in self.marks.values_mut() {
            mark.resolved_bar_index = timestamp_to_bar_index(mark.timestamp_ms, bars);
        }
    }

    /// Get all marks with their resolved bar indices, for rendering.
    pub fn visible_marks_for_render(
        &self,
        start_bar: usize,
        end_bar: usize,
    ) -> Vec<&ExecutionMark> {
        self.in_bar_range(start_bar, end_bar)
    }

    /// Get marks by group ID.
    pub fn by_group(&self, group_id: &str) -> Vec<&ExecutionMark> {
        self.marks
            .values()
            .filter(|m| m.group_id.as_deref() == Some(group_id))
            .collect()
    }
}

/// Resolve a timestamp to a bar index using binary search.
///
/// Returns the index of the bar that contains or immediately precedes the timestamp.
/// Returns None if the timestamp is before all bars.
pub fn timestamp_to_bar_index(timestamp_ms: u64, bars: &BarArray) -> Option<usize> {
    if bars.is_empty() {
        return None;
    }

    let len = bars.len();

    // Check bounds
    let first_ts = bars.timestamp(0);
    let last_ts = bars.timestamp(len - 1);

    if timestamp_ms < first_ts {
        return None; // Before all data
    }
    if timestamp_ms >= last_ts {
        return Some(len - 1); // At or after last bar
    }

    // Binary search for the bar containing or preceding the timestamp
    let mut lo = 0;
    let mut hi = len - 1;

    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        if bars.timestamp(mid) <= timestamp_ms {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    Some(lo)
}

/// Convert a bar index back to its timestamp.
pub fn bar_index_to_timestamp(bar_index: usize, bars: &BarArray) -> Option<u64> {
    if bar_index < bars.len() {
        Some(bars.timestamp(bar_index))
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::data::Bar;

    fn make_bar(ts: u64) -> Bar {
        Bar {
            timestamp: ts,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 1000.0,
            _pad: 0.0,
        }
    }

    fn make_bars(timestamps: &[u64]) -> BarArray {
        let mut bars = BarArray::new();
        bars.set(timestamps.iter().map(|&ts| make_bar(ts)).collect());
        bars
    }

    #[test]
    fn test_execution_side_from_str() {
        assert_eq!(ExecutionSide::from_str("buy"), ExecutionSide::Buy);
        assert_eq!(ExecutionSide::from_str("BUY"), ExecutionSide::Buy);
        assert_eq!(ExecutionSide::from_str("long"), ExecutionSide::Buy);
        assert_eq!(ExecutionSide::from_str("sell"), ExecutionSide::Sell);
        assert_eq!(ExecutionSide::from_str("SELL"), ExecutionSide::Sell);
        assert_eq!(ExecutionSide::from_str("short"), ExecutionSide::Sell);
    }

    #[test]
    fn test_execution_role_from_str() {
        assert_eq!(ExecutionRole::from_str("entry"), ExecutionRole::Entry);
        assert_eq!(ExecutionRole::from_str("ENTRY"), ExecutionRole::Entry);
        assert_eq!(ExecutionRole::from_str("scale_in"), ExecutionRole::ScaleIn);
        assert_eq!(ExecutionRole::from_str("scalein"), ExecutionRole::ScaleIn);
        assert_eq!(
            ExecutionRole::from_str("scale_out"),
            ExecutionRole::ScaleOut
        );
        assert_eq!(ExecutionRole::from_str("exit"), ExecutionRole::Exit);
    }

    #[test]
    fn test_execution_mark_builder() {
        let mark = ExecutionMark::new(
            "exec-1",
            1000,
            100.5,
            10.0,
            ExecutionSide::Buy,
            ExecutionRole::Entry,
        )
        .with_order_type("market")
        .with_label("Entry Long")
        .with_realized_pnl(0.0)
        .with_group_id("trade-1");

        assert_eq!(mark.id, "exec-1");
        assert_eq!(mark.timestamp_ms, 1000);
        assert_eq!(mark.price, 100.5);
        assert_eq!(mark.quantity, 10.0);
        assert_eq!(mark.side, ExecutionSide::Buy);
        assert_eq!(mark.role, ExecutionRole::Entry);
        assert_eq!(mark.order_type, Some("market".to_string()));
        assert_eq!(mark.label, Some("Entry Long".to_string()));
        assert_eq!(mark.realized_pnl, Some(0.0));
        assert_eq!(mark.group_id, Some("trade-1".to_string()));
    }

    #[test]
    fn test_manager_add_remove() {
        let mut mgr = ExecutionMarkManager::new();
        assert!(mgr.is_empty());

        mgr.add(ExecutionMark::new(
            "1",
            1000,
            100.0,
            1.0,
            ExecutionSide::Buy,
            ExecutionRole::Entry,
        ));
        mgr.add(ExecutionMark::new(
            "2",
            2000,
            101.0,
            1.0,
            ExecutionSide::Sell,
            ExecutionRole::Exit,
        ));

        assert_eq!(mgr.len(), 2);
        assert!(!mgr.is_empty());
        assert!(mgr.get("1").is_some());
        assert!(mgr.get("2").is_some());
        assert!(mgr.get("3").is_none());

        assert!(mgr.remove("1"));
        assert_eq!(mgr.len(), 1);
        assert!(mgr.get("1").is_none());
        assert!(!mgr.remove("1")); // Already removed
    }

    #[test]
    fn test_manager_set_clear() {
        let mut mgr = ExecutionMarkManager::new();

        mgr.set(vec![
            ExecutionMark::new(
                "a",
                1000,
                100.0,
                1.0,
                ExecutionSide::Buy,
                ExecutionRole::Entry,
            ),
            ExecutionMark::new(
                "b",
                2000,
                101.0,
                1.0,
                ExecutionSide::Sell,
                ExecutionRole::Exit,
            ),
        ]);
        assert_eq!(mgr.len(), 2);

        mgr.clear();
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_timestamp_to_bar_index() {
        let bars = make_bars(&[1000, 2000, 3000, 4000, 5000]);

        // Exact matches
        assert_eq!(timestamp_to_bar_index(1000, &bars), Some(0));
        assert_eq!(timestamp_to_bar_index(3000, &bars), Some(2));
        assert_eq!(timestamp_to_bar_index(5000, &bars), Some(4));

        // Between bars (should return preceding bar)
        assert_eq!(timestamp_to_bar_index(1500, &bars), Some(0));
        assert_eq!(timestamp_to_bar_index(2500, &bars), Some(1));
        assert_eq!(timestamp_to_bar_index(4999, &bars), Some(3));

        // Before all bars
        assert_eq!(timestamp_to_bar_index(500, &bars), None);

        // After all bars
        assert_eq!(timestamp_to_bar_index(6000, &bars), Some(4));
    }

    #[test]
    fn test_bar_index_to_timestamp() {
        let bars = make_bars(&[1000, 2000, 3000, 4000, 5000]);

        assert_eq!(bar_index_to_timestamp(0, &bars), Some(1000));
        assert_eq!(bar_index_to_timestamp(2, &bars), Some(3000));
        assert_eq!(bar_index_to_timestamp(4, &bars), Some(5000));
        assert_eq!(bar_index_to_timestamp(10, &bars), None); // Out of bounds
    }

    #[test]
    fn test_resolve_bar_indices() {
        let bars = make_bars(&[1000, 2000, 3000, 4000, 5000]);
        let mut mgr = ExecutionMarkManager::new();

        mgr.set(vec![
            ExecutionMark::new(
                "a",
                1500,
                100.0,
                1.0,
                ExecutionSide::Buy,
                ExecutionRole::Entry,
            ),
            ExecutionMark::new(
                "b",
                3000,
                101.0,
                1.0,
                ExecutionSide::Sell,
                ExecutionRole::Exit,
            ),
            ExecutionMark::new(
                "c",
                500,
                99.0,
                1.0,
                ExecutionSide::Buy,
                ExecutionRole::Entry,
            ), // Before data
        ]);

        mgr.resolve_bar_indices(&bars);

        assert_eq!(mgr.get("a").unwrap().resolved_bar_index, Some(0));
        assert_eq!(mgr.get("b").unwrap().resolved_bar_index, Some(2));
        assert_eq!(mgr.get("c").unwrap().resolved_bar_index, None);
    }

    #[test]
    fn test_in_bar_range() {
        let bars = make_bars(&[1000, 2000, 3000, 4000, 5000]);
        let mut mgr = ExecutionMarkManager::new();

        mgr.set(vec![
            ExecutionMark::new(
                "a",
                1000,
                100.0,
                1.0,
                ExecutionSide::Buy,
                ExecutionRole::Entry,
            ),
            ExecutionMark::new(
                "b",
                3000,
                101.0,
                1.0,
                ExecutionSide::Sell,
                ExecutionRole::ScaleOut,
            ),
            ExecutionMark::new(
                "c",
                5000,
                102.0,
                1.0,
                ExecutionSide::Sell,
                ExecutionRole::Exit,
            ),
        ]);

        mgr.resolve_bar_indices(&bars);

        let visible = mgr.in_bar_range(0, 2);
        assert_eq!(visible.len(), 2);

        let visible = mgr.in_bar_range(2, 4);
        assert_eq!(visible.len(), 2);

        let visible = mgr.in_bar_range(0, 4);
        assert_eq!(visible.len(), 3);
    }

    #[test]
    fn test_by_group() {
        let mut mgr = ExecutionMarkManager::new();

        mgr.set(vec![
            ExecutionMark::new(
                "a",
                1000,
                100.0,
                1.0,
                ExecutionSide::Buy,
                ExecutionRole::Entry,
            )
            .with_group_id("trade-1"),
            ExecutionMark::new(
                "b",
                2000,
                101.0,
                0.5,
                ExecutionSide::Sell,
                ExecutionRole::ScaleOut,
            )
            .with_group_id("trade-1"),
            ExecutionMark::new(
                "c",
                3000,
                102.0,
                0.5,
                ExecutionSide::Sell,
                ExecutionRole::Exit,
            )
            .with_group_id("trade-1"),
            ExecutionMark::new(
                "d",
                4000,
                99.0,
                1.0,
                ExecutionSide::Sell,
                ExecutionRole::Entry,
            )
            .with_group_id("trade-2"),
        ]);

        let trade1 = mgr.by_group("trade-1");
        assert_eq!(trade1.len(), 3);

        let trade2 = mgr.by_group("trade-2");
        assert_eq!(trade2.len(), 1);

        let trade3 = mgr.by_group("trade-3");
        assert_eq!(trade3.len(), 0);
    }
}
