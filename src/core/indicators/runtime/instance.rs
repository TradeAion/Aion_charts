use crate::core::indicators::render::types::{DrawInstruction, ObjectMutation};
use crate::core::indicators::runtime::events::RuntimeEvent;
use crate::core::indicators::runtime::limits::{ResourceCounters, ResourceLimits};
use crate::core::indicators::runtime::value::RayValue;
use crate::core::indicators::runtime::var_series::VarSeries;
use crate::core::indicators::{
    IndicatorFrameOutput, IndicatorInstanceId, IndicatorProgramId, ObjectState,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/// A single plotshape entry emitted for one bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeEntry {
    pub timestamp: u64,
    pub value: f64,
}

/// Accumulated plot data for one IR call position, built up across bars.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlotAccumulator {
    pub line_points: Vec<(u64, f64)>,
    pub area_points: Vec<(u64, f64)>,
    pub histogram_points: Vec<(u64, f64)>,
    /// Per-point histogram base values (one per histogram_points entry).
    pub histogram_bases: Vec<f64>,
    /// Per-point histogram colors for dynamic styling (one per histogram_points entry).
    /// If empty, uses default color.
    pub histogram_colors: Vec<[f32; 4]>,
    pub bar_points: Vec<(u64, f64, f64, f64, f64)>,
    pub candle_points: Vec<(u64, f64, f64, f64, f64)>,
    pub shape_entries: Vec<ShapeEntry>,
}

/// State accumulated incrementally across bars during VM execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IncrementalState {
    /// Persistent variables (`var`-declared), survive across bars.
    pub persistent_vars: HashMap<String, RayValue>,
    /// Per-variable bar-aligned history for indexed lookups (e.g. x[1]).
    pub var_series: HashMap<String, VarSeries>,
    /// Accumulated plot data keyed by IR call position index.
    pub plot_data: HashMap<usize, PlotAccumulator>,
    /// FillBetween metadata keyed by IR call position (constant across bars).
    pub fill_between: HashMap<usize, FillBetweenMeta>,
    /// Number of bars processed so far (for incremental update tracking).
    pub bars_processed: usize,
}

/// Metadata for a FillBetween instruction (does not vary per bar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillBetweenMeta {
    pub upper_series_id: String,
    pub lower_series_id: String,
    pub z: i16,
    pub declaration_order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorInstance {
    pub instance_id: IndicatorInstanceId,
    pub program_id: IndicatorProgramId,
    pub enabled: bool,
    pub inputs: serde_json::Value,
    pub limits: ResourceLimits,
    pub counters: ResourceCounters,
    pub object_registry: BTreeMap<u64, ObjectState>,
    pub last_good_frame: Option<IndicatorFrameOutput>,
    pub recent_events: Vec<RuntimeEvent>,
    pub updates_applied: u64,
    /// Incremental execution state accumulated across bars by the VM.
    pub incremental_state: IncrementalState,
}

impl IndicatorInstance {
    pub fn new(
        instance_id: IndicatorInstanceId,
        program_id: IndicatorProgramId,
        inputs: serde_json::Value,
    ) -> Self {
        Self {
            instance_id,
            program_id,
            enabled: true,
            inputs,
            limits: ResourceLimits::default(),
            counters: ResourceCounters::default(),
            object_registry: BTreeMap::new(),
            last_good_frame: None,
            recent_events: Vec::new(),
            updates_applied: 0,
            incremental_state: IncrementalState::default(),
        }
    }

    /// Clear all incremental execution state for a full re-run.
    pub fn reset_incremental_state(&mut self) {
        self.incremental_state = IncrementalState::default();
    }

    pub fn push_event(&mut self, event: RuntimeEvent) {
        self.recent_events.push(event);
        if self.recent_events.len() > 64 {
            let drain = self.recent_events.len().saturating_sub(64);
            self.recent_events.drain(0..drain);
        }
    }

    pub fn apply_object_mutations(&mut self, mutations: &[ObjectMutation]) {
        for mutation in mutations {
            match mutation {
                ObjectMutation::Create {
                    id,
                    object_type,
                    layer_band,
                    z,
                    props,
                } => {
                    self.object_registry.insert(
                        *id,
                        ObjectState {
                            object_type: object_type.clone(),
                            layer_band: *layer_band,
                            z: *z,
                            anchors: Value::Null,
                            style: Value::Null,
                            lifetime: "persistent".to_string(),
                            mutable_props: props.clone(),
                        },
                    );
                }
                ObjectMutation::Update { id, props } => {
                    if let Some(state) = self.object_registry.get_mut(id) {
                        merge_json(&mut state.mutable_props, props);
                    }
                }
                ObjectMutation::Delete { id } => {
                    self.object_registry.remove(id);
                }
            }
        }

        self.counters.peak_objects = self.counters.peak_objects.max(self.object_registry.len());
    }

    pub fn estimate_memory_bytes(&self) -> usize {
        let mut total = 0usize;
        total = total.saturating_add(estimate_json_bytes(&self.inputs));
        total = total.saturating_add(self.recent_events.len().saturating_mul(96));
        for state in self.object_registry.values() {
            total = total.saturating_add(128);
            total = total.saturating_add(state.object_type.len());
            total = total.saturating_add(state.lifetime.len());
            total = total.saturating_add(estimate_json_bytes(&state.anchors));
            total = total.saturating_add(estimate_json_bytes(&state.style));
            total = total.saturating_add(estimate_json_bytes(&state.mutable_props));
        }
        if let Some(frame) = &self.last_good_frame {
            total = total.saturating_add(estimate_frame_bytes(frame));
        }
        // Account for incremental state
        total = total.saturating_add(
            self.incremental_state
                .persistent_vars
                .len()
                .saturating_mul(48),
        );
        for (name, series) in &self.incremental_state.var_series {
            total = total
                .saturating_add(name.len())
                .saturating_add(series.len().saturating_mul(16));
        }
        for acc in self.incremental_state.plot_data.values() {
            total = total.saturating_add(acc.line_points.len().saturating_mul(16));
            total = total.saturating_add(acc.area_points.len().saturating_mul(16));
            total = total.saturating_add(acc.histogram_points.len().saturating_mul(16));
            total = total.saturating_add(acc.histogram_bases.len().saturating_mul(8));
            total = total.saturating_add(acc.bar_points.len().saturating_mul(40));
            total = total.saturating_add(acc.candle_points.len().saturating_mul(40));
            total = total.saturating_add(acc.shape_entries.len().saturating_mul(16));
        }
        total
    }
}

fn merge_json(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                if let Some(existing) = target_map.get_mut(key) {
                    merge_json(existing, value);
                } else {
                    target_map.insert(key.clone(), value.clone());
                }
            }
        }
        (target_value, patch_value) => {
            *target_value = patch_value.clone();
        }
    }
}

fn estimate_frame_bytes(frame: &IndicatorFrameOutput) -> usize {
    let mut total = 96usize;
    for inst in &frame.instructions {
        total = total.saturating_add(match inst {
            DrawInstruction::PlotLine {
                series_id, points, ..
            } => 48 + series_id.len() + points.len().saturating_mul(16),
            DrawInstruction::PlotArea {
                series_id, points, ..
            } => 64 + series_id.len() + points.len().saturating_mul(16),
            DrawInstruction::PlotHistogram {
                series_id, points, ..
            } => 56 + series_id.len() + points.len().saturating_mul(16),
            DrawInstruction::PlotBar {
                series_id, points, ..
            } => 72 + series_id.len() + points.len().saturating_mul(40),
            DrawInstruction::PlotCandle {
                series_id, points, ..
            } => 72 + series_id.len() + points.len().saturating_mul(40),
            DrawInstruction::PlotShape { shape, .. } => 64 + shape.len(),
            DrawInstruction::DrawLabel { text, .. } => 72 + text.len(),
            DrawInstruction::DrawBox { .. } => 96,
            DrawInstruction::DrawLine { style, extend, .. } => 96 + style.len() + extend.len(),
            DrawInstruction::DrawPolyline { points, .. } => 72 + points.len().saturating_mul(16),
            DrawInstruction::FillBetween {
                upper_series_id,
                lower_series_id,
                ..
            } => 64 + upper_series_id.len() + lower_series_id.len(),
        });
    }

    for mutation in &frame.object_mutations {
        total = total.saturating_add(match mutation {
            ObjectMutation::Create {
                object_type, props, ..
            } => 96 + object_type.len() + estimate_json_bytes(props),
            ObjectMutation::Update { props, .. } => 64 + estimate_json_bytes(props),
            ObjectMutation::Delete { .. } => 32,
        });
    }

    for sample in &frame.mtf_samples {
        total = total.saturating_add(96 + sample.request_id.len() + sample.source_timeframe.len());
    }

    total
}

fn estimate_json_bytes(value: &Value) -> usize {
    match value {
        Value::Null => 4,
        Value::Bool(_) => 1,
        Value::Number(_) => 8,
        Value::String(s) => s.len(),
        Value::Array(items) => items.iter().fold(8usize, |acc, item| {
            acc.saturating_add(estimate_json_bytes(item))
        }),
        Value::Object(map) => map.iter().fold(16usize, |acc, (k, v)| {
            acc.saturating_add(k.len())
                .saturating_add(estimate_json_bytes(v))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::IndicatorInstance;
    use crate::core::indicators::render::types::{LayerBand, ObjectMutation};
    use serde_json::json;

    #[test]
    fn applies_create_update_delete_mutations() {
        let mut instance = IndicatorInstance::new(1, 1, serde_json::Value::Null);
        let mutations = vec![
            ObjectMutation::Create {
                id: 10,
                object_type: "box".to_string(),
                layer_band: LayerBand::IndicatorObjects,
                z: 0,
                props: json!({"x1": 1u64, "y1": 10.0}),
            },
            ObjectMutation::Update {
                id: 10,
                props: json!({"y1": 12.5, "x2": 3u64}),
            },
        ];

        instance.apply_object_mutations(&mutations);
        assert_eq!(instance.object_registry.len(), 1);
        let state = instance.object_registry.get(&10).expect("object exists");
        assert_eq!(state.object_type, "box");
        assert_eq!(state.mutable_props["x1"], json!(1u64));
        assert_eq!(state.mutable_props["x2"], json!(3u64));
        assert_eq!(state.mutable_props["y1"], json!(12.5));

        instance.apply_object_mutations(&[ObjectMutation::Delete { id: 10 }]);
        assert!(instance.object_registry.is_empty());
    }

    #[test]
    fn estimates_memory_for_instance_state() {
        let mut instance = IndicatorInstance::new(1, 1, json!({"len": 14}));
        instance.apply_object_mutations(&[ObjectMutation::Create {
            id: 1,
            object_type: "label".to_string(),
            layer_band: LayerBand::IndicatorObjects,
            z: 0,
            props: json!({"timestamp": 1u64, "value": 10.0, "text": "x"}),
        }]);
        assert!(instance.estimate_memory_bytes() > 0);
    }
}
