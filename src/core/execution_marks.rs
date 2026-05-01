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

use serde::Serialize;
use serde_json::Value;

use crate::core::data::BarArray;
use crate::core::renderer::value_projection::TimeScaleIndex;

/// Version of the execution-mark JSON wire format.
pub const EXECUTION_MARKS_SNAPSHOT_VERSION: u32 = 1;

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
            "buy" | "long" | "b" => Self::Buy,
            "sell" | "short" | "s" => Self::Sell,
            _ => Self::Buy,
        }
    }

    /// Convert to the serialized string key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }

    /// Convert to the rendered uppercase label.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
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
            "entry" | "open" | "start" => Self::Entry,
            "scale_in" | "scalein" | "add" | "pyramid" => Self::ScaleIn,
            "scale_out" | "scaleout" | "partial" | "reduce" => Self::ScaleOut,
            "exit" | "close" | "end" => Self::Exit,
            _ => Self::Entry,
        }
    }

    /// Convert to the serialized string key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::ScaleIn => "scale_in",
            Self::ScaleOut => "scale_out",
            Self::Exit => "exit",
        }
    }

    /// Convert to the rendered uppercase label.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Entry => "ENTRY",
            Self::ScaleIn => "SCALE IN",
            Self::ScaleOut => "SCALE OUT",
            Self::Exit => "EXIT",
        }
    }
}

/// Chart-wide execution label rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionLabelMode {
    /// Render only the trade side (`BUY` / `SELL`).
    #[default]
    SideOnly,
    /// Render only the role (`ENTRY` / `SCALE IN` / `SCALE OUT` / `EXIT`).
    RoleOnly,
    /// Render both side and role (`BUY · ENTRY`).
    SideAndRole,
}

impl ExecutionLabelMode {
    /// Parse from a case-insensitive public key.
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "side" => Some(Self::SideOnly),
            "role" => Some(Self::RoleOnly),
            "side_and_role" | "sideandrole" | "side-and-role" => Some(Self::SideAndRole),
            _ => None,
        }
    }

    /// Public key used by the WASM API.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SideOnly => "side",
            Self::RoleOnly => "role",
            Self::SideAndRole => "side_and_role",
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

    // Internal cached logical index for the current time-scale mapping.
    pub(crate) resolved_time_index: Option<f64>,
    // Backward-compatible integer slot cache derived from the logical index.
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
            resolved_time_index: None,
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

    /// Legacy display helper retained for direct Rust consumers.
    pub fn display_label(&self) -> String {
        format!(
            "{} @ {:.2}",
            format_execution_label(self, ExecutionLabelMode::SideAndRole),
            self.price
        )
    }
}

/// Serialize-ready execution-mark payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SerializedExecutionMark {
    pub id: String,
    pub timestamp_ms: u64,
    pub price: f64,
    pub quantity: f64,
    pub side: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realized_pnl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

impl From<&ExecutionMark> for SerializedExecutionMark {
    fn from(mark: &ExecutionMark) -> Self {
        Self {
            id: mark.id.clone(),
            timestamp_ms: mark.timestamp_ms,
            price: mark.price,
            quantity: mark.quantity,
            side: mark.side.as_str().to_string(),
            role: mark.role.as_str().to_string(),
            order_type: mark.order_type.clone(),
            realized_pnl: mark.realized_pnl,
            label: mark.label.clone(),
            color: mark.color,
            group_id: mark.group_id.clone(),
        }
    }
}

impl SerializedExecutionMark {
    pub fn into_mark(self) -> ExecutionMark {
        let mut mark = ExecutionMark::new(
            self.id,
            self.timestamp_ms,
            self.price,
            self.quantity,
            ExecutionSide::from_str(&self.side),
            ExecutionRole::from_str(&self.role),
        );
        mark.order_type = self.order_type;
        mark.realized_pnl = self.realized_pnl;
        mark.label = self.label;
        mark.color = self.color;
        mark.group_id = self.group_id;
        mark
    }
}

/// Versioned execution-mark snapshot payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExecutionMarksSnapshot {
    pub version: u32,
    pub marks: Vec<SerializedExecutionMark>,
}

/// Screen-space hit area for a rendered execution mark or cluster.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionMarkHitArea {
    pub id: String,
    pub member_ids: Vec<String>,
    pub x_css: f64,
    pub y_css: f64,
    pub radius_css: f64,
}

impl ExecutionMarkHitArea {
    pub fn new(
        id: impl Into<String>,
        member_ids: Vec<String>,
        x_css: f64,
        y_css: f64,
        radius_css: f64,
    ) -> Self {
        let id = id.into();
        let member_ids = if member_ids.is_empty() {
            vec![id.clone()]
        } else {
            member_ids
        };
        Self {
            id,
            member_ids,
            x_css,
            y_css,
            radius_css,
        }
    }

    pub fn contains(&self, x_css: f64, y_css: f64) -> bool {
        let dx = x_css - self.x_css;
        let dy = y_css - self.y_css;
        let radius = self.radius_css.max(1.0);
        (dx * dx) + (dy * dy) <= radius * radius
    }

    pub fn is_cluster(&self) -> bool {
        self.member_ids.len() >= 2
    }
}

/// Pre-projected execution mark used by the renderer-side clustering helper.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionRenderableMark {
    pub id: String,
    pub timestamp_ms: u64,
    pub price: f64,
    pub quantity: f64,
    pub side: ExecutionSide,
    pub role: ExecutionRole,
    pub label: Option<String>,
    pub realized_pnl: Option<f64>,
    pub color: [f32; 4],
    pub group_id: Option<String>,
    pub x_css: f64,
    pub arrow_y_css: f64,
    pub price_y_css: f64,
}

/// Render-time execution cluster.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionCluster {
    pub leader_id: String,
    pub member_ids: Vec<String>,
    pub side: ExecutionSide,
    pub x_css: f64,
    pub y_css: f64,
    pub vwap_price: f64,
    pub hit_area: ExecutionMarkHitArea,
}

impl ExecutionCluster {
    pub fn is_cluster(&self) -> bool {
        self.member_ids.len() >= 2
    }
}

/// Selected-trade locator chevron at an exact execution price.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionLocatorChevron {
    pub id: String,
    pub time_index: f64,
    pub price: f64,
    pub side: ExecutionSide,
    pub color: Option<[f32; 4]>,
}

/// Explicit connector segment type kept only for regression testing.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionLocatorSegment {
    pub from_id: String,
    pub to_id: String,
}

/// Planned selected-trade locators for overlay rendering.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExecutionLocatorPlan {
    pub chevrons: Vec<ExecutionLocatorChevron>,
    pub connector_segments: Vec<ExecutionLocatorSegment>,
}

/// Format the rendered execution label, respecting custom per-mark overrides.
pub fn format_execution_label(mark: &ExecutionMark, mode: ExecutionLabelMode) -> String {
    if let Some(label) = &mark.label {
        return label.clone();
    }

    match mode {
        ExecutionLabelMode::SideOnly => mark.side.as_label().to_string(),
        ExecutionLabelMode::RoleOnly => mark.role.as_label().to_string(),
        ExecutionLabelMode::SideAndRole => {
            format!("{} · {}", mark.side.as_label(), mark.role.as_label())
        }
    }
}

/// Format realized P&L for execution labels.
pub fn format_execution_pnl(realized_pnl: f64, reference_price: f64) -> String {
    let decimals = if reference_price.abs() < 1.0 { 4 } else { 2 };
    if realized_pnl > 0.0 {
        format!("+${:.*}", decimals, realized_pnl)
    } else if realized_pnl < 0.0 {
        format!("-${:.*}", decimals, realized_pnl.abs())
    } else {
        format!("${:.*}", decimals, 0.0)
    }
}

/// Build the visible text lines for an execution mark.
pub fn build_execution_text_lines(
    mark: &ExecutionMark,
    _label_mode: ExecutionLabelMode,
    _show_pnl: bool,
    price_step: f64,
) -> Vec<String> {
    let price = format_execution_text_price(mark.price, price_step);
    let line = if mark.quantity > 0.0 {
        format!(
            "{} @ {}",
            crate::core::formatters::format_qty(mark.quantity),
            price
        )
    } else {
        format!("@ {}", price)
    };

    vec![line]
}

fn format_execution_text_price(price: f64, price_step: f64) -> String {
    let decimals = if price_step <= 0.0 {
        2
    } else {
        let mut precision = 0usize;
        let mut step = price_step;
        while step < 0.9999 && precision < 8 {
            step *= 10.0;
            precision += 1;
        }
        precision
    };

    format!("{price:.decimals$}")
}

/// Find the top-most execution-mark hit area under the pointer.
pub fn hit_test_execution_mark_hit_areas(
    hit_areas: &[ExecutionMarkHitArea],
    x_css: f64,
    y_css: f64,
) -> Option<&ExecutionMarkHitArea> {
    hit_areas
        .iter()
        .rev()
        .find(|hit_area| hit_area.contains(x_css, y_css))
}

/// Cluster visible execution marks by side and projected X distance.
pub fn cluster_execution_mark_renderables(
    renderables: &[ExecutionRenderableMark],
    cluster_threshold_px: f64,
    base_hit_radius_css: f64,
) -> Vec<ExecutionCluster> {
    if renderables.is_empty() {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, &ExecutionRenderableMark)> =
        renderables.iter().enumerate().collect();
    indexed.sort_by(|(a_idx, a), (b_idx, b)| {
        a.x_css
            .total_cmp(&b.x_css)
            .then_with(|| a.side.as_str().cmp(b.side.as_str()))
            .then_with(|| a.timestamp_ms.cmp(&b.timestamp_ms))
            .then_with(|| a_idx.cmp(b_idx))
    });

    let mut clusters = Vec::new();
    let mut current_group: Vec<&ExecutionRenderableMark> = Vec::new();
    let mut current_side = indexed[0].1.side;
    let mut last_x = indexed[0].1.x_css;

    for (_, renderable) in indexed {
        let should_start_new_group = current_group.is_empty()
            || renderable.side != current_side
            || cluster_threshold_px <= 0.0
            || (renderable.x_css - last_x).abs() >= cluster_threshold_px;

        if should_start_new_group && !current_group.is_empty() {
            clusters.push(cluster_group(&current_group, base_hit_radius_css));
            current_group.clear();
        }

        current_side = renderable.side;
        last_x = renderable.x_css;
        current_group.push(renderable);
    }

    if !current_group.is_empty() {
        clusters.push(cluster_group(&current_group, base_hit_radius_css));
    }

    clusters
}

fn cluster_group(group: &[&ExecutionRenderableMark], base_hit_radius_css: f64) -> ExecutionCluster {
    let leader_id = group
        .first()
        .map(|renderable| renderable.id.clone())
        .unwrap_or_default();
    let side = group
        .first()
        .map(|renderable| renderable.side)
        .unwrap_or(ExecutionSide::Buy);
    let member_ids: Vec<String> = group
        .iter()
        .map(|renderable| renderable.id.clone())
        .collect();

    let (x_css, y_css, vwap_price, radius_css) = if group.len() == 1 {
        let renderable = group[0];
        (
            renderable.x_css,
            renderable.arrow_y_css,
            renderable.price,
            base_hit_radius_css.max(1.0),
        )
    } else {
        let (weighted_x, weighted_y, weighted_price, total_weight) =
            group.iter().fold((0.0, 0.0, 0.0, 0.0), |acc, renderable| {
                let weight = renderable.quantity.abs().max(f64::EPSILON);
                (
                    acc.0 + renderable.x_css * weight,
                    acc.1 + renderable.price_y_css * weight,
                    acc.2 + renderable.price * weight,
                    acc.3 + weight,
                )
            });

        let x_css = weighted_x / total_weight.max(f64::EPSILON);
        let y_css = weighted_y / total_weight.max(f64::EPSILON);
        let vwap_price = weighted_price / total_weight.max(f64::EPSILON);
        let max_distance = group.iter().fold(0.0_f64, |acc, renderable| {
            let dx = renderable.x_css - x_css;
            let dy = renderable.price_y_css - y_css;
            acc.max(((dx * dx) + (dy * dy)).sqrt())
        });
        (
            x_css,
            y_css,
            vwap_price,
            base_hit_radius_css.max(1.0) + max_distance,
        )
    };

    ExecutionCluster {
        leader_id: leader_id.clone(),
        member_ids: member_ids.clone(),
        side,
        x_css,
        y_css,
        vwap_price,
        hit_area: ExecutionMarkHitArea::new(leader_id, member_ids, x_css, y_css, radius_css),
    }
}

/// Build the selected-trade locator plan.
///
/// Grouped trades intentionally emit only per-fill chevrons. Connector segments
/// are never generated.
pub fn build_selected_trade_locator_plan(
    execution_marks: &ExecutionMarkManager,
    selected_mark_id: Option<&str>,
) -> ExecutionLocatorPlan {
    let Some(selected_id) = selected_mark_id else {
        return ExecutionLocatorPlan::default();
    };
    let Some(selected_mark) = execution_marks.get(selected_id) else {
        return ExecutionLocatorPlan::default();
    };
    let Some(group_id) = selected_mark.group_id.as_deref() else {
        return ExecutionLocatorPlan::default();
    };

    let mut group_marks = execution_marks.by_group(group_id);
    if group_marks.len() < 2 {
        return ExecutionLocatorPlan::default();
    }
    group_marks.sort_by(|a, b| {
        a.timestamp_ms
            .cmp(&b.timestamp_ms)
            .then_with(|| a.id.cmp(&b.id))
    });

    ExecutionLocatorPlan {
        chevrons: group_marks
            .into_iter()
            .filter_map(|mark| {
                mark.resolved_time_index
                    .map(|time_index| ExecutionLocatorChevron {
                        id: mark.id.clone(),
                        time_index,
                        price: mark.price,
                        side: mark.side,
                        color: mark.color,
                    })
            })
            .collect(),
        connector_segments: Vec::new(),
    }
}

/// Convert manager contents into the wrapped versioned snapshot shape.
pub fn execution_marks_snapshot(marks: &ExecutionMarkManager) -> ExecutionMarksSnapshot {
    let mut serialized: Vec<_> = marks.iter().map(SerializedExecutionMark::from).collect();
    serialized.sort_by(|a, b| a.id.cmp(&b.id));
    ExecutionMarksSnapshot {
        version: EXECUTION_MARKS_SNAPSHOT_VERSION,
        marks: serialized,
    }
}

/// Stub version-to-version migration seam for future snapshot upgrades.
pub fn migrate_execution_marks_snapshot(value: Value, from_version: u32) -> Result<Value, String> {
    match from_version {
        1 => Ok(value),
        _ => Err(format!(
            "unsupported execution marks snapshot migration from version {}",
            from_version
        )),
    }
}

/// Parse execution marks from either the legacy bare array or the wrapped snapshot.
pub fn parse_execution_marks_snapshot_value(
    value: &Value,
) -> Result<ExecutionMarksSnapshot, String> {
    match value {
        Value::Array(items) => Ok(ExecutionMarksSnapshot {
            version: EXECUTION_MARKS_SNAPSHOT_VERSION,
            marks: parse_execution_mark_items(items)?,
        }),
        Value::Object(map) => {
            let version = map
                .get("version")
                .and_then(Value::as_u64)
                .ok_or_else(|| "execution marks snapshot missing version".to_string())?
                as u32;
            if version > EXECUTION_MARKS_SNAPSHOT_VERSION {
                return Err(format!(
                    "execution marks snapshot version {} is newer than supported version {}",
                    version, EXECUTION_MARKS_SNAPSHOT_VERSION
                ));
            }
            let migrated = migrate_execution_marks_snapshot(value.clone(), version)?;
            let migrated_object = migrated
                .as_object()
                .ok_or_else(|| "execution marks snapshot must be a JSON object".to_string())?;
            let migrated_marks = migrated_object
                .get("marks")
                .and_then(Value::as_array)
                .ok_or_else(|| "execution marks snapshot missing marks array".to_string())?;

            Ok(ExecutionMarksSnapshot {
                version: EXECUTION_MARKS_SNAPSHOT_VERSION,
                marks: parse_execution_mark_items(migrated_marks)?,
            })
        }
        _ => Err("execution marks snapshot must be a JSON array or object".to_string()),
    }
}

fn parse_execution_mark_items(items: &[Value]) -> Result<Vec<SerializedExecutionMark>, String> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| parse_execution_mark_item(item, index))
        .collect()
}

fn parse_execution_mark_item(
    item: &Value,
    index: usize,
) -> Result<SerializedExecutionMark, String> {
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("execution mark {} missing id", index))?;
    let timestamp_ms = item
        .get("timestamp_ms")
        .or_else(|| item.get("timestampMs"))
        .or_else(|| item.get("timestamp"))
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("execution mark {} missing timestamp_ms", index))?;
    let price = item
        .get("price")
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("execution mark {} missing price", index))?;
    let quantity = item
        .get("quantity")
        .or_else(|| item.get("qty"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let side = item
        .get("side")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("execution mark {} missing side", index))?;
    let role = item
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("execution mark {} missing role", index))?;

    Ok(SerializedExecutionMark {
        id: id.to_string(),
        timestamp_ms,
        price,
        quantity,
        side: ExecutionSide::from_str(side).as_str().to_string(),
        role: ExecutionRole::from_str(role).as_str().to_string(),
        order_type: item
            .get("order_type")
            .or_else(|| item.get("orderType"))
            .and_then(Value::as_str)
            .map(str::to_string),
        realized_pnl: item
            .get("realized_pnl")
            .or_else(|| item.get("realizedPnl"))
            .and_then(Value::as_f64),
        label: item
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string),
        color: parse_color_json_value(item.get("color").unwrap_or(&Value::Null)),
        group_id: item
            .get("group_id")
            .or_else(|| item.get("groupId"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn parse_color_json_value(value: &Value) -> Option<[f32; 4]> {
    let arr = value.as_array()?;
    if arr.len() != 4 {
        return None;
    }

    let mut color = [0.0_f32; 4];
    for (index, component) in arr.iter().enumerate() {
        color[index] = component.as_f64()? as f32;
    }
    Some(color)
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
            self.sorted_ids.sort_by_key(|id| {
                self.marks
                    .get(id)
                    .map(|mark| mark.timestamp_ms)
                    .unwrap_or(0)
            });
            self.dirty_sort = false;
        }
    }

    /// Get marks in a timestamp range [start_ms, end_ms] (inclusive).
    pub fn in_time_range(&mut self, start_ms: u64, end_ms: u64) -> Vec<&ExecutionMark> {
        self.ensure_sorted();
        self.sorted_ids
            .iter()
            .filter_map(|id| self.marks.get(id))
            .filter(|mark| mark.timestamp_ms >= start_ms && mark.timestamp_ms <= end_ms)
            .collect()
    }

    /// Get marks visible in a bar index range [start_idx, end_idx].
    /// Requires resolve_bar_indices to have been called first.
    pub fn in_bar_range(&self, start_idx: usize, end_idx: usize) -> Vec<&ExecutionMark> {
        self.marks
            .values()
            .filter(|mark| {
                mark.resolved_bar_index
                    .map(|idx| idx >= start_idx && idx <= end_idx)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get marks visible in a logical index range [start, end].
    pub fn in_logical_range(&self, start: f64, end: f64) -> Vec<&ExecutionMark> {
        self.marks
            .values()
            .filter(|mark| {
                mark.resolved_time_index
                    .map(|index| index >= start && index <= end)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Resolve timestamps to bar indices using the provided bar data.
    ///
    /// This backwards-compatible helper rebuilds a temporary [`TimeScaleIndex`].
    /// Runtime code should prefer [`Self::resolve_time_scale_indices`] so the
    /// shared time-scale contract stays canonical.
    pub fn resolve_bar_indices(&mut self, bars: &BarArray) {
        let time_scale = TimeScaleIndex::from_bars(bars);
        self.resolve_time_scale_indices(&time_scale);
    }

    /// Resolve timestamps through the shared time-scale contract.
    pub fn resolve_time_scale_indices(&mut self, time_scale: &TimeScaleIndex) {
        if time_scale.is_empty() {
            for mark in self.marks.values_mut() {
                mark.resolved_time_index = None;
                mark.resolved_bar_index = None;
            }
            return;
        }

        let first_timestamp = time_scale.timestamp_at(0).unwrap_or(0);
        let last_timestamp = time_scale
            .timestamp_at(time_scale.len().saturating_sub(1))
            .unwrap_or(first_timestamp);

        for mark in self.marks.values_mut() {
            mark.resolved_time_index = if mark.timestamp_ms < first_timestamp {
                None
            } else if mark.timestamp_ms >= last_timestamp {
                Some(time_scale.len().saturating_sub(1) as f64)
            } else {
                time_scale.logical_index_for_timestamp(mark.timestamp_ms)
            };
            mark.resolved_bar_index = mark
                .resolved_time_index
                .and_then(|index| time_scale.nearest_main_bar_index_for_logical(index));
        }
    }

    /// Get all marks with their resolved bar indices, for rendering.
    pub fn visible_marks_for_render(
        &self,
        start_bar: usize,
        end_bar: usize,
    ) -> Vec<&ExecutionMark> {
        self.in_logical_range(start_bar as f64, end_bar as f64)
    }

    /// Get marks by group ID.
    pub fn by_group(&self, group_id: &str) -> Vec<&ExecutionMark> {
        self.marks
            .values()
            .filter(|mark| mark.group_id.as_deref() == Some(group_id))
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
    let first_ts = bars.timestamp(0);
    let last_ts = bars.timestamp(len - 1);

    if timestamp_ms < first_ts {
        return None;
    }
    if timestamp_ms >= last_ts {
        return Some(len - 1);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::data::Bar;

    fn make_bar(ts: u64) -> Bar {
        Bar::new(ts, 100.0, 101.0, 99.0, 100.5, 1000.0)
    }

    fn make_bars(timestamps: &[u64]) -> BarArray {
        let mut bars = BarArray::new();
        bars.set(timestamps.iter().map(|&ts| make_bar(ts)).collect())
            .unwrap();
        bars
    }

    fn sample_mark(id: &str, side: ExecutionSide, role: ExecutionRole) -> ExecutionMark {
        ExecutionMark::new(id, 1_700_000_000_000, 103_842.5712345, 1.5, side, role)
    }

    fn sample_renderable(
        id: &str,
        side: ExecutionSide,
        x_css: f64,
        price: f64,
        quantity: f64,
    ) -> ExecutionRenderableMark {
        ExecutionRenderableMark {
            id: id.to_string(),
            timestamp_ms: 1_700_000_000_000,
            price,
            quantity,
            side,
            role: ExecutionRole::Entry,
            label: None,
            realized_pnl: None,
            color: [1.0, 1.0, 1.0, 1.0],
            group_id: None,
            x_css,
            arrow_y_css: 40.0,
            price_y_css: 100.0 - price,
        }
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
        assert!(!mgr.remove("1"));
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

        assert_eq!(timestamp_to_bar_index(1000, &bars), Some(0));
        assert_eq!(timestamp_to_bar_index(3000, &bars), Some(2));
        assert_eq!(timestamp_to_bar_index(5000, &bars), Some(4));
        assert_eq!(timestamp_to_bar_index(1500, &bars), Some(0));
        assert_eq!(timestamp_to_bar_index(2500, &bars), Some(1));
        assert_eq!(timestamp_to_bar_index(4999, &bars), Some(3));
        assert_eq!(timestamp_to_bar_index(500, &bars), None);
        assert_eq!(timestamp_to_bar_index(6000, &bars), Some(4));
    }

    #[test]
    fn test_bar_index_to_timestamp() {
        let bars = make_bars(&[1000, 2000, 3000, 4000, 5000]);

        assert_eq!(bar_index_to_timestamp(0, &bars), Some(1000));
        assert_eq!(bar_index_to_timestamp(2, &bars), Some(3000));
        assert_eq!(bar_index_to_timestamp(4, &bars), Some(5000));
        assert_eq!(bar_index_to_timestamp(10, &bars), None);
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
            ),
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

    #[test]
    fn bar_preserves_crypto_precision() {
        let bar = Bar::new(
            1_700_000_000_000,
            103_842.57_f64,
            103_842.58_f64,
            103_842.56_f64,
            103_842.5712345_f64,
            1_000.0_f64,
        );
        assert_eq!(bar.close, 103_842.5712345_f64);
        let mut arr = BarArray::new();
        arr.set(vec![bar]).unwrap();
        assert_eq!(arr.get(0).unwrap().close, 103_842.5712345_f64);
    }

    #[test]
    fn bar_preserves_small_alt_precision() {
        let bar = Bar::new(
            1_700_000_000_000,
            0.0000001234_f64,
            0.0000001235_f64,
            0.0000001233_f64,
            0.00000012345678_f64,
            1.0_f64,
        );
        let mut arr = BarArray::new();
        arr.set(vec![bar]).unwrap();
        assert_eq!(arr.get(0).unwrap().close, 0.00000012345678_f64);
    }

    #[test]
    fn label_mode_formats_every_role_and_side() {
        let cases = [
            (
                ExecutionSide::Buy,
                ExecutionRole::Entry,
                "BUY",
                "ENTRY",
                "BUY · ENTRY",
            ),
            (
                ExecutionSide::Buy,
                ExecutionRole::ScaleIn,
                "BUY",
                "SCALE IN",
                "BUY · SCALE IN",
            ),
            (
                ExecutionSide::Buy,
                ExecutionRole::ScaleOut,
                "BUY",
                "SCALE OUT",
                "BUY · SCALE OUT",
            ),
            (
                ExecutionSide::Buy,
                ExecutionRole::Exit,
                "BUY",
                "EXIT",
                "BUY · EXIT",
            ),
            (
                ExecutionSide::Sell,
                ExecutionRole::Entry,
                "SELL",
                "ENTRY",
                "SELL · ENTRY",
            ),
            (
                ExecutionSide::Sell,
                ExecutionRole::ScaleIn,
                "SELL",
                "SCALE IN",
                "SELL · SCALE IN",
            ),
            (
                ExecutionSide::Sell,
                ExecutionRole::ScaleOut,
                "SELL",
                "SCALE OUT",
                "SELL · SCALE OUT",
            ),
            (
                ExecutionSide::Sell,
                ExecutionRole::Exit,
                "SELL",
                "EXIT",
                "SELL · EXIT",
            ),
        ];

        for (side, role, side_only, role_only, both) in cases {
            let mark = sample_mark("label", side, role);
            assert_eq!(
                format_execution_label(&mark, ExecutionLabelMode::SideOnly),
                side_only
            );
            assert_eq!(
                format_execution_label(&mark, ExecutionLabelMode::RoleOnly),
                role_only
            );
            assert_eq!(
                format_execution_label(&mark, ExecutionLabelMode::SideAndRole),
                both
            );
        }
    }

    #[test]
    fn label_mode_custom_label_override_wins() {
        let mark = sample_mark("custom", ExecutionSide::Buy, ExecutionRole::ScaleOut)
            .with_label("MANUAL OVERRIDE");
        assert_eq!(
            format_execution_label(&mark, ExecutionLabelMode::SideOnly),
            "MANUAL OVERRIDE"
        );
        assert_eq!(
            format_execution_label(&mark, ExecutionLabelMode::RoleOnly),
            "MANUAL OVERRIDE"
        );
        assert_eq!(
            format_execution_label(&mark, ExecutionLabelMode::SideAndRole),
            "MANUAL OVERRIDE"
        );
    }

    #[test]
    fn pnl_formatter_uses_two_decimals_for_normal_price_ranges() {
        assert_eq!(format_execution_pnl(150.0, 103_842.57), "+$150.00");
        assert_eq!(format_execution_pnl(-67.891, 103_842.57), "-$67.89");
        assert_eq!(format_execution_pnl(0.0, 103_842.57), "$0.00");
    }

    #[test]
    fn pnl_formatter_uses_four_decimals_for_sub_dollar_price_ranges() {
        assert_eq!(format_execution_pnl(0.125, 0.0000001234), "+$0.1250");
        assert_eq!(format_execution_pnl(-0.5, 0.25), "-$0.5000");
        assert_eq!(format_execution_pnl(0.0, 0.5), "$0.0000");
    }

    #[test]
    fn execution_text_lines_only_include_quantity_and_price() {
        let mark =
            sample_mark("exit", ExecutionSide::Sell, ExecutionRole::Exit).with_realized_pnl(150.0);
        let lines = build_execution_text_lines(&mark, ExecutionLabelMode::SideOnly, true, 0.01);
        assert_eq!(lines, vec!["1.5 @ 103842.57"]);
    }

    #[test]
    fn execution_text_lines_skip_side_role_and_custom_label_text() {
        let mark = sample_mark("entry", ExecutionSide::Buy, ExecutionRole::Entry)
            .with_label("BUY ENTRY")
            .with_realized_pnl(150.0);
        let lines = build_execution_text_lines(&mark, ExecutionLabelMode::SideOnly, true, 0.01);
        assert_eq!(lines, vec!["1.5 @ 103842.57"]);
    }

    #[test]
    fn clustering_groups_marks_by_same_side_and_threshold() {
        let renderables = vec![
            sample_renderable("a", ExecutionSide::Buy, 10.0, 100.0, 1.0),
            sample_renderable("b", ExecutionSide::Buy, 18.0, 101.0, 1.0),
            sample_renderable("c", ExecutionSide::Buy, 26.5, 102.0, 1.0),
            sample_renderable("d", ExecutionSide::Sell, 27.0, 103.0, 1.0),
            sample_renderable("e", ExecutionSide::Sell, 35.0, 104.0, 1.0),
            sample_renderable("f", ExecutionSide::Buy, 80.0, 105.0, 1.0),
            sample_renderable("g", ExecutionSide::Buy, 82.0, 106.0, 1.0),
            sample_renderable("h", ExecutionSide::Buy, 120.0, 107.0, 1.0),
            sample_renderable("i", ExecutionSide::Sell, 160.0, 108.0, 1.0),
            sample_renderable("j", ExecutionSide::Sell, 166.0, 109.0, 1.0),
        ];

        let clusters = cluster_execution_mark_renderables(&renderables, 14.0, 14.0);
        let member_counts: Vec<usize> = clusters
            .iter()
            .map(|cluster| cluster.member_ids.len())
            .collect();
        assert_eq!(member_counts, vec![3, 2, 2, 1, 2]);
    }

    #[test]
    fn clustering_vwap_is_quantity_weighted() {
        let renderables = vec![
            sample_renderable("a", ExecutionSide::Buy, 10.0, 100.0, 1.0),
            sample_renderable("b", ExecutionSide::Buy, 12.0, 110.0, 3.0),
        ];

        let clusters = cluster_execution_mark_renderables(&renderables, 14.0, 14.0);
        assert_eq!(clusters.len(), 1);
        assert!((clusters[0].vwap_price - 107.5).abs() < 1e-9);
    }

    #[test]
    fn clustering_threshold_zero_restores_per_mark_rendering() {
        let renderables = vec![
            sample_renderable("a", ExecutionSide::Buy, 10.0, 100.0, 1.0),
            sample_renderable("b", ExecutionSide::Buy, 12.0, 101.0, 1.0),
            sample_renderable("c", ExecutionSide::Buy, 14.0, 102.0, 1.0),
        ];

        let clusters = cluster_execution_mark_renderables(&renderables, 0.0, 14.0);
        assert_eq!(clusters.len(), 3);
        assert!(clusters.iter().all(|cluster| !cluster.is_cluster()));
    }

    #[test]
    fn clustering_twenty_same_timestamp_marks_forms_one_cluster() {
        let renderables: Vec<_> = (0..20)
            .map(|index| {
                sample_renderable(
                    &format!("m{index}"),
                    ExecutionSide::Buy,
                    10.0,
                    100.0 + index as f64 * 0.1,
                    1.0,
                )
            })
            .collect();

        let clusters = cluster_execution_mark_renderables(&renderables, 14.0, 14.0);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].member_ids.len(), 20);
    }

    #[test]
    fn cluster_hit_area_covers_all_cluster_members() {
        let renderables = vec![
            sample_renderable("a", ExecutionSide::Buy, 10.0, 100.0, 1.0),
            sample_renderable("b", ExecutionSide::Buy, 18.0, 104.0, 1.0),
            sample_renderable("c", ExecutionSide::Buy, 24.0, 108.0, 1.0),
        ];

        let clusters = cluster_execution_mark_renderables(&renderables, 20.0, 8.0);
        let cluster = &clusters[0];
        for renderable in &renderables {
            assert!(
                cluster
                    .hit_area
                    .contains(renderable.x_css, renderable.price_y_css),
                "cluster hit area should cover {}",
                renderable.id
            );
        }
    }

    #[test]
    fn hit_test_returns_topmost_cluster_hit_area() {
        let hit_areas = vec![
            ExecutionMarkHitArea::new("a", vec!["a".to_string()], 10.0, 10.0, 4.0),
            ExecutionMarkHitArea::new(
                "leader",
                vec!["leader".to_string(), "member-2".to_string()],
                10.0,
                10.0,
                8.0,
            ),
        ];

        let hit = hit_test_execution_mark_hit_areas(&hit_areas, 10.0, 10.0).unwrap();
        assert_eq!(hit.id, "leader");
        assert!(hit.is_cluster());
    }

    #[test]
    fn selected_trade_locator_plan_emits_per_fill_chevrons_and_no_connectors() {
        let bars = make_bars(&[1000, 2000, 3000, 4000]);
        let mut mgr = ExecutionMarkManager::new();
        mgr.set(vec![
            ExecutionMark::new(
                "entry",
                1000,
                100.0,
                1.0,
                ExecutionSide::Buy,
                ExecutionRole::Entry,
            )
            .with_group_id("trade-1"),
            ExecutionMark::new(
                "scale-out",
                2000,
                101.0,
                0.5,
                ExecutionSide::Sell,
                ExecutionRole::ScaleOut,
            )
            .with_group_id("trade-1"),
            ExecutionMark::new(
                "exit",
                3000,
                102.0,
                0.5,
                ExecutionSide::Sell,
                ExecutionRole::Exit,
            )
            .with_group_id("trade-1"),
        ]);
        mgr.resolve_bar_indices(&bars);

        let plan = build_selected_trade_locator_plan(&mgr, Some("entry"));
        assert_eq!(plan.chevrons.len(), 3);
        assert!(plan.connector_segments.is_empty());
    }

    #[test]
    fn execution_marks_snapshot_round_trip_preserves_marks() {
        let mut mgr = ExecutionMarkManager::new();
        mgr.set(vec![
            sample_mark("a", ExecutionSide::Buy, ExecutionRole::Entry).with_group_id("trade-1"),
            sample_mark("b", ExecutionSide::Sell, ExecutionRole::Exit)
                .with_realized_pnl(150.0)
                .with_order_type("limit"),
        ]);

        let snapshot = execution_marks_snapshot(&mgr);
        let value = serde_json::to_value(&snapshot).unwrap();
        let parsed = parse_execution_marks_snapshot_value(&value).unwrap();

        assert_eq!(parsed.version, EXECUTION_MARKS_SNAPSHOT_VERSION);
        assert_eq!(parsed.marks, snapshot.marks);
    }

    #[test]
    fn execution_marks_snapshot_accepts_legacy_bare_array() {
        let payload = serde_json::json!([
            {
                "id": "legacy",
                "timestamp_ms": 1_700_000_000_000u64,
                "price": 100.0,
                "quantity": 1.0,
                "side": "buy",
                "role": "entry"
            }
        ]);

        let parsed = parse_execution_marks_snapshot_value(&payload).unwrap();
        assert_eq!(parsed.version, EXECUTION_MARKS_SNAPSHOT_VERSION);
        assert_eq!(parsed.marks.len(), 1);
        assert_eq!(parsed.marks[0].id, "legacy");
    }

    #[test]
    fn execution_marks_snapshot_rejects_future_version() {
        let payload = serde_json::json!({
            "version": 999,
            "marks": []
        });

        let error = parse_execution_marks_snapshot_value(&payload).unwrap_err();
        assert!(error.contains("newer than supported version"));
    }

    #[test]
    fn execution_marks_snapshot_migration_stub_accepts_v1() {
        let payload = serde_json::json!({
            "version": 1,
            "marks": []
        });
        let migrated = migrate_execution_marks_snapshot(payload.clone(), 1).unwrap();
        assert_eq!(migrated, payload);
    }
}
