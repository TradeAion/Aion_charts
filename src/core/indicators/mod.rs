pub mod compiler;
pub mod language;
pub mod render;
pub mod runtime;

use crate::core::data::BarArray;
use crate::core::indicators::compiler::compile_source;
use crate::core::indicators::compiler::diagnostics::{CompileDiagnostic, DiagnosticSeverity};
use crate::core::indicators::language::normalize_source;
use crate::core::indicators::render::types::{
    DrawInstruction, LayerBand, ObjectMutation, RenderOrderKey,
};
use crate::core::indicators::runtime::events::RuntimeEvent;
use crate::core::indicators::runtime::instance::IndicatorInstance;
use crate::core::indicators::runtime::limits::ResourceLimits;
use crate::core::indicators::runtime::mtf::{MtfResolvedSample, MtfResolver, NoopMtfResolver};
use crate::core::indicators::runtime::scheduler::Scheduler;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

pub type IndicatorProgramId = u32;
pub type IndicatorInstanceId = u32;

pub const INDICATOR_IR_VERSION: u32 = 1;
pub const INDICATOR_STDLIB_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstantValue {
    Na,
    Bool(bool),
    Int(i64),
    Float(f64),
    Color(String),
    String(String),
    Timeframe(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpCode {
    Nop,
    LoadSeries,
    StoreSeries,
    Add,
    Sub,
    Mul,
    Div,
    BranchIfTrue,
    BranchIfFalse,
    CallBuiltin,
    RequestSeries,
    EmitPlotLine,
    EmitPlotArea,
    EmitFillBetween,
    EmitPlotCandle,
    EmitPlotBar,
    EmitPlotShape,
    EmitDrawLabel,
    EmitDrawBox,
    EmitDrawPolyline,
    Halt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IrBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IrSeriesField {
    Open,
    High,
    Low,
    Close,
    Volume,
    Time,
    BarIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IrExpr {
    Bool(bool),
    Number(f64),
    Na,
    Var(String),
    UnaryNot(Box<IrExpr>),
    UnaryNeg(Box<IrExpr>),
    Binary {
        lhs: Box<IrExpr>,
        op: IrBinaryOp,
        rhs: Box<IrExpr>,
    },
    ReqSeries {
        symbol: String,
        timeframe: String,
        field: String,
        mode: String,
        index: Option<Box<IrExpr>>,
    },
    Series {
        field: IrSeriesField,
        index: Option<Box<IrExpr>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IrCallArg {
    Expr(IrExpr),
    Text(String),
    NamedExpr { name: String, value: IrExpr },
    NamedText { name: String, value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IrCallKind {
    StateVarDecl,
    StateLetDecl,
    StateAssign,
    PlotLine,
    PlotArea,
    PlotHistogram,
    PlotBar,
    PlotCandle,
    PlotShape,
    FillBetween,
    ObjBoxNew,
    ObjBoxSet,
    ObjBoxDelete,
    ObjLabelNew,
    ObjLabelSet,
    ObjLabelDelete,
    ObjPolylineNew,
    ObjPolylineSet,
    ObjPolylineDelete,
    ObjDelete,
    RequestSeries,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrCall {
    pub kind: IrCallKind,
    pub args: Vec<IrCallArg>,
    pub guard: Option<IrExpr>,
    pub declaration_order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSchemaField {
    pub name: String,
    pub type_name: String,
    pub default_value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSchemaField {
    pub name: String,
    pub output_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDecl {
    pub max_objects: usize,
    pub max_vertices_per_frame: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorProgram {
    pub program_id: IndicatorProgramId,
    pub name: String,
    pub ir_version: u32,
    pub stdlib_version: u32,
    pub source_hash: String,
    pub feature_flags: Vec<String>,
    pub constants: Vec<ConstantValue>,
    pub opcodes: Vec<OpCode>,
    pub ir_calls: Vec<IrCall>,
    pub input_schema: Vec<InputSchemaField>,
    pub output_schema: Vec<OutputSchemaField>,
    pub resource_decl: ResourceDecl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorFrameInput {
    pub bar_index: usize,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorFrameOutput {
    pub bar_index: usize,
    pub timestamp: u64,
    pub instructions: Vec<DrawInstruction>,
    pub object_mutations: Vec<ObjectMutation>,
    pub mtf_samples: Vec<MtfResolvedSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectState {
    pub object_type: String,
    pub layer_band: LayerBand,
    pub z: i16,
    pub anchors: serde_json::Value,
    pub style: serde_json::Value,
    pub lifetime: String,
    pub mutable_props: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorCompileResult {
    pub indicator_id: Option<IndicatorProgramId>,
    pub diagnostics: Vec<CompileDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorInstanceSummary {
    pub instance_id: IndicatorInstanceId,
    pub program_id: IndicatorProgramId,
    pub enabled: bool,
    pub updates_applied: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorInstanceStats {
    pub instance_id: IndicatorInstanceId,
    pub program_id: IndicatorProgramId,
    pub ops_used: u64,
    pub last_elapsed_micros: u64,
    pub peak_objects: usize,
    pub peak_vertices: usize,
    pub updates_applied: u64,
    pub recent_events: Vec<RuntimeEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorRuntimeMessage {
    pub instance_id: IndicatorInstanceId,
    pub program_id: IndicatorProgramId,
    pub event: RuntimeEvent,
}

pub struct IndicatorManager {
    enabled: bool,
    next_program_id: IndicatorProgramId,
    next_instance_id: IndicatorInstanceId,
    programs: HashMap<IndicatorProgramId, IndicatorProgram>,
    source_cache: HashMap<String, IndicatorProgramId>,
    program_diagnostics: HashMap<IndicatorProgramId, Vec<CompileDiagnostic>>,
    instances: HashMap<IndicatorInstanceId, IndicatorInstance>,
    pending_runtime_events: Vec<IndicatorRuntimeMessage>,
    scheduler: Scheduler,
    mtf_resolver: Arc<dyn MtfResolver>,
}

impl Default for IndicatorManager {
    fn default() -> Self {
        Self::new(cfg!(feature = "indicator_runtime_v1"))
    }
}

impl IndicatorManager {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            next_program_id: 1,
            next_instance_id: 1,
            programs: HashMap::new(),
            source_cache: HashMap::new(),
            program_diagnostics: HashMap::new(),
            instances: HashMap::new(),
            pending_runtime_events: Vec::new(),
            scheduler: Scheduler,
            mtf_resolver: Arc::new(NoopMtfResolver),
        }
    }

    pub fn set_mtf_resolver(&mut self, resolver: Arc<dyn MtfResolver>) {
        self.mtf_resolver = resolver;
    }

    pub fn compile(&mut self, source: &str, feature_flags: &[String]) -> IndicatorCompileResult {
        if !self.enabled {
            return IndicatorCompileResult {
                indicator_id: None,
                diagnostics: vec![CompileDiagnostic {
                    code: "INDL-2001".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "indicator runtime feature flag is disabled".to_string(),
                    hint: Some("enable feature `indicator_runtime_v1`".to_string()),
                    span: None,
                }],
            };
        }

        let compile_output = compile_source(
            source,
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            feature_flags,
        );
        let normalized_source = normalize_source(source);
        let compile_cache_key = build_compile_cache_key(
            &normalized_source,
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            feature_flags,
        );

        if let Some(cached_id) = self.source_cache.get(&compile_cache_key).copied() {
            return IndicatorCompileResult {
                indicator_id: Some(cached_id),
                diagnostics: compile_output.diagnostics,
            };
        }

        let Some(mut program) = compile_output.program else {
            return IndicatorCompileResult {
                indicator_id: None,
                diagnostics: compile_output.diagnostics,
            };
        };

        let program_id = self.next_program_id;
        self.next_program_id = self.next_program_id.saturating_add(1);
        program.program_id = program_id;
        program.source_hash = compile_output.source_hash;

        self.programs.insert(program_id, program);
        self.program_diagnostics
            .insert(program_id, compile_output.diagnostics.clone());
        self.source_cache.insert(compile_cache_key, program_id);

        IndicatorCompileResult {
            indicator_id: Some(program_id),
            diagnostics: compile_output.diagnostics,
        }
    }

    pub fn attach(
        &mut self,
        program_id: IndicatorProgramId,
        inputs: serde_json::Value,
    ) -> Option<IndicatorInstanceId> {
        if !self.programs.contains_key(&program_id) {
            return None;
        }
        let instance_id = self.next_instance_id;
        self.next_instance_id = self.next_instance_id.saturating_add(1);
        let instance = IndicatorInstance::new(instance_id, program_id, inputs);
        self.instances.insert(instance_id, instance);
        Some(instance_id)
    }

    pub fn detach(&mut self, instance_id: IndicatorInstanceId) -> bool {
        self.instances.remove(&instance_id).is_some()
    }

    pub fn set_inputs(
        &mut self,
        instance_id: IndicatorInstanceId,
        inputs: serde_json::Value,
    ) -> bool {
        if let Some(instance) = self.instances.get_mut(&instance_id) {
            instance.inputs = inputs;
            return true;
        }
        false
    }

    pub fn set_enabled(&mut self, instance_id: IndicatorInstanceId, enabled: bool) -> bool {
        if let Some(instance) = self.instances.get_mut(&instance_id) {
            instance.enabled = enabled;
            return true;
        }
        false
    }

    pub fn set_resource_limits(
        &mut self,
        instance_id: IndicatorInstanceId,
        limits: ResourceLimits,
    ) -> bool {
        if let Some(instance) = self.instances.get_mut(&instance_id) {
            instance.limits = limits;
            return true;
        }
        false
    }

    pub fn on_set_data(&mut self, bars: &BarArray) {
        self.run_instances(bars, true);
    }

    pub fn on_incremental_update(&mut self, bars: &BarArray) {
        self.run_instances(bars, false);
    }

    fn run_instances(&mut self, bars: &BarArray, historical: bool) {
        let instance_ids: Vec<IndicatorInstanceId> = self.instances.keys().copied().collect();
        for instance_id in instance_ids {
            let (program_id, enabled) = match self.instances.get(&instance_id) {
                Some(instance) => (instance.program_id, instance.enabled),
                None => continue,
            };
            if !enabled {
                continue;
            }
            let Some(program) = self.programs.get(&program_id).cloned() else {
                continue;
            };
            let instance = match self.instances.get_mut(&instance_id) {
                Some(instance) => instance,
                None => continue,
            };
            let result = if historical {
                self.scheduler
                    .run_historical(&program, instance, bars, self.mtf_resolver.as_ref())
            } else {
                self.scheduler
                    .run_incremental(&program, instance, bars, self.mtf_resolver.as_ref())
            };
            let runtime_event = match result {
                Err(event) => {
                    instance.push_event(event.clone());
                    Some(event)
                }
                Ok(()) => None,
            };
            if let Some(event) = runtime_event {
                self.pending_runtime_events.push(IndicatorRuntimeMessage {
                    instance_id,
                    program_id,
                    event,
                });
            }
        }
    }

    pub fn drain_runtime_events(&mut self) -> Vec<IndicatorRuntimeMessage> {
        std::mem::take(&mut self.pending_runtime_events)
    }

    pub fn list_instances(&self) -> Vec<IndicatorInstanceSummary> {
        self.instances
            .values()
            .map(|instance| IndicatorInstanceSummary {
                instance_id: instance.instance_id,
                program_id: instance.program_id,
                enabled: instance.enabled,
                updates_applied: instance.updates_applied,
            })
            .collect()
    }

    pub fn get_program_diagnostics(
        &self,
        program_id: IndicatorProgramId,
    ) -> Vec<CompileDiagnostic> {
        self.program_diagnostics
            .get(&program_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_instance_stats(
        &self,
        instance_id: IndicatorInstanceId,
    ) -> Option<IndicatorInstanceStats> {
        self.instances
            .get(&instance_id)
            .map(|instance| IndicatorInstanceStats {
                instance_id: instance.instance_id,
                program_id: instance.program_id,
                ops_used: instance.counters.ops_used,
                last_elapsed_micros: instance.counters.last_elapsed_micros,
                peak_objects: instance.counters.peak_objects,
                peak_vertices: instance.counters.peak_vertices,
                updates_applied: instance.updates_applied,
                recent_events: instance.recent_events.clone(),
            })
    }

    pub fn collect_draw_instructions(
        &self,
    ) -> std::collections::BTreeMap<(LayerBand, i16), Vec<DrawInstruction>> {
        let mut out: std::collections::BTreeMap<(LayerBand, i16), Vec<DrawInstruction>> =
            std::collections::BTreeMap::new();
        for instance in self.instances.values().filter(|it| it.enabled) {
            if let Some(frame) = &instance.last_good_frame {
                for instruction in &frame.instructions {
                    let order = instruction.order_key();
                    let (band, z) = (order.layer_band, order.z);
                    out.entry((band, z)).or_default().push(instruction.clone());
                }
            }
            for instruction in object_registry_draw_instructions(instance) {
                let order = instruction.order_key();
                let (band, z) = (order.layer_band, order.z);
                out.entry((band, z)).or_default().push(instruction);
            }
        }
        out
    }

    pub fn collect_sorted_draw_instructions(&self) -> Vec<DrawInstruction> {
        let mut out = Vec::new();
        for instance in self.instances.values().filter(|it| it.enabled) {
            if let Some(frame) = &instance.last_good_frame {
                out.extend(frame.instructions.iter().cloned());
            }
            out.extend(object_registry_draw_instructions(instance));
        }
        out.sort_by_key(|inst| inst.order_key());
        out
    }
}

fn object_registry_draw_instructions(instance: &IndicatorInstance) -> Vec<DrawInstruction> {
    let mut out = Vec::new();
    for (object_id, state) in &instance.object_registry {
        let order = RenderOrderKey {
            layer_band: state.layer_band,
            z: state.z,
            declaration_order: 0,
            stable_id: ((instance.instance_id as u64) << 32) | *object_id,
        };

        match state.object_type.as_str() {
            "box" => {
                let x1 = state.mutable_props.get("x1").and_then(Value::as_u64);
                let y1 = state.mutable_props.get("y1").and_then(Value::as_f64);
                let x2 = state.mutable_props.get("x2").and_then(Value::as_u64);
                let y2 = state.mutable_props.get("y2").and_then(Value::as_f64);
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (x1, y1, x2, y2) {
                    out.push(DrawInstruction::DrawBox {
                        order,
                        id: *object_id,
                        x1,
                        y1,
                        x2,
                        y2,
                        line_color: parse_color4(
                            state.mutable_props.get("line_color"),
                            [0.94, 0.72, 0.18, 1.0],
                        ),
                        fill_color: parse_color4(
                            state.mutable_props.get("fill_color"),
                            [0.94, 0.72, 0.18, 0.16],
                        ),
                    });
                }
            }
            "label" => {
                let timestamp = state.mutable_props.get("timestamp").and_then(Value::as_u64);
                let value = state.mutable_props.get("value").and_then(Value::as_f64);
                let text = state
                    .mutable_props
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if let (Some(timestamp), Some(value)) = (timestamp, value) {
                    out.push(DrawInstruction::DrawLabel {
                        order,
                        id: *object_id,
                        timestamp,
                        value,
                        text,
                        color: parse_color4(
                            state.mutable_props.get("color"),
                            [0.98, 0.98, 0.98, 1.0],
                        ),
                    });
                }
            }
            "polyline" => {
                let points = parse_points(state.mutable_props.get("points"));
                if points.len() >= 2 {
                    out.push(DrawInstruction::DrawPolyline {
                        order,
                        id: *object_id,
                        points,
                        color: parse_color4(
                            state.mutable_props.get("color"),
                            [0.14, 0.80, 0.92, 1.0],
                        ),
                        width: state
                            .mutable_props
                            .get("width")
                            .and_then(Value::as_f64)
                            .unwrap_or(2.0) as f32,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn build_compile_cache_key(
    normalized_source: &str,
    ir_version: u32,
    stdlib_version: u32,
    feature_flags: &[String],
) -> String {
    let mut feature_flags_sorted = feature_flags.to_vec();
    feature_flags_sorted.sort();

    let mut hasher = Sha256::new();
    hasher.update(normalized_source.as_bytes());
    hasher.update([0u8]);
    hasher.update(ir_version.to_le_bytes());
    hasher.update(stdlib_version.to_le_bytes());
    for flag in feature_flags_sorted {
        hasher.update([0u8]);
        hasher.update(flag.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn parse_color4(raw: Option<&Value>, fallback: [f32; 4]) -> [f32; 4] {
    let Some(value) = raw else {
        return fallback;
    };
    let Some(array) = value.as_array() else {
        return fallback;
    };
    if array.len() != 4 {
        return fallback;
    }
    let mut out = fallback;
    for (idx, item) in array.iter().enumerate().take(4) {
        if let Some(v) = item.as_f64() {
            out[idx] = v as f32;
        }
    }
    out
}

fn parse_points(raw: Option<&Value>) -> Vec<(u64, f64)> {
    let Some(points_value) = raw else {
        return Vec::new();
    };
    let Some(items) = points_value.as_array() else {
        return Vec::new();
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Some(pair) = item.as_array() else {
            continue;
        };
        if pair.len() != 2 {
            continue;
        }
        let Some(x) = pair[0].as_u64() else {
            continue;
        };
        let Some(y) = pair[1].as_f64() else {
            continue;
        };
        out.push((x, y));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::object_registry_draw_instructions;
    use crate::core::indicators::render::types::LayerBand;
    use crate::core::indicators::runtime::instance::IndicatorInstance;
    use crate::core::indicators::ObjectState;
    use serde_json::json;

    #[test]
    fn registry_box_is_converted_to_draw_instruction() {
        let mut instance = IndicatorInstance::new(7, 1, serde_json::Value::Null);
        instance.object_registry.insert(
            10,
            ObjectState {
                object_type: "box".to_string(),
                layer_band: LayerBand::IndicatorObjects,
                z: 2,
                anchors: serde_json::Value::Null,
                style: serde_json::Value::Null,
                lifetime: "persistent".to_string(),
                mutable_props: json!({
                    "x1": 100u64,
                    "y1": 12.0,
                    "x2": 200u64,
                    "y2": 10.0
                }),
            },
        );

        let instructions = object_registry_draw_instructions(&instance);
        assert_eq!(instructions.len(), 1);
        match &instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::DrawBox {
                order, id, ..
            } => {
                assert_eq!(*id, 10);
                assert_eq!(order.layer_band, LayerBand::IndicatorObjects);
                assert_eq!(order.z, 2);
            }
            other => panic!("unexpected instruction {:?}", other),
        }
    }
}
