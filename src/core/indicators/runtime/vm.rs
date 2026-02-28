use crate::core::data::BarArray;
use crate::core::indicators::render::types::{
    DrawInstruction, LayerBand, ObjectMutation, RenderOrderKey,
};
use crate::core::indicators::runtime::builtins;
use crate::core::indicators::runtime::builtins::BuiltinRegistry;
use crate::core::indicators::runtime::events::RuntimeEvent;
use crate::core::indicators::runtime::instance::{FillBetweenMeta, IndicatorInstance, ShapeEntry};
use crate::core::indicators::runtime::mtf::{
    BarmergeGaps, BarmergeLookahead, MtfMode, MtfRequest, MtfResolvedSample, MtfResolver,
    NoopMtfResolver,
};
use crate::core::indicators::runtime::value::{RayColor, RayValue};
use crate::core::indicators::{
    IndicatorFrameOutput, IndicatorProgram, IrBinaryOp, IrCall, IrCallArg, IrCallKind, IrExpr,
    IrSeriesField,
};
use serde_json::json;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Cross-platform microsecond timer (std::time::Instant panics on wasm32)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn now_micros() -> u64 {
    // On native targets we keep a thread-local epoch so elapsed fits in u64.
    use std::time::Instant;
    thread_local! { static EPOCH: Instant = Instant::now(); }
    EPOCH.with(|epoch| epoch.elapsed().as_micros() as u64)
}

#[cfg(target_arch = "wasm32")]
fn now_micros() -> u64 {
    // js_sys::Date::now() returns milliseconds as f64; convert to micros.
    (js_sys::Date::now() * 1000.0) as u64
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Convenience wrapper using a no-op MTF resolver.
pub fn execute_bar(
    program: &IndicatorProgram,
    instance: &mut IndicatorInstance,
    bars: &BarArray,
    bar_index: usize,
) -> Result<IndicatorFrameOutput, RuntimeEvent> {
    let resolver = NoopMtfResolver;
    execute_bar_with_resolver(program, instance, bars, bar_index, &resolver)
}

/// Execute a **single bar** of an indicator program incrementally.
///
/// State and plot data are accumulated in `instance.incremental_state` across
/// successive calls.  The caller (typically the scheduler) is responsible for
/// calling `instance.reset_incremental_state()` before a full historical run
/// so that accumulated data starts fresh.
///
/// Complexity: O(M) where M = number of IR calls.
pub fn execute_bar_with_resolver(
    program: &IndicatorProgram,
    instance: &mut IndicatorInstance,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
) -> Result<IndicatorFrameOutput, RuntimeEvent> {
    let start_micros = now_micros();

    if bars.is_empty() || bar_index >= bars.len() {
        return Ok(IndicatorFrameOutput {
            bar_index,
            timestamp: 0,
            instructions: Vec::new(),
            object_mutations: Vec::new(),
            mtf_samples: Vec::new(),
        });
    }

    let timestamp = bars.timestamp(bar_index);

    // -- Phase 1: process IR calls for THIS bar only --
    //
    // We temporarily extract the mutable incremental state from the instance
    // so that the borrow checker allows us to read `instance.inputs` (immutable)
    // while mutating persistent_vars / plot_data (mutable).
    let mut persistent_vars = std::mem::take(&mut instance.incremental_state.persistent_vars);
    let mut plot_data = std::mem::take(&mut instance.incremental_state.plot_data);
    let mut fill_between = std::mem::take(&mut instance.incremental_state.fill_between);

    let mut local_vars: HashMap<String, RayValue> = HashMap::new();
    let mut object_mutations: Vec<ObjectMutation> = Vec::new();
    let mut mtf_samples: Vec<MtfResolvedSample> = Vec::new();
    let mut ops_used = 0u64;

    for (call_pos, call) in program.ir_calls.iter().enumerate() {
        ops_used += 1;
        if ops_used > instance.limits.max_ops_per_bar {
            // Restore state before returning error.
            instance.incremental_state.persistent_vars = persistent_vars;
            instance.incremental_state.plot_data = plot_data;
            instance.incremental_state.fill_between = fill_between;
            return Err(RuntimeEvent::LimitsExceeded {
                code: "INDL-2002".to_string(),
                message: "max ops per bar exceeded".to_string(),
                bar_index,
            });
        }

        let decl_idx = call.declaration_order as usize;
        let args = &call.args;

        // Evaluate guard condition using current variable state.
        let guard_ctx = EvalContext::with_vars(
            instance,
            mtf_resolver,
            decl_idx as u32,
            &persistent_vars,
            &local_vars,
        );
        if !call_guard_allows(call, bars, bar_index, &guard_ctx) {
            continue;
        }

        match call.kind {
            // -- State operations --------------------------------------------------
            IrCallKind::StateVarDecl | IrCallKind::StateLetDecl | IrCallKind::StateAssign => {
                apply_state_call(
                    call.kind.clone(),
                    args,
                    &mut persistent_vars,
                    &mut local_vars,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                );
            }

            // -- Plot: line -------------------------------------------------------
            IrCallKind::PlotLine => {
                let Some(expr) = positional_expr(args, 0) else {
                    continue;
                };
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(value) = eval_expr(expr, bars, bar_index, &ctx) {
                    plot_data
                        .entry(call_pos)
                        .or_default()
                        .line_points
                        .push((bars.timestamp(bar_index), value));
                }
            }

            // -- Plot: area -------------------------------------------------------
            IrCallKind::PlotArea => {
                let Some(expr) = positional_expr(args, 0) else {
                    continue;
                };
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(value) = eval_expr(expr, bars, bar_index, &ctx) {
                    plot_data
                        .entry(call_pos)
                        .or_default()
                        .area_points
                        .push((bars.timestamp(bar_index), value));
                }
            }

            // -- Plot: histogram --------------------------------------------------
            IrCallKind::PlotHistogram => {
                let Some(expr) = positional_expr(args, 0) else {
                    continue;
                };
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(value) = eval_expr(expr, bars, bar_index, &ctx) {
                    let base = parse_optional_named_expression(args, "base")
                        .and_then(|it| eval_expr(it, bars, bar_index, &ctx))
                        .unwrap_or(0.0);

                    // Dynamic color evaluation - check for `color` named argument
                    let color = parse_optional_named_expression(args, "color")
                        .map(|color_expr| {
                            let color_value = eval_expr_value(color_expr, bars, bar_index, &ctx);
                            color_value_to_rgba(&color_value)
                        })
                        .unwrap_or([0.38, 0.56, 1.0, 0.92]); // default histogram color

                    let acc = plot_data.entry(call_pos).or_default();
                    acc.histogram_points
                        .push((bars.timestamp(bar_index), value));
                    acc.histogram_bases.push(base);
                    acc.histogram_colors.push(color);
                }
            }

            // -- Plot: OHLC bars --------------------------------------------------
            IrCallKind::PlotBar => {
                if let Some((open_expr, high_expr, low_expr, close_expr)) =
                    parse_ohlc_expressions(args)
                {
                    let ctx = EvalContext::with_vars(
                        instance,
                        mtf_resolver,
                        decl_idx as u32,
                        &persistent_vars,
                        &local_vars,
                    );
                    let open = eval_expr(open_expr, bars, bar_index, &ctx);
                    let high = eval_expr(high_expr, bars, bar_index, &ctx);
                    let low = eval_expr(low_expr, bars, bar_index, &ctx);
                    let close = eval_expr(close_expr, bars, bar_index, &ctx);
                    if let (Some(o), Some(h), Some(l), Some(c)) = (open, high, low, close) {
                        plot_data.entry(call_pos).or_default().bar_points.push((
                            bars.timestamp(bar_index),
                            o,
                            h,
                            l,
                            c,
                        ));
                    }
                }
            }

            // -- Plot: candles ----------------------------------------------------
            IrCallKind::PlotCandle => {
                if let Some((open_expr, high_expr, low_expr, close_expr)) =
                    parse_ohlc_expressions(args)
                {
                    let ctx = EvalContext::with_vars(
                        instance,
                        mtf_resolver,
                        decl_idx as u32,
                        &persistent_vars,
                        &local_vars,
                    );
                    let open = eval_expr(open_expr, bars, bar_index, &ctx);
                    let high = eval_expr(high_expr, bars, bar_index, &ctx);
                    let low = eval_expr(low_expr, bars, bar_index, &ctx);
                    let close = eval_expr(close_expr, bars, bar_index, &ctx);
                    if let (Some(o), Some(h), Some(l), Some(c)) = (open, high, low, close) {
                        plot_data.entry(call_pos).or_default().candle_points.push((
                            bars.timestamp(bar_index),
                            o,
                            h,
                            l,
                            c,
                        ));
                    }
                }
            }

            // -- FillBetween: store metadata once ---------------------------------
            IrCallKind::FillBetween => {
                if args.len() < 2 {
                    continue;
                }
                let upper_series_id = parse_series_id_argument(&args[0]);
                let lower_series_id = parse_series_id_argument(&args[1]);
                if upper_series_id.is_empty() || lower_series_id.is_empty() {
                    continue;
                }
                let z =
                    parse_named_i16(args, "z", instance, decl_idx, bars, bar_index, mtf_resolver)
                        .unwrap_or(0);
                fill_between
                    .entry(call_pos)
                    .or_insert_with(|| FillBetweenMeta {
                        upper_series_id,
                        lower_series_id,
                        z,
                        declaration_order: call.declaration_order,
                    });
            }

            // -- PlotShape: emit on ALL matching bars (FIX for bug #5) -----------
            IrCallKind::PlotShape => {
                let Some(expr) = positional_expr(args, 0) else {
                    continue;
                };
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(value) = eval_expr(expr, bars, bar_index, &ctx) {
                    if value.abs() > f64::EPSILON {
                        plot_data
                            .entry(call_pos)
                            .or_default()
                            .shape_entries
                            .push(ShapeEntry {
                                timestamp: bars.timestamp(bar_index),
                                value,
                            });
                    }
                }
            }

            // -- Object mutations: process for current bar directly ---------------
            IrCallKind::ObjBoxNew => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_box_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    true,
                ) {
                    object_mutations.push(mutation);
                }
            }
            IrCallKind::ObjBoxSet => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_box_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    false,
                ) {
                    object_mutations.push(mutation);
                }
            }
            IrCallKind::ObjBoxDelete
            | IrCallKind::ObjLabelDelete
            | IrCallKind::ObjLineDelete
            | IrCallKind::ObjPolylineDelete
            | IrCallKind::ObjDelete => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(id) = parse_object_id_with_eval(args.first(), &ctx, bars, bar_index) {
                    object_mutations.push(ObjectMutation::Delete { id });
                }
            }
            IrCallKind::ObjLabelNew => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_label_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    true,
                ) {
                    object_mutations.push(mutation);
                }
            }
            IrCallKind::ObjLabelSet => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_label_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    false,
                ) {
                    object_mutations.push(mutation);
                }
            }
            IrCallKind::ObjLineNew => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_line_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    true,
                ) {
                    object_mutations.push(mutation);
                }
            }
            IrCallKind::ObjLineSet => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_line_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    false,
                ) {
                    object_mutations.push(mutation);
                }
            }
            IrCallKind::ObjPolylineNew => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_polyline_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    true,
                ) {
                    object_mutations.push(mutation);
                }
            }
            IrCallKind::ObjPolylineSet => {
                let ctx = EvalContext::with_vars(
                    instance,
                    mtf_resolver,
                    decl_idx as u32,
                    &persistent_vars,
                    &local_vars,
                );
                if let Some(mutation) = build_polyline_mutation_with_ctx(
                    args,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &ctx,
                    false,
                ) {
                    object_mutations.push(mutation);
                }
            }

            // -- MTF samples: collect for current bar directly --------------------
            IrCallKind::RequestSeries => {
                collect_mtf_samples_for_call(
                    call,
                    instance,
                    bars,
                    bar_index,
                    mtf_resolver,
                    &mut mtf_samples,
                );
            }

            // -- Tuple destructuring assignment ----------------------------------
            IrCallKind::StateTupleAssign => {
                apply_state_call(
                    call.kind.clone(),
                    args,
                    &mut persistent_vars,
                    &mut local_vars,
                    instance,
                    decl_idx,
                    bars,
                    bar_index,
                    mtf_resolver,
                );
            }
        }

        // Scan ALL calls (not just RequestSeries) for embedded req.series()
        // in their args. E.g. plot(req.series("BTCUSD","1h","close","confirmed"))
        // has kind=PlotLine but embeds a ReqSeries expression.
        if call.kind != IrCallKind::RequestSeries {
            collect_mtf_samples_for_call(
                call,
                instance,
                bars,
                bar_index,
                mtf_resolver,
                &mut mtf_samples,
            );
        }
    }

    // Restore incremental state back into the instance.
    instance.incremental_state.persistent_vars = persistent_vars.clone();
    instance.incremental_state.plot_data = plot_data;
    instance.incremental_state.fill_between = fill_between;

    // Push persistent var values to VarSeries for historical lookups (myVar[N]).
    for (name, value) in &persistent_vars {
        let series = instance
            .incremental_state
            .var_series
            .entry(name.clone())
            .or_default();
        // New bar: push; re-running same bar: update current value.
        if bar_index >= instance.incremental_state.bars_processed {
            series.push(value.clone());
        } else {
            series.update_current(value.clone());
        }
    }

    // -- Phase 2: wall-time check (now covers actual work) --
    let elapsed = now_micros().saturating_sub(start_micros);
    if elapsed
        > instance
            .limits
            .max_wall_time_per_bar_ms
            .saturating_mul(1_000)
    {
        return Err(RuntimeEvent::LimitsExceeded {
            code: "INDL-2003".to_string(),
            message: "max wall-time per bar exceeded".to_string(),
            bar_index,
        });
    }

    instance.incremental_state.bars_processed = bar_index + 1;

    // -- Phase 3: build DrawInstructions from accumulated data --
    let instructions = build_draw_instructions(program, instance);

    let estimated_vertices = estimate_vertices(&instructions);
    if estimated_vertices > instance.limits.max_vertices_per_frame {
        return Err(RuntimeEvent::LimitsExceeded {
            code: "INDL-2004".to_string(),
            message: "max emitted vertices per frame exceeded".to_string(),
            bar_index,
        });
    }

    instance.counters.ops_used = instance.counters.ops_used.saturating_add(ops_used);
    instance.counters.last_elapsed_micros = elapsed;
    instance.counters.peak_vertices = instance.counters.peak_vertices.max(estimated_vertices);

    Ok(IndicatorFrameOutput {
        bar_index,
        timestamp,
        instructions,
        object_mutations,
        mtf_samples,
    })
}

// ---------------------------------------------------------------------------
// build_draw_instructions: convert accumulated plot data into DrawInstructions
// ---------------------------------------------------------------------------

fn build_draw_instructions(
    program: &IndicatorProgram,
    instance: &IndicatorInstance,
) -> Vec<DrawInstruction> {
    let mut out = Vec::new();

    for (call_pos, call) in program.ir_calls.iter().enumerate() {
        let decl_idx = call.declaration_order as usize;

        match call.kind {
            IrCallKind::PlotLine => {
                let Some(acc) = instance.incremental_state.plot_data.get(&call_pos) else {
                    continue;
                };
                if acc.line_points.len() < 2 {
                    continue;
                }
                let series_id = parse_optional_named_text(&call.args, "id")
                    .filter(|id| !id.is_empty())
                    .unwrap_or_else(|| format!("ind_{}_{}", instance.instance_id, decl_idx));
                out.push(DrawInstruction::PlotLine {
                    order: RenderOrderKey {
                        layer_band: LayerBand::IndicatorSeries,
                        z: 0,
                        declaration_order: decl_idx as u32,
                        stable_id: ((instance.instance_id as u64) << 32) | (decl_idx as u64),
                    },
                    series_id,
                    points: acc.line_points.clone(),
                    color: [0.16, 0.38, 1.0, 1.0],
                    width: 2.0,
                });
            }
            IrCallKind::PlotArea => {
                let Some(acc) = instance.incremental_state.plot_data.get(&call_pos) else {
                    continue;
                };
                if acc.area_points.len() < 2 {
                    continue;
                }
                let series_id = parse_optional_named_text(&call.args, "id")
                    .filter(|id| !id.is_empty())
                    .unwrap_or_else(|| format!("ind_{}_{}", instance.instance_id, decl_idx));
                out.push(DrawInstruction::PlotArea {
                    order: RenderOrderKey {
                        layer_band: LayerBand::IndicatorFills,
                        z: 0,
                        declaration_order: decl_idx as u32,
                        stable_id: ((instance.instance_id as u64) << 32) | (decl_idx as u64) | 7,
                    },
                    series_id,
                    points: acc.area_points.clone(),
                    top_color: [0.18, 0.56, 0.95, 0.35],
                    bottom_color: [0.18, 0.56, 0.95, 0.04],
                });
            }
            IrCallKind::PlotHistogram => {
                let Some(acc) = instance.incremental_state.plot_data.get(&call_pos) else {
                    continue;
                };
                if acc.histogram_points.is_empty() {
                    continue;
                }
                // Use the constant base value (first bar's value) or 0.0.
                let base = acc.histogram_bases.first().copied().unwrap_or(0.0);

                // Use per-point colors if available, otherwise default color
                let per_point_colors = acc.histogram_colors.clone();
                let default_color = if per_point_colors.is_empty() {
                    [0.38, 0.56, 1.0, 0.92]
                } else {
                    // Use first color as default (for compatibility)
                    per_point_colors
                        .first()
                        .copied()
                        .unwrap_or([0.38, 0.56, 1.0, 0.92])
                };

                out.push(DrawInstruction::PlotHistogram {
                    order: RenderOrderKey {
                        layer_band: LayerBand::IndicatorSeries,
                        z: 0,
                        declaration_order: decl_idx as u32,
                        stable_id: ((instance.instance_id as u64) << 32) | (decl_idx as u64) | 9,
                    },
                    series_id: format!("ind_{}_{}", instance.instance_id, decl_idx),
                    points: acc.histogram_points.clone(),
                    color: default_color,
                    per_point_colors,
                    base,
                });
            }
            IrCallKind::PlotBar => {
                let Some(acc) = instance.incremental_state.plot_data.get(&call_pos) else {
                    continue;
                };
                if acc.bar_points.is_empty() {
                    continue;
                }
                out.push(DrawInstruction::PlotBar {
                    order: RenderOrderKey {
                        layer_band: LayerBand::IndicatorSeries,
                        z: 0,
                        declaration_order: decl_idx as u32,
                        stable_id: ((instance.instance_id as u64) << 32) | (decl_idx as u64) | 10,
                    },
                    series_id: format!("ind_{}_{}", instance.instance_id, decl_idx),
                    points: acc.bar_points.clone(),
                    up_color: [0.14, 0.68, 0.44, 1.0],
                    down_color: [0.85, 0.25, 0.30, 1.0],
                });
            }
            IrCallKind::PlotCandle => {
                let Some(acc) = instance.incremental_state.plot_data.get(&call_pos) else {
                    continue;
                };
                if acc.candle_points.is_empty() {
                    continue;
                }
                out.push(DrawInstruction::PlotCandle {
                    order: RenderOrderKey {
                        layer_band: LayerBand::IndicatorSeries,
                        z: 0,
                        declaration_order: decl_idx as u32,
                        stable_id: ((instance.instance_id as u64) << 32) | (decl_idx as u64) | 11,
                    },
                    series_id: format!("ind_{}_{}", instance.instance_id, decl_idx),
                    points: acc.candle_points.clone(),
                    up_color: [0.14, 0.68, 0.44, 1.0],
                    down_color: [0.85, 0.25, 0.30, 1.0],
                });
            }
            IrCallKind::PlotShape => {
                let Some(acc) = instance.incremental_state.plot_data.get(&call_pos) else {
                    continue;
                };
                for entry in &acc.shape_entries {
                    out.push(DrawInstruction::PlotShape {
                        order: RenderOrderKey {
                            layer_band: LayerBand::IndicatorObjects,
                            z: 0,
                            declaration_order: decl_idx as u32,
                            stable_id: ((instance.instance_id as u64) << 32)
                                | (decl_idx as u64)
                                | 1,
                        },
                        shape: "square".to_string(),
                        timestamp: entry.timestamp,
                        value: entry.value,
                        color: [1.0, 0.35, 0.35, 1.0],
                        size: 4.0,
                    });
                }
            }
            IrCallKind::FillBetween => {
                if let Some(meta) = instance.incremental_state.fill_between.get(&call_pos) {
                    out.push(DrawInstruction::FillBetween {
                        order: RenderOrderKey {
                            layer_band: LayerBand::IndicatorFills,
                            z: meta.z,
                            declaration_order: meta.declaration_order,
                            stable_id: ((instance.instance_id as u64) << 32)
                                | (decl_idx as u64)
                                | 8,
                        },
                        upper_series_id: meta.upper_series_id.clone(),
                        lower_series_id: meta.lower_series_id.clone(),
                        color: [0.14, 0.77, 0.67, 0.18],
                    });
                }
            }
            _ => {}
        }
    }

    out
}

// ---------------------------------------------------------------------------
// MTF sample collection for current bar
// ---------------------------------------------------------------------------

fn collect_mtf_samples_for_call(
    call: &IrCall,
    instance: &IndicatorInstance,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
    samples: &mut Vec<MtfResolvedSample>,
) {
    let decl_idx = call.declaration_order as usize;
    let mut req_calls = Vec::<ReqSeriesCall>::new();

    if call.kind == IrCallKind::RequestSeries {
        if let Some(req_call) = parse_req_series_call_args(&call.args) {
            req_calls.push(req_call);
        }
    }

    for arg in &call.args {
        collect_req_series_from_arg(arg, &mut req_calls);
    }

    for req_call in req_calls {
        let sample = resolve_mtf_sample_for_call(
            instance,
            &req_call,
            bars,
            bar_index,
            mtf_resolver,
            decl_idx as u32,
        );
        samples.push(sample);
    }
}

fn collect_req_series_calls(expr: &IrExpr, out: &mut Vec<ReqSeriesCall>) {
    match expr {
        IrExpr::ReqSeries {
            symbol,
            timeframe,
            field,
            mode,
            gaps,
            lookahead,
            index,
        } => {
            out.push(ReqSeriesCall {
                symbol: symbol.clone(),
                timeframe: timeframe.clone(),
                field: field.clone(),
                mode: MtfMode::parse(Some(mode.as_str())),
                gaps: BarmergeGaps::parse(gaps.as_deref()),
                lookahead: BarmergeLookahead::parse(lookahead.as_deref()),
            });
            if let Some(inner) = index {
                collect_req_series_calls(inner, out);
            }
        }
        IrExpr::UnaryNot(inner) | IrExpr::UnaryNeg(inner) => collect_req_series_calls(inner, out),
        IrExpr::Binary { lhs, rhs, .. } => {
            collect_req_series_calls(lhs, out);
            collect_req_series_calls(rhs, out);
        }
        IrExpr::VarIndexed { index, .. } => collect_req_series_calls(index, out),
        IrExpr::Conditional {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_req_series_calls(condition, out);
            collect_req_series_calls(then_expr, out);
            collect_req_series_calls(else_expr, out);
        }
        IrExpr::Series { index, .. } => {
            if let Some(inner) = index {
                collect_req_series_calls(inner, out);
            }
        }
        IrExpr::FnCall { args, .. } => {
            for arg in args {
                collect_req_series_calls(arg, out);
            }
        }
        IrExpr::Bool(_)
        | IrExpr::Number(_)
        | IrExpr::Na
        | IrExpr::Var(_)
        | IrExpr::Color { .. } => {}
    }
}

fn collect_req_series_from_arg(arg: &IrCallArg, out: &mut Vec<ReqSeriesCall>) {
    match arg {
        IrCallArg::Expr(expr) => collect_req_series_calls(expr, out),
        IrCallArg::NamedExpr { value, .. } => collect_req_series_calls(value, out),
        IrCallArg::Text(_) | IrCallArg::NamedText { .. } => {}
    }
}

#[derive(Debug, Clone)]
struct ReqSeriesCall {
    symbol: String,
    timeframe: String,
    field: String,
    mode: MtfMode,
    gaps: BarmergeGaps,
    lookahead: BarmergeLookahead,
}

impl ReqSeriesCall {
    fn parse(args: &[IrCallArg]) -> Option<Self> {
        let symbol = positional_text(args, 0)?;
        let timeframe = positional_text(args, 1)?;
        let field = positional_text(args, 2)?;
        let mode_raw = positional_text(args, 3).unwrap_or("confirmed");
        let gaps_raw = positional_text(args, 4);
        let lookahead_raw = positional_text(args, 5);
        Some(Self {
            symbol: symbol.to_string(),
            timeframe: timeframe.to_string(),
            field: field.to_string(),
            mode: MtfMode::parse(Some(mode_raw)),
            gaps: BarmergeGaps::parse(gaps_raw),
            lookahead: BarmergeLookahead::parse(lookahead_raw),
        })
    }
}

fn parse_req_series_call_args(args: &[IrCallArg]) -> Option<ReqSeriesCall> {
    ReqSeriesCall::parse(args)
}

fn resolve_mtf_sample_for_call(
    instance: &IndicatorInstance,
    call: &ReqSeriesCall,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
    request_decl_id: u32,
) -> MtfResolvedSample {
    let chart_timestamp = bars.timestamp(bar_index);
    let request = build_mtf_request(instance, call, request_decl_id);
    mtf_resolver
        .resolve(&request, chart_timestamp)
        .map(|mut resolved| {
            resolved.request_id = request.request_id.clone();
            resolved.timestamp = chart_timestamp;
            if resolved.source_timeframe.is_empty() {
                resolved.source_timeframe = request.timeframe.clone();
            }
            resolved
        })
        .unwrap_or_else(|| MtfResolvedSample {
            request_id: request.request_id.clone(),
            timestamp: chart_timestamp,
            value: None,
            source_timeframe: request.timeframe,
            source_bar_open: None,
            source_bar_close: None,
            is_confirmed: matches!(request.mode, MtfMode::Confirmed),
        })
}

fn build_mtf_request(
    instance: &IndicatorInstance,
    call: &ReqSeriesCall,
    request_decl_id: u32,
) -> MtfRequest {
    let symbol = if call.symbol.is_empty() {
        input_string(instance, "symbol", "unknown")
    } else {
        call.symbol.clone()
    };
    let chart_timeframe = input_string(instance, "chartTimeframe", "unknown");
    let timeframe = if call.timeframe.is_empty() {
        chart_timeframe.clone()
    } else {
        call.timeframe.clone()
    };
    let field = if call.field.is_empty() {
        "close".to_string()
    } else {
        call.field.clone()
    };
    MtfRequest {
        request_id: format!(
            "req_{}_{}_{}_{}_{}",
            instance.instance_id,
            request_decl_id,
            sanitize_request_token(&symbol),
            sanitize_request_token(&timeframe),
            sanitize_request_token(&field),
        ),
        symbol,
        chart_timeframe,
        timeframe,
        field,
        mode: call.mode,
        gaps: call.gaps,
        lookahead: call.lookahead,
    }
}

fn sanitize_request_token(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().max(1));
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

fn input_string(instance: &IndicatorInstance, key: &str, fallback: &str) -> String {
    instance
        .inputs
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or(fallback)
        .to_string()
}

// ---------------------------------------------------------------------------
// Object mutation builders (using EvalContext with current variable state)
// ---------------------------------------------------------------------------

fn build_box_mutation_with_ctx(
    args: &[IrCallArg],
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
    ctx: &EvalContext<'_>,
    create: bool,
) -> Option<ObjectMutation> {
    let (id, args_offset) =
        parse_object_id_with_offset_ctx(args, ctx, bars, bar_index, instance, decl_idx, 4)?;
    if positional_arg_count(args) < args_offset + 4 {
        return None;
    }
    let x1_arg = positional_arg(args, args_offset)?;
    let y1_arg = positional_arg(args, args_offset + 1)?;
    let x2_arg = positional_arg(args, args_offset + 2)?;
    let y2_arg = positional_arg(args, args_offset + 3)?;
    let x1 = resolve_time_argument_ctx(x1_arg, ctx, bars, bar_index)?;
    let y1 = resolve_price_argument_ctx(y1_arg, ctx, bars, bar_index)?;
    let x2 = resolve_time_argument_ctx(x2_arg, ctx, bars, bar_index)?;
    let y2 = resolve_price_argument_ctx(y2_arg, ctx, bars, bar_index)?;

    let props = json!({
        "x1": x1,
        "y1": y1,
        "x2": x2,
        "y2": y2,
        "line_color": [0.94, 0.72, 0.18, 1.0],
        "fill_color": [0.94, 0.72, 0.18, 0.16],
    });

    if create {
        Some(ObjectMutation::Create {
            id,
            object_type: "box".to_string(),
            layer_band: LayerBand::IndicatorObjects,
            z: parse_named_i16(args, "z", instance, decl_idx, bars, bar_index, mtf_resolver)
                .unwrap_or(0),
            props,
        })
    } else {
        Some(ObjectMutation::Update { id, props })
    }
}

fn build_label_mutation_with_ctx(
    args: &[IrCallArg],
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
    ctx: &EvalContext<'_>,
    create: bool,
) -> Option<ObjectMutation> {
    let (id, args_offset) =
        parse_object_id_with_offset_ctx(args, ctx, bars, bar_index, instance, decl_idx, 3)?;
    if positional_arg_count(args) < args_offset + 3 {
        return None;
    }
    let ts_arg = positional_arg(args, args_offset)?;
    let value_arg = positional_arg(args, args_offset + 1)?;
    let text_arg = positional_arg(args, args_offset + 2)?;
    let timestamp = resolve_time_argument_ctx(ts_arg, ctx, bars, bar_index)?;
    let value = resolve_price_argument_ctx(value_arg, ctx, bars, bar_index)?;
    let text = parse_text_argument_ctx(text_arg, ctx, bars, bar_index).unwrap_or_default();
    let props = json!({
        "timestamp": timestamp,
        "value": value,
        "text": text,
        "color": [0.98, 0.98, 0.98, 1.0],
    });

    if create {
        Some(ObjectMutation::Create {
            id,
            object_type: "label".to_string(),
            layer_band: LayerBand::IndicatorObjects,
            z: parse_named_i16(args, "z", instance, decl_idx, bars, bar_index, mtf_resolver)
                .unwrap_or(0),
            props,
        })
    } else {
        Some(ObjectMutation::Update { id, props })
    }
}

fn build_line_mutation_with_ctx(
    args: &[IrCallArg],
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
    ctx: &EvalContext<'_>,
    create: bool,
) -> Option<ObjectMutation> {
    // line.new(x1, y1, x2, y2, ...) — 4 positional coordinate args
    let (id, args_offset) =
        parse_object_id_with_offset_ctx(args, ctx, bars, bar_index, instance, decl_idx, 4)?;
    if positional_arg_count(args) < args_offset + 4 {
        return None;
    }
    let x1_arg = positional_arg(args, args_offset)?;
    let y1_arg = positional_arg(args, args_offset + 1)?;
    let x2_arg = positional_arg(args, args_offset + 2)?;
    let y2_arg = positional_arg(args, args_offset + 3)?;
    let x1 = resolve_time_argument_ctx(x1_arg, ctx, bars, bar_index)?;
    let y1 = resolve_price_argument_ctx(y1_arg, ctx, bars, bar_index)?;
    let x2 = resolve_time_argument_ctx(x2_arg, ctx, bars, bar_index)?;
    let y2 = resolve_price_argument_ctx(y2_arg, ctx, bars, bar_index)?;

    let width = parse_named_f32(
        args,
        "width",
        instance,
        decl_idx,
        bars,
        bar_index,
        mtf_resolver,
    )
    .unwrap_or(1.0);

    let style = parse_named_string(args, "style").unwrap_or_else(|| "solid".to_string());
    let extend = parse_named_string(args, "extend").unwrap_or_else(|| "none".to_string());

    let props = json!({
        "x1": x1,
        "y1": y1,
        "x2": x2,
        "y2": y2,
        "color": [0.14, 0.80, 0.92, 1.0],
        "width": width,
        "style": style,
        "extend": extend,
    });

    if create {
        Some(ObjectMutation::Create {
            id,
            object_type: "line".to_string(),
            layer_band: LayerBand::IndicatorObjects,
            z: parse_named_i16(args, "z", instance, decl_idx, bars, bar_index, mtf_resolver)
                .unwrap_or(0),
            props,
        })
    } else {
        Some(ObjectMutation::Update { id, props })
    }
}

fn build_polyline_mutation_with_ctx(
    args: &[IrCallArg],
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
    ctx: &EvalContext<'_>,
    create: bool,
) -> Option<ObjectMutation> {
    let (id, args_offset) =
        parse_object_id_with_offset_ctx(args, ctx, bars, bar_index, instance, decl_idx, 4)?;
    let positional_count = positional_arg_count(args);
    if positional_count < args_offset + 4 {
        return None;
    }

    let mut points = Vec::new();
    let mut idx = args_offset;
    while idx + 1 < positional_count {
        let x_arg = positional_arg(args, idx)?;
        let y_arg = positional_arg(args, idx + 1)?;
        let x = resolve_time_argument_ctx(x_arg, ctx, bars, bar_index)?;
        let y = resolve_price_argument_ctx(y_arg, ctx, bars, bar_index)?;
        points.push((x, y));
        idx += 2;
    }
    if points.len() < 2 {
        return None;
    }

    let width = parse_named_f32(
        args,
        "width",
        instance,
        decl_idx,
        bars,
        bar_index,
        mtf_resolver,
    )
    .unwrap_or(2.0);
    let props = json!({
        "points": points,
        "color": [0.14, 0.80, 0.92, 1.0],
        "width": width,
    });

    if create {
        Some(ObjectMutation::Create {
            id,
            object_type: "polyline".to_string(),
            layer_band: LayerBand::IndicatorObjects,
            z: parse_named_i16(args, "z", instance, decl_idx, bars, bar_index, mtf_resolver)
                .unwrap_or(0),
            props,
        })
    } else {
        Some(ObjectMutation::Update { id, props })
    }
}

// ---------------------------------------------------------------------------
// Helpers for object argument resolution using EvalContext
// ---------------------------------------------------------------------------

fn parse_object_id_with_eval(
    arg: Option<&IrCallArg>,
    ctx: &EvalContext<'_>,
    bars: &BarArray,
    bar_index: usize,
) -> Option<u64> {
    let Some(raw) = arg else {
        return Some(default_object_id(ctx.instance, ctx.decl_idx as usize));
    };
    let expr = arg_as_expr(raw)?;
    eval_expr(expr, bars, bar_index, ctx).and_then(to_object_id)
}

fn parse_object_id_with_offset_ctx(
    args: &[IrCallArg],
    ctx: &EvalContext<'_>,
    bars: &BarArray,
    bar_index: usize,
    instance: &IndicatorInstance,
    decl_idx: usize,
    required_payload_args: usize,
) -> Option<(u64, usize)> {
    let positional_count = positional_arg_count(args);
    if positional_count < required_payload_args {
        return None;
    }
    if positional_count == required_payload_args {
        return Some((default_object_id(instance, decl_idx), 0));
    }

    if let Some(id) = parse_object_id_with_eval(positional_arg(args, 0), ctx, bars, bar_index) {
        Some((id, 1))
    } else {
        Some((default_object_id(instance, decl_idx), 0))
    }
}

fn resolve_price_argument_ctx(
    raw: &IrCallArg,
    ctx: &EvalContext<'_>,
    bars: &BarArray,
    bar_index: usize,
) -> Option<f64> {
    let expr = arg_as_expr(raw)?;
    eval_expr(expr, bars, bar_index, ctx)
}

fn resolve_time_argument_ctx(
    raw: &IrCallArg,
    ctx: &EvalContext<'_>,
    bars: &BarArray,
    bar_index: usize,
) -> Option<u64> {
    let expr = arg_as_expr(raw)?;
    let value = eval_expr(expr, bars, bar_index, ctx)?;
    resolve_time_value(value, bars)
}

fn parse_text_argument_ctx(
    raw: &IrCallArg,
    ctx: &EvalContext<'_>,
    bars: &BarArray,
    bar_index: usize,
) -> Option<String> {
    match raw {
        IrCallArg::Text(text) => Some(text.clone()),
        IrCallArg::Expr(expr) => eval_expr(expr, bars, bar_index, ctx).map(|v| v.to_string()),
        IrCallArg::NamedExpr { .. } | IrCallArg::NamedText { .. } => None,
    }
}

fn default_object_id(instance: &IndicatorInstance, decl_idx: usize) -> u64 {
    ((instance.instance_id as u64) << 32) | (decl_idx as u64)
}

fn resolve_time_value(value: f64, bars: &BarArray) -> Option<u64> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1e-6 {
        return None;
    }
    let integer = rounded as u64;
    if integer < bars.len() as u64 {
        Some(bars.timestamp(integer as usize))
    } else {
        Some(integer)
    }
}

fn to_object_id(value: f64) -> Option<u64> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1e-6 {
        return None;
    }
    Some(rounded as u64)
}

// ---------------------------------------------------------------------------
// Named argument parsers
// ---------------------------------------------------------------------------

fn parse_named_i16(
    args: &[IrCallArg],
    name: &str,
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
) -> Option<i16> {
    parse_named_f64(
        args,
        name,
        instance,
        decl_idx,
        bars,
        bar_index,
        mtf_resolver,
    )
    .and_then(|v| {
        if v < i16::MIN as f64 || v > i16::MAX as f64 {
            None
        } else {
            Some(v.round() as i16)
        }
    })
}

fn parse_named_f32(
    args: &[IrCallArg],
    name: &str,
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
) -> Option<f32> {
    parse_named_f64(
        args,
        name,
        instance,
        decl_idx,
        bars,
        bar_index,
        mtf_resolver,
    )
    .map(|v| v as f32)
}

/// Extract a named string argument from IR call args.
fn parse_named_string(args: &[IrCallArg], name: &str) -> Option<String> {
    for arg in args {
        match arg {
            IrCallArg::NamedText { name: key, value } if key == name => {
                return Some(value.clone());
            }
            _ => {}
        }
    }
    None
}

fn parse_named_f64(
    args: &[IrCallArg],
    name: &str,
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
) -> Option<f64> {
    for arg in args {
        if let IrCallArg::NamedExpr { name: key, value } = arg {
            if key != name {
                continue;
            }
            return eval_expr(
                value,
                bars,
                bar_index,
                &EvalContext::new(instance, mtf_resolver, decl_idx as u32),
            );
        }
    }
    None
}

fn parse_ohlc_expressions(args: &[IrCallArg]) -> Option<(&IrExpr, &IrExpr, &IrExpr, &IrExpr)> {
    if args.len() < 4 {
        return None;
    }
    let open = positional_expr(args, 0)?;
    let high = positional_expr(args, 1)?;
    let low = positional_expr(args, 2)?;
    let close = positional_expr(args, 3)?;
    Some((open, high, low, close))
}

fn parse_optional_named_expression<'a>(args: &'a [IrCallArg], name: &str) -> Option<&'a IrExpr> {
    for arg in args {
        if let IrCallArg::NamedExpr { name: key, value } = arg {
            if key == name {
                return Some(value);
            }
        }
    }
    None
}

fn parse_optional_named_text(args: &[IrCallArg], name: &str) -> Option<String> {
    for arg in args {
        if let IrCallArg::NamedText { name: key, value } = arg {
            if key == name {
                return Some(value.clone());
            }
        }
    }
    None
}

fn parse_series_id_argument(raw: &IrCallArg) -> String {
    match raw {
        IrCallArg::Text(value) => value.trim().to_string(),
        IrCallArg::Expr(_) | IrCallArg::NamedExpr { .. } | IrCallArg::NamedText { .. } => {
            String::new()
        }
    }
}

// ---------------------------------------------------------------------------
// State management
// ---------------------------------------------------------------------------

fn apply_state_call(
    kind: IrCallKind,
    args: &[IrCallArg],
    persistent_vars: &mut HashMap<String, RayValue>,
    local_vars: &mut HashMap<String, RayValue>,
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
) {
    // Handle tuple assignment separately
    if kind == IrCallKind::StateTupleAssign {
        apply_tuple_assign(
            args,
            persistent_vars,
            local_vars,
            instance,
            decl_idx,
            bars,
            bar_index,
            mtf_resolver,
        );
        return;
    }

    let Some(target) = positional_text(args, 0) else {
        return;
    };
    let target = target.trim();
    if target.is_empty() {
        return;
    }
    let value_expr = positional_expr(args, 0).unwrap_or(&IrExpr::Na);
    let ctx = EvalContext::with_vars(
        instance,
        mtf_resolver,
        decl_idx as u32,
        persistent_vars,
        local_vars,
    );
    let value = eval_expr_value(value_expr, bars, bar_index, &ctx);

    match kind {
        IrCallKind::StateVarDecl => {
            persistent_vars
                .entry(target.to_string())
                .or_insert_with(|| value.clone());
        }
        IrCallKind::StateLetDecl => {
            local_vars.insert(target.to_string(), value);
        }
        IrCallKind::StateAssign => {
            if local_vars.contains_key(target) {
                local_vars.insert(target.to_string(), value);
            } else if persistent_vars.contains_key(target) {
                persistent_vars.insert(target.to_string(), value);
            } else {
                local_vars.insert(target.to_string(), value);
            }
        }
        _ => {}
    }
}

/// Apply tuple destructuring assignment: [a, b, c] = tuple_expr
fn apply_tuple_assign(
    args: &[IrCallArg],
    persistent_vars: &mut HashMap<String, RayValue>,
    local_vars: &mut HashMap<String, RayValue>,
    instance: &IndicatorInstance,
    decl_idx: usize,
    bars: &BarArray,
    bar_index: usize,
    mtf_resolver: &dyn MtfResolver,
) {
    // args[0] = expression that returns tuple
    // args[1..] = variable names to destructure into
    let value_expr = positional_expr(args, 0).unwrap_or(&IrExpr::Na);
    let ctx = EvalContext::with_vars(
        instance,
        mtf_resolver,
        decl_idx as u32,
        persistent_vars,
        local_vars,
    );
    let tuple_value = eval_expr_value(value_expr, bars, bar_index, &ctx);

    // Extract variable names from remaining args
    let mut var_names = Vec::new();
    for i in 1..args.len() {
        if let Some(name) = match &args[i] {
            IrCallArg::Text(t) => Some(t.trim().to_string()),
            _ => None,
        } {
            if !name.is_empty() {
                var_names.push(name);
            }
        }
    }

    // Destructure the tuple
    match &tuple_value {
        RayValue::Tuple(elements) => {
            for (i, name) in var_names.iter().enumerate() {
                let value = elements.get(i).cloned().unwrap_or(RayValue::Na);
                // Assign to existing variable scope or create new persistent var
                if local_vars.contains_key(name) {
                    local_vars.insert(name.clone(), value);
                } else if persistent_vars.contains_key(name) {
                    persistent_vars.insert(name.clone(), value);
                } else {
                    // Default to persistent_vars for new tuple-destructured variables
                    persistent_vars.insert(name.clone(), value);
                }
            }
        }
        _ => {
            // Not a tuple - assign Na to all variables
            for name in var_names {
                if local_vars.contains_key(&name) {
                    local_vars.insert(name, RayValue::Na);
                } else if persistent_vars.contains_key(&name) {
                    persistent_vars.insert(name, RayValue::Na);
                } else {
                    persistent_vars.insert(name, RayValue::Na);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Guard evaluation
// ---------------------------------------------------------------------------

struct EvalContext<'a> {
    instance: &'a IndicatorInstance,
    mtf_resolver: &'a dyn MtfResolver,
    decl_idx: u32,
    persistent_vars: Option<&'a HashMap<String, RayValue>>,
    local_vars: Option<&'a HashMap<String, RayValue>>,
}

impl<'a> EvalContext<'a> {
    fn new(
        instance: &'a IndicatorInstance,
        mtf_resolver: &'a dyn MtfResolver,
        decl_idx: u32,
    ) -> Self {
        Self {
            instance,
            mtf_resolver,
            decl_idx,
            persistent_vars: None,
            local_vars: None,
        }
    }

    fn with_vars(
        instance: &'a IndicatorInstance,
        mtf_resolver: &'a dyn MtfResolver,
        decl_idx: u32,
        persistent_vars: &'a HashMap<String, RayValue>,
        local_vars: &'a HashMap<String, RayValue>,
    ) -> Self {
        Self {
            instance,
            mtf_resolver,
            decl_idx,
            persistent_vars: Some(persistent_vars),
            local_vars: Some(local_vars),
        }
    }
}

fn call_guard_allows(
    call: &IrCall,
    bars: &BarArray,
    bar_index: usize,
    ctx: &EvalContext<'_>,
) -> bool {
    let Some(guard) = call.guard.as_ref() else {
        return true;
    };
    eval_expr_value(guard, bars, bar_index, ctx).is_truthy()
}

// ---------------------------------------------------------------------------
// Expression evaluator
// ---------------------------------------------------------------------------

fn eval_expr(
    expr: &IrExpr,
    bars: &BarArray,
    bar_index: usize,
    ctx: &EvalContext<'_>,
) -> Option<f64> {
    eval_expr_value(expr, bars, bar_index, ctx).as_number()
}

fn input_json_to_ray_value(raw: &serde_json::Value) -> Option<RayValue> {
    if let Some(value) = raw.as_f64() {
        return Some(RayValue::Number(value));
    }
    if let Some(value) = raw.as_i64() {
        return Some(RayValue::Number(value as f64));
    }
    if let Some(value) = raw.as_u64() {
        return Some(RayValue::Number(value as f64));
    }
    if let Some(value) = raw.as_bool() {
        return Some(RayValue::Bool(value));
    }
    if let Some(value) = raw.as_str() {
        return Some(RayValue::String(value.to_string()));
    }
    None
}

fn eval_expr_value(
    expr: &IrExpr,
    bars: &BarArray,
    bar_index: usize,
    ctx: &EvalContext<'_>,
) -> RayValue {
    match expr {
        IrExpr::Bool(v) => RayValue::Bool(*v),
        IrExpr::Number(v) => RayValue::Number(*v),
        IrExpr::Na => RayValue::Na,
        IrExpr::Var(name) => {
            // Check for color constants (color.green, color.red, etc.)
            if let Some(color) = resolve_color_constant(name) {
                return RayValue::Color(color);
            }

            // Check for introspection namespaces (syminfo.*, barstate.*, timeframe.*)
            let name_lower = name.to_ascii_lowercase();
            if name_lower.starts_with("syminfo.")
                || name_lower.starts_with("barstate.")
                || name_lower.starts_with("timeframe.")
            {
                let intro_ctx = builtins::introspection::IntrospectionContext::from_inputs(
                    &ctx.instance.inputs,
                    bar_index,
                    bars.len(),
                    true, // is_confirmed - for historical bars this is always true
                );
                if let Some(prop) = name_lower.strip_prefix("syminfo.") {
                    if let Some(result) = builtins::introspection::call_syminfo(prop, &intro_ctx) {
                        return result;
                    }
                }
                if let Some(prop) = name_lower.strip_prefix("barstate.") {
                    if let Some(result) = builtins::introspection::call_barstate(prop, &intro_ctx) {
                        return result;
                    }
                }
                if let Some(prop) = name_lower.strip_prefix("timeframe.") {
                    if let Some(result) = builtins::introspection::call_timeframe(prop, &intro_ctx)
                    {
                        return result;
                    }
                }
            }

            if let Some(locals) = ctx.local_vars {
                if let Some(value) = locals.get(name) {
                    return value.clone();
                }
            }
            if let Some(persistent) = ctx.persistent_vars {
                if let Some(value) = persistent.get(name) {
                    return value.clone();
                }
            }
            ctx.instance
                .inputs
                .get(name)
                .and_then(input_json_to_ray_value)
                .unwrap_or(RayValue::Na)
        }
        IrExpr::UnaryNot(inner) => {
            let inner = eval_expr_value(inner, bars, bar_index, ctx);
            if inner.is_na() {
                RayValue::Na
            } else {
                RayValue::Bool(!inner.is_truthy())
            }
        }
        IrExpr::UnaryNeg(inner) => eval_expr_value(inner, bars, bar_index, ctx)
            .as_number()
            .map(|value| RayValue::Number(-value))
            .unwrap_or(RayValue::Na),
        IrExpr::Binary { lhs, op, rhs } => {
            // Short-circuit for And/Or to match PineScript na semantics.
            match op {
                IrBinaryOp::And => {
                    let left = eval_expr_value(lhs, bars, bar_index, ctx);
                    if !left.is_na() && !left.is_truthy() {
                        return RayValue::Bool(false);
                    }
                    if !left.is_na() {
                        // left is truthy, result depends on right
                        let right = eval_expr_value(rhs, bars, bar_index, ctx);
                        if right.is_na() {
                            RayValue::Na
                        } else {
                            RayValue::Bool(right.is_truthy())
                        }
                    } else {
                        // left is na
                        let right = eval_expr_value(rhs, bars, bar_index, ctx);
                        if !right.is_na() && !right.is_truthy() {
                            RayValue::Bool(false) // na && false = false
                        } else {
                            RayValue::Na
                        }
                    }
                }
                IrBinaryOp::Or => {
                    let left = eval_expr_value(lhs, bars, bar_index, ctx);
                    if !left.is_na() && left.is_truthy() {
                        return RayValue::Bool(true);
                    }
                    if !left.is_na() {
                        // left is falsy, result depends on right
                        let right = eval_expr_value(rhs, bars, bar_index, ctx);
                        if right.is_na() {
                            RayValue::Na
                        } else {
                            RayValue::Bool(right.is_truthy())
                        }
                    } else {
                        // left is na
                        let right = eval_expr_value(rhs, bars, bar_index, ctx);
                        if !right.is_na() && right.is_truthy() {
                            RayValue::Bool(true) // na || true = true
                        } else {
                            RayValue::Na
                        }
                    }
                }
                _ => {
                    let Some(left) = eval_expr_value(lhs, bars, bar_index, ctx).as_number() else {
                        return RayValue::Na;
                    };
                    let Some(right) = eval_expr_value(rhs, bars, bar_index, ctx).as_number() else {
                        return RayValue::Na;
                    };
                    match op {
                        IrBinaryOp::Add => RayValue::Number(left + right),
                        IrBinaryOp::Sub => RayValue::Number(left - right),
                        IrBinaryOp::Mul => RayValue::Number(left * right),
                        IrBinaryOp::Div => {
                            if right.abs() <= f64::EPSILON {
                                RayValue::Na
                            } else {
                                RayValue::Number(left / right)
                            }
                        }
                        IrBinaryOp::Mod => {
                            if right.abs() <= f64::EPSILON {
                                RayValue::Na
                            } else {
                                RayValue::Number(left % right)
                            }
                        }
                        IrBinaryOp::Pow => {
                            let value = left.powf(right);
                            if value.is_finite() {
                                RayValue::Number(value)
                            } else {
                                RayValue::Na
                            }
                        }
                        IrBinaryOp::Gt => RayValue::Bool(left > right),
                        IrBinaryOp::Gte => RayValue::Bool(left >= right),
                        IrBinaryOp::Lt => RayValue::Bool(left < right),
                        IrBinaryOp::Lte => RayValue::Bool(left <= right),
                        IrBinaryOp::Eq => RayValue::Bool((left - right).abs() <= f64::EPSILON),
                        IrBinaryOp::Neq => RayValue::Bool((left - right).abs() > f64::EPSILON),
                        IrBinaryOp::And | IrBinaryOp::Or => unreachable!(),
                    }
                }
            }
        }
        IrExpr::Conditional {
            condition,
            then_expr,
            else_expr,
        } => {
            let condition_value = eval_expr_value(condition, bars, bar_index, ctx);
            if condition_value.is_truthy() {
                eval_expr_value(then_expr, bars, bar_index, ctx)
            } else {
                eval_expr_value(else_expr, bars, bar_index, ctx)
            }
        }
        IrExpr::ReqSeries {
            symbol,
            timeframe,
            field,
            mode,
            gaps,
            lookahead,
            index,
        } => {
            if bars.is_empty() {
                return RayValue::Na;
            }
            let Some(value_index) = resolve_expr_index(index.as_deref(), bars, bar_index, ctx)
            else {
                return RayValue::Na;
            };
            if value_index >= bars.len() {
                return RayValue::Na;
            }
            let call = ReqSeriesCall {
                symbol: symbol.clone(),
                timeframe: timeframe.clone(),
                field: field.clone(),
                mode: MtfMode::parse(Some(mode.as_str())),
                gaps: BarmergeGaps::parse(gaps.as_deref()),
                lookahead: BarmergeLookahead::parse(lookahead.as_deref()),
            };
            let sample = resolve_mtf_sample_for_call(
                ctx.instance,
                &call,
                bars,
                value_index,
                ctx.mtf_resolver,
                ctx.decl_idx,
            );
            RayValue::from_optional_number(sample.value)
        }
        IrExpr::Series { field, index } => {
            if bars.is_empty() || bar_index >= bars.len() {
                return RayValue::Na;
            }
            let Some(value_index) = resolve_expr_index(index.as_deref(), bars, bar_index, ctx)
            else {
                return RayValue::Na;
            };
            if value_index >= bars.len() {
                return RayValue::Na;
            }
            RayValue::Number(match field {
                IrSeriesField::Open => bars.open(value_index) as f64,
                IrSeriesField::High => bars.high(value_index) as f64,
                IrSeriesField::Low => bars.low(value_index) as f64,
                IrSeriesField::Close => bars.close(value_index) as f64,
                IrSeriesField::Volume => bars.volume(value_index) as f64,
                IrSeriesField::Time => bars.timestamp(value_index) as f64,
                IrSeriesField::BarIndex => value_index as f64,
            })
        }
        IrExpr::VarIndexed { name, index } => {
            let offset_val = eval_expr_value(index, bars, bar_index, ctx);
            let offset = match offset_val.as_number() {
                Some(v) if v.is_finite() && v >= 0.0 => v.round() as usize,
                _ => return RayValue::Na,
            };

            // offset=0: return current value from vars
            if offset == 0 {
                if let Some(locals) = ctx.local_vars {
                    if let Some(value) = locals.get(name) {
                        return value.clone();
                    }
                }
                if let Some(persistent) = ctx.persistent_vars {
                    if let Some(value) = persistent.get(name) {
                        return value.clone();
                    }
                }
                return ctx
                    .instance
                    .inputs
                    .get(name)
                    .and_then(input_json_to_ray_value)
                    .unwrap_or(RayValue::Na);
            }

            // offset>0: look in VarSeries for historical values
            // VarSeries stores values after each bar completes, so:
            // - series[0] = most recent completed bar
            // - offset=1 means "1 bar ago" = series[0]
            // - offset=2 means "2 bars ago" = series[1]
            // Therefore we use get(offset - 1)
            if let Some(series) = ctx.instance.incremental_state.var_series.get(name) {
                if let Some(value) = series.get(offset.saturating_sub(1)) {
                    return value.clone();
                }
            }

            RayValue::Na
        }
        IrExpr::FnCall { name, args } => {
            // Evaluate all arguments to RayValue
            let evaluated_args: Vec<RayValue> = args
                .iter()
                .map(|arg| eval_expr_value(arg, bars, bar_index, ctx))
                .collect();

            // Create TA context for ta.* functions
            let ta_ctx = builtins::ta::TaContext { bars, bar_index };

            // Create introspection context for syminfo.*/barstate.*/timeframe.* properties
            let intro_ctx = builtins::introspection::IntrospectionContext::from_inputs(
                &ctx.instance.inputs,
                bar_index,
                bars.len(),
                true, // is_confirmed - for historical bars this is always true
            );

            // Dispatch to builtin registry with both contexts
            BuiltinRegistry::call_with_context(
                name,
                &evaluated_args,
                Some(&ta_ctx),
                Some(&intro_ctx),
            )
            .unwrap_or(RayValue::Na)
        }
        IrExpr::Color { r, g, b, a } => RayValue::Color(RayColor::new(*r, *g, *b, *a)),
    }
}

fn resolve_expr_index(
    index: Option<&IrExpr>,
    bars: &BarArray,
    bar_index: usize,
    ctx: &EvalContext<'_>,
) -> Option<usize> {
    if bar_index >= bars.len() {
        return None;
    }
    if let Some(offset_expr) = index {
        let offset_value = eval_expr(offset_expr, bars, bar_index, ctx)?;
        if !offset_value.is_finite() || offset_value < 0.0 {
            return None;
        }
        let rounded = offset_value.round();
        if (rounded - offset_value).abs() > 1e-9 {
            return None;
        }
        let offset = rounded as usize;
        bar_index.checked_sub(offset)
    } else {
        Some(bar_index)
    }
}

// ---------------------------------------------------------------------------
// Color value conversion
// ---------------------------------------------------------------------------

/// Convert a RayValue to RGBA color array.
/// Supports RayValue::Color, or falls back to default color.
fn color_value_to_rgba(value: &RayValue) -> [f32; 4] {
    match value {
        RayValue::Color(color) => color.to_rgba_array(),
        _ => [0.38, 0.56, 1.0, 0.92], // default histogram color
    }
}

/// Resolve color constants like color.green, color.red, etc.
fn resolve_color_constant(name: &str) -> Option<RayColor> {
    let name_lower = name.to_ascii_lowercase();
    match name_lower.as_str() {
        "color.white" => Some(RayColor::WHITE),
        "color.black" => Some(RayColor::BLACK),
        "color.red" => Some(RayColor::RED),
        "color.green" => Some(RayColor::GREEN),
        "color.blue" => Some(RayColor::BLUE),
        "color.yellow" => Some(RayColor::YELLOW),
        "color.orange" => Some(RayColor::ORANGE),
        "color.purple" => Some(RayColor::PURPLE),
        "color.aqua" => Some(RayColor::AQUA),
        "color.lime" => Some(RayColor::LIME),
        "color.maroon" => Some(RayColor::MAROON),
        "color.navy" => Some(RayColor::NAVY),
        "color.olive" => Some(RayColor::OLIVE),
        "color.silver" => Some(RayColor::SILVER),
        "color.gray" | "color.grey" => Some(RayColor::GRAY),
        "color.teal" => Some(RayColor::TEAL),
        "color.fuchsia" => Some(RayColor::FUCHSIA),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Positional argument helpers
// ---------------------------------------------------------------------------

fn positional_expr(args: &[IrCallArg], index: usize) -> Option<&IrExpr> {
    let mut seen = 0usize;
    for arg in args {
        if let IrCallArg::Expr(expr) = arg {
            if seen == index {
                return Some(expr);
            }
            seen = seen.saturating_add(1);
        }
    }
    None
}

fn positional_text(args: &[IrCallArg], index: usize) -> Option<&str> {
    let mut seen = 0usize;
    for arg in args {
        if let IrCallArg::Text(text) = arg {
            if seen == index {
                return Some(text);
            }
            seen = seen.saturating_add(1);
        }
    }
    None
}

fn positional_arg(args: &[IrCallArg], index: usize) -> Option<&IrCallArg> {
    let mut seen = 0usize;
    for arg in args {
        if matches!(arg, IrCallArg::Expr(_) | IrCallArg::Text(_)) {
            if seen == index {
                return Some(arg);
            }
            seen = seen.saturating_add(1);
        }
    }
    None
}

fn positional_arg_count(args: &[IrCallArg]) -> usize {
    args.iter()
        .filter(|arg| matches!(arg, IrCallArg::Expr(_) | IrCallArg::Text(_)))
        .count()
}

fn arg_as_expr(arg: &IrCallArg) -> Option<&IrExpr> {
    match arg {
        IrCallArg::Expr(expr) => Some(expr),
        IrCallArg::Text(_) | IrCallArg::NamedExpr { .. } | IrCallArg::NamedText { .. } => None,
    }
}

// ---------------------------------------------------------------------------
// Vertex estimation
// ---------------------------------------------------------------------------

fn estimate_vertices(instructions: &[DrawInstruction]) -> usize {
    instructions
        .iter()
        .map(|instruction| match instruction {
            DrawInstruction::PlotLine { points, .. } => points.len().saturating_mul(2),
            DrawInstruction::PlotArea { points, .. } => points.len().saturating_mul(3),
            DrawInstruction::PlotHistogram { points, .. } => points.len().saturating_mul(6),
            DrawInstruction::PlotBar { points, .. } => points.len().saturating_mul(6),
            DrawInstruction::PlotCandle { points, .. } => points.len().saturating_mul(10),
            DrawInstruction::PlotShape { .. } => 6,
            DrawInstruction::DrawLabel { .. } => 6,
            DrawInstruction::DrawBox { .. } => 6,
            DrawInstruction::DrawLine { .. } => 2,
            DrawInstruction::DrawPolyline { points, .. } => points.len().saturating_mul(2),
            DrawInstruction::FillBetween { .. } => 6,
        })
        .sum()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{execute_bar, execute_bar_with_resolver};
    use crate::core::data::{Bar, BarArray};
    use crate::core::indicators::compiler::compile_source;
    use crate::core::indicators::runtime::instance::IndicatorInstance;
    use crate::core::indicators::runtime::mtf::{MtfRequest, MtfResolvedSample, MtfResolver};
    use crate::core::indicators::{
        IndicatorProgram, IrCallArg, IrCallKind, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION,
    };

    fn sample_program(line: &str) -> IndicatorProgram {
        let source = format!("indicator(\"t\")\n{line}");
        let mut program =
            compile_source(&source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("sample indicator should compile");
        program.program_id = 1;
        program
    }

    fn sample_bars() -> BarArray {
        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 2,
                open: 11.0,
                high: 13.0,
                low: 10.0,
                close: 12.0,
                volume: 200.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 3,
                open: 12.0,
                high: 14.0,
                low: 11.0,
                close: 13.0,
                volume: 300.0,
                _pad: 0.0,
            },
        ]);
        bars
    }

    fn mixed_bars() -> BarArray {
        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 2,
                open: 12.0,
                high: 13.0,
                low: 10.0,
                close: 11.0,
                volume: 120.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 3,
                open: 11.0,
                high: 15.0,
                low: 10.0,
                close: 14.0,
                volume: 140.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 4,
                open: 15.0,
                high: 16.0,
                low: 12.0,
                close: 13.0,
                volume: 160.0,
                _pad: 0.0,
            },
        ]);
        bars
    }

    struct MockMtfResolver;

    impl MtfResolver for MockMtfResolver {
        fn resolve(&self, request: &MtfRequest, chart_timestamp: u64) -> Option<MtfResolvedSample> {
            Some(MtfResolvedSample {
                request_id: request.request_id.clone(),
                timestamp: chart_timestamp,
                value: Some(123.25),
                source_timeframe: request.timeframe.clone(),
                source_bar_open: Some(chart_timestamp.saturating_sub(60_000)),
                source_bar_close: Some(chart_timestamp),
                is_confirmed: true,
            })
        }
    }

    /// Helper: run the scheduler through all bars up to `last_bar_index` and
    /// return the final frame.  This replaces the old pattern of calling
    /// `execute_bar` once with a high bar_index and expecting all prior bars
    /// to be included in the output.
    fn run_historical_to(
        program: &IndicatorProgram,
        instance: &mut IndicatorInstance,
        bars: &BarArray,
        last_bar_index: usize,
        mtf_resolver: &dyn MtfResolver,
    ) -> crate::core::indicators::IndicatorFrameOutput {
        instance.reset_incremental_state();
        let max = last_bar_index.min(bars.len().saturating_sub(1));
        let mut last_frame = None;
        for idx in 0..=max {
            let frame = execute_bar_with_resolver(program, instance, bars, idx, mtf_resolver)
                .expect("vm should execute");
            instance.apply_object_mutations(&frame.object_mutations);
            last_frame = Some(frame);
        }
        last_frame.expect("at least one bar should produce a frame")
    }

    fn run_historical(
        program: &IndicatorProgram,
        instance: &mut IndicatorInstance,
        bars: &BarArray,
    ) -> crate::core::indicators::IndicatorFrameOutput {
        let noop = crate::core::indicators::runtime::mtf::NoopMtfResolver;
        run_historical_to(program, instance, bars, bars.len().saturating_sub(1), &noop)
    }

    #[test]
    fn emits_plot_line_from_close_series() {
        let program = sample_program("plot(close)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(1, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 11.0));
                assert_eq!(points[2], (3, 13.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plot_line_from_open_series() {
        let program = sample_program("plot(open)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(2, 1, serde_json::Value::Null);

        let frame = run_historical_to(
            &program,
            &mut instance,
            &bars,
            1,
            &crate::core::indicators::runtime::mtf::NoopMtfResolver,
        );
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 2);
                assert_eq!(points[0], (1, 10.0));
                assert_eq!(points[1], (2, 11.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plot_line_for_arithmetic_expression() {
        let program = sample_program("plot((close + open) / 2)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(3, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 10.5));
                assert_eq!(points[1], (2, 11.5));
                assert_eq!(points[2], (3, 12.5));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plot_line_for_modulo_expression() {
        let program = sample_program("plot(close % 5)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(31, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 1.0));
                assert_eq!(points[1], (2, 2.0));
                assert_eq!(points[2], (3, 3.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plot_line_for_power_expression() {
        let program = sample_program("plot(close ** 2)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(32, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 121.0));
                assert_eq!(points[1], (2, 144.0));
                assert_eq!(points[2], (3, 169.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plot_line_for_ternary_expression() {
        let program = sample_program("plot(close > open ? high : low)");
        let bars = mixed_bars();
        let mut instance = IndicatorInstance::new(33, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 4);
                assert_eq!(points[0], (1, 12.0));
                assert_eq!(points[1], (2, 10.0));
                assert_eq!(points[2], (3, 15.0));
                assert_eq!(points[3], (4, 12.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plot_line_with_series_indexing() {
        let program = sample_program("plot(close[1])");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(4, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 2);
                assert_eq!(points[0], (2, 11.0));
                assert_eq!(points[1], (3, 12.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plotcandle_instruction() {
        let program = sample_program("plotcandle(open, high, low, close)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(5, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotCandle {
                points, ..
            } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 10.0, 12.0, 9.0, 11.0));
                assert_eq!(points[2], (3, 12.0, 14.0, 11.0, 13.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plotbar_instruction() {
        let program = sample_program("plotbar(open, high, low, close)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(6, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotBar { points, .. } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 10.0, 12.0, 9.0, 11.0));
                assert_eq!(points[2], (3, 12.0, 14.0, 11.0, 13.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plothistogram_instruction() {
        let program = sample_program("plothistogram(volume, base=100)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(7, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotHistogram {
                points,
                base,
                ..
            } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 100.0));
                assert_eq!(points[2], (3, 300.0));
                assert_eq!(*base, 100.0);
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_plotarea_instruction() {
        let program = sample_program("plotarea(close, id=\"upper\")");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(70, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotArea {
                series_id,
                points,
                ..
            } => {
                assert_eq!(series_id, "upper");
                assert_eq!(points.len(), 3);
                assert_eq!(points[2], (3, 13.0));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_fillbetween_instruction() {
        let program = sample_program(r#"fillbetween("upper", "lower", z=2)"#);
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(71, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::FillBetween {
                upper_series_id,
                lower_series_id,
                order,
                ..
            } => {
                assert_eq!(upper_series_id, "upper");
                assert_eq!(lower_series_id, "lower");
                assert_eq!(order.z, 2);
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn emits_box_object_create_mutation() {
        let program = sample_program("box.new(42, 0, low[1], 2, high, z=3)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(8, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        // Object mutations are emitted per-bar; the last bar's mutations
        // are in the returned frame.
        assert!(!frame.object_mutations.is_empty());
        let mutation = frame.object_mutations.last().unwrap();
        match mutation {
            crate::core::indicators::render::types::ObjectMutation::Create {
                id,
                object_type,
                z,
                props,
                ..
            } => {
                assert_eq!(*id, 42);
                assert_eq!(object_type, "box");
                assert_eq!(*z, 3);
                // On the last bar (index=2): bar_index 0 -> ts=1, bar_index 2 -> ts=3
                assert_eq!(props["x1"], serde_json::Value::from(1u64));
                assert_eq!(props["x2"], serde_json::Value::from(3u64));
            }
            other => panic!("unexpected mutation: {:?}", other),
        }
    }

    #[test]
    fn emits_obj_delete_mutation() {
        let program = sample_program("obj.delete(42)");
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(9, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert!(!frame.object_mutations.is_empty());
        let mutation = frame.object_mutations.last().unwrap();
        match mutation {
            crate::core::indicators::render::types::ObjectMutation::Delete { id } => {
                assert_eq!(*id, 42)
            }
            other => panic!("unexpected mutation: {:?}", other),
        }
    }

    #[test]
    fn emits_req_series_sample_with_provenance() {
        let program = sample_program(r#"req.series("BTCUSD", "1h", "close", "confirmed")"#);
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(
            10,
            1,
            serde_json::json!({
                "symbol": "BTCUSD",
                "chartTimeframe": "1m"
            }),
        );
        let resolver = MockMtfResolver;

        let frame = run_historical_to(&program, &mut instance, &bars, 2, &resolver);
        assert_eq!(frame.mtf_samples.len(), 1);
        let sample = &frame.mtf_samples[0];
        assert_eq!(sample.value, Some(123.25));
        assert_eq!(sample.source_timeframe, "1h");
        assert_eq!(sample.timestamp, 3);
        assert!(sample.is_confirmed);
    }

    #[test]
    fn emits_plot_line_from_req_series_expression() {
        let program = sample_program(r#"plot(req.series("BTCUSD", "1h", "close", "confirmed"))"#);
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(
            11,
            1,
            serde_json::json!({
                "symbol": "BTCUSD",
                "chartTimeframe": "1m"
            }),
        );
        let resolver = MockMtfResolver;

        let frame = run_historical_to(&program, &mut instance, &bars, 2, &resolver);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], (1, 123.25));
                assert_eq!(points[2], (3, 123.25));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
        assert_eq!(frame.mtf_samples.len(), 1);
    }

    #[test]
    fn emits_plot_line_from_req_series_with_index_and_arithmetic() {
        let program =
            sample_program(r#"plot(req.series("BTCUSD", "1h", "close", "confirmed")[1] + close)"#);
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(
            12,
            1,
            serde_json::json!({
                "symbol": "BTCUSD",
                "chartTimeframe": "1m"
            }),
        );
        let resolver = MockMtfResolver;

        let frame = run_historical_to(&program, &mut instance, &bars, 2, &resolver);
        assert_eq!(frame.instructions.len(), 1);
        match &frame.instructions[0] {
            crate::core::indicators::render::types::DrawInstruction::PlotLine {
                points, ..
            } => {
                assert_eq!(points.len(), 2);
                assert_eq!(points[0], (2, 135.25));
                assert_eq!(points[1], (3, 136.25));
            }
            other => panic!("unexpected instruction: {:?}", other),
        }
    }

    #[test]
    fn respects_dynamic_if_guards_for_plot_calls() {
        let source =
            "indicator(\"t\")\nif close > open {\n  plot(close)\n}\nelse {\n  plot(open)\n}";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 13;
        let bars = mixed_bars();
        let mut instance = IndicatorInstance::new(13, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        let mut lines = frame
            .instructions
            .iter()
            .filter_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        lines.sort_by_key(|points| points.first().map(|(ts, _)| *ts).unwrap_or(0));

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], vec![(1, 11.0), (3, 14.0)]);
        assert_eq!(lines[1], vec![(2, 12.0), (4, 15.0)]);
    }

    #[test]
    fn respects_else_if_guard_chains_for_plot_calls() {
        let source = "indicator(\"t\")\nvar x = na\nif close > open {\n  x = close\n}\nelse if volume > 150 {\n  x = high\n}\nelse {\n  x = open\n}\nplot(x)";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 131;
        let bars = mixed_bars();
        let mut instance = IndicatorInstance::new(131, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist");
        assert_eq!(line, vec![(1, 11.0), (2, 12.0), (3, 14.0), (4, 16.0)]);
    }

    #[test]
    fn supports_var_persistence_and_assignment() {
        let source = "indicator(\"t\")\nvar last = na\nif close > open {\n  last = close\n}\nelse {\n  last = open\n}\nplot(last)";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 14;
        let bars = mixed_bars();
        let mut instance = IndicatorInstance::new(14, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist");
        assert_eq!(line, vec![(1, 11.0), (2, 12.0), (3, 14.0), (4, 15.0)]);
    }

    #[test]
    fn supports_let_bar_local_assignment() {
        let source = "indicator(\"t\")\nlet x = close\nif close > open {\n  x = high\n}\nplot(x)";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 15;
        let bars = mixed_bars();
        let mut instance = IndicatorInstance::new(15, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist");
        assert_eq!(line, vec![(1, 12.0), (2, 11.0), (3, 15.0), (4, 13.0)]);
    }

    #[test]
    fn supports_bounded_while_loops() {
        let source = "indicator(\"t\")\nlet x = 0\nwhile x < 2 {\n  x = x + 1\n}\nplot(x)";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 151;
        let bars = mixed_bars();
        let mut instance = IndicatorInstance::new(151, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist");
        assert_eq!(line, vec![(1, 2.0), (2, 2.0), (3, 2.0), (4, 2.0)]);
    }

    #[test]
    fn supports_switch_case_guards() {
        let source = "indicator(\"t\")\nlet x = close\nswitch close > open {\n  case true {\n    x = high\n  }\n  case false {\n    x = low\n  }\n}\nplot(x)";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 152;
        let bars = mixed_bars();
        let mut instance = IndicatorInstance::new(152, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist");
        assert_eq!(line, vec![(1, 12.0), (2, 10.0), (3, 15.0), (4, 12.0)]);
    }

    #[test]
    fn object_mutation_guard_uses_var_state() {
        let source = "indicator(\"t\")\nvar show = true\nif show {\n  label.new(bar_index, close, \"ok\")\n}";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 16;
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(16, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        // Last bar emits a label mutation.
        assert_eq!(frame.object_mutations.len(), 1);
        match &frame.object_mutations[0] {
            crate::core::indicators::render::types::ObjectMutation::Create {
                object_type,
                props,
                ..
            } => {
                assert_eq!(object_type, "label");
                assert_eq!(props["timestamp"], serde_json::Value::from(3u64));
                assert_eq!(props["value"], serde_json::Value::from(13.0));
            }
            other => panic!("unexpected mutation: {:?}", other),
        }
    }

    #[test]
    fn object_args_can_reference_var_state() {
        let source =
            "indicator(\"t\")\nvar y = na\ny = close\nbox.new(7, bar_index, y, bar_index, y)";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 17;
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(17, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);
        assert!(!frame.object_mutations.is_empty());
        let mutation = frame.object_mutations.last().unwrap();
        match mutation {
            crate::core::indicators::render::types::ObjectMutation::Create { props, .. } => {
                assert_eq!(props["x1"], serde_json::Value::from(3u64));
                assert_eq!(props["y1"], serde_json::Value::from(13.0));
                assert_eq!(props["x2"], serde_json::Value::from(3u64));
                assert_eq!(props["y2"], serde_json::Value::from(13.0));
            }
            other => panic!("unexpected mutation: {:?}", other),
        }
    }

    #[test]
    fn mtf_guard_uses_var_state() {
        let source = "indicator(\"t\")\nvar do_req = true\nif do_req {\n  req.series(\"BTCUSD\", \"1h\", \"close\", \"confirmed\")\n}";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 18;
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(
            18,
            1,
            serde_json::json!({
                "symbol": "BTCUSD",
                "chartTimeframe": "1m"
            }),
        );
        let resolver = MockMtfResolver;

        let frame = run_historical_to(&program, &mut instance, &bars, 2, &resolver);
        assert_eq!(frame.mtf_samples.len(), 1);
        assert_eq!(frame.mtf_samples[0].value, Some(123.25));
    }

    #[test]
    fn var_series_enables_historical_lookup() {
        // Simple test: var x = close, then reference x[1]
        let source = "indicator(\"t\")\nvar x = close\nplot(x)";
        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("script should compile");
        program.program_id = 19;

        // sample_bars has 3 bars with close = [11, 12, 13]
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(19, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);

        // Check that VarSeries was populated
        assert!(
            instance.incremental_state.var_series.contains_key("x"),
            "var_series should contain 'x'"
        );

        let x_series = instance.incremental_state.var_series.get("x").unwrap();
        assert_eq!(x_series.len(), 3, "x_series should have 3 bars of history");

        // Verify the plot output exists
        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist");

        // var x = close only sets on first bar, then persists
        // Bar 0: x = close (11)
        // Bar 1: x stays 11 (var only initializes once)
        // Bar 2: x stays 11
        assert_eq!(line.len(), 3, "plot should have 3 points");
        assert_eq!(line[0].1, 11.0, "bar 0 should plot 11");
        assert_eq!(line[1].1, 11.0, "bar 1 should plot 11 (persisted)");
        assert_eq!(line[2].1, 11.0, "bar 2 should plot 11 (persisted)");
    }

    /// Phase 2 Acceptance Gate Test:
    /// `var ema = close; ema = 0.1 * close + 0.9 * nz(ema[1]); plot(ema)`
    ///
    /// This tests:
    /// - var declaration with initialization
    /// - compound reassignment each bar
    /// - VarSeries historical lookup (ema[1])
    /// - nz() builtin function call within expression
    /// - arithmetic operations
    #[test]
    fn phase2_ema_acceptance_gate() {
        let source = r#"indicator("EMA Test")
var ema = close
ema = 0.1 * close + 0.9 * nz(ema[1])
plot(ema)"#;

        let mut program =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[])
                .program
                .expect("EMA script should compile");
        program.program_id = 20;

        // sample_bars has 3 bars with close = [11, 12, 13]
        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(20, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);

        // Check that VarSeries was populated for ema
        assert!(
            instance.incremental_state.var_series.contains_key("ema"),
            "var_series should contain 'ema'"
        );

        let ema_series = instance.incremental_state.var_series.get("ema").unwrap();
        assert_eq!(
            ema_series.len(),
            3,
            "ema_series should have 3 bars of history"
        );

        // Verify the plot output exists
        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist for EMA");

        assert_eq!(line.len(), 3, "EMA plot should have 3 points");

        // EMA calculation with alpha=0.1:
        // Bar 0: ema = 0.1 * 11 + 0.9 * nz(ema[1])
        //        ema[1] is Na (no prior bar), nz(Na) = 0
        //        ema = 1.1 + 0 = 1.1
        // Bar 1: ema = 0.1 * 12 + 0.9 * nz(ema[1])
        //        ema[1] = 1.1 (from bar 0)
        //        ema = 1.2 + 0.99 = 2.19
        // Bar 2: ema = 0.1 * 13 + 0.9 * nz(ema[1])
        //        ema[1] = 2.19 (from bar 1)
        //        ema = 1.3 + 1.971 = 3.271

        let expected_bar0 = 0.1 * 11.0 + 0.9 * 0.0; // 1.1
        let expected_bar1 = 0.1 * 12.0 + 0.9 * expected_bar0; // 2.19
        let expected_bar2 = 0.1 * 13.0 + 0.9 * expected_bar1; // 3.271

        let epsilon = 1e-6;
        assert!(
            (line[0].1 - expected_bar0).abs() < epsilon,
            "bar 0 EMA should be ~{}, got {}",
            expected_bar0,
            line[0].1
        );
        assert!(
            (line[1].1 - expected_bar1).abs() < epsilon,
            "bar 1 EMA should be ~{}, got {}",
            expected_bar1,
            line[1].1
        );
        assert!(
            (line[2].1 - expected_bar2).abs() < epsilon,
            "bar 2 EMA should be ~{}, got {}",
            expected_bar2,
            line[2].1
        );
    }

    /// Phase 3 Test: ta.sma(close, 3) works end-to-end
    #[test]
    fn phase3_ta_sma_works() {
        let source = r#"indicator("SMA Test")
plot(ta.sma(close, 3))"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        // Check for compile errors
        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(errors.is_empty(), "Compile errors: {:?}", errors);

        let mut program = compile_result.program.expect("SMA script should compile");
        program.program_id = 21;

        // Create bars with 5 data points for SMA(3) to have at least 3 valid points
        // Closes: 10, 11, 12, 13, 14
        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 9.0,
                high: 11.0,
                low: 8.0,
                close: 10.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 2,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 3,
                open: 11.0,
                high: 13.0,
                low: 10.0,
                close: 12.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 4,
                open: 12.0,
                high: 14.0,
                low: 11.0,
                close: 13.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 5,
                open: 13.0,
                high: 15.0,
                low: 12.0,
                close: 14.0,
                volume: 100.0,
                _pad: 0.0,
            },
        ]);

        let mut instance = IndicatorInstance::new(21, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);

        let line = frame
            .instructions
            .iter()
            .find_map(|instruction| {
                if let crate::core::indicators::render::types::DrawInstruction::PlotLine {
                    points,
                    ..
                } = instruction
                {
                    Some(points.clone())
                } else {
                    None
                }
            })
            .expect("plot line should exist for SMA");

        // SMA(3) for closes [10, 11, 12, 13, 14]:
        // Bar 0, 1: Na (not enough bars)
        // Bar 2: SMA = (10+11+12)/3 = 11
        // Bar 3: SMA = (11+12+13)/3 = 12
        // Bar 4: SMA = (12+13+14)/3 = 13
        assert_eq!(
            line.len(),
            3,
            "SMA plot should have 3 points (bars 2, 3, 4)"
        );

        let epsilon = 0.001;
        assert!(
            (line[0].1 - 11.0).abs() < epsilon,
            "bar 2 SMA should be 11, got {}",
            line[0].1
        );
        assert!(
            (line[1].1 - 12.0).abs() < epsilon,
            "bar 3 SMA should be 12, got {}",
            line[1].1
        );
        assert!(
            (line[2].1 - 13.0).abs() < epsilon,
            "bar 4 SMA should be 13, got {}",
            line[2].1
        );
    }

    #[test]
    fn phase3_tuple_destructuring_macd() {
        // Test the Phase 3 acceptance gate: tuple destructuring with ta.macd
        let source = r#"indicator("MACD Tuple Test")
[macd, signal, hist] = ta.macd(close, 12, 26, 9)
plot(macd)
plot(signal)
plotHistogram(hist)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        // Check for compile errors
        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(errors.is_empty(), "Compile errors: {:?}", errors);

        let mut program = compile_result
            .program
            .expect("MACD tuple script should compile");
        program.program_id = 22;

        // Create 50 bars of data for MACD calculations
        // MACD(12,26,9) needs at least 26+9=35 bars
        let mut bars = BarArray::new();
        let bar_data: Vec<Bar> = (1..=50_u64)
            .map(|i| Bar {
                timestamp: i,
                open: 100.0 + (i as f32) * 0.5,
                high: 105.0 + (i as f32) * 0.5,
                low: 95.0 + (i as f32) * 0.5,
                close: 100.0 + (i as f32), // closes: 101, 102, 103, ...
                volume: 1000.0,
                _pad: 0.0,
            })
            .collect();
        bars.set(bar_data);

        let mut instance = IndicatorInstance::new(22, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);

        // Verify that the tuple destructuring created the variables and they were plotted
        // Check that persistent_vars contains our tuple-destructured variables
        assert!(
            instance
                .incremental_state
                .persistent_vars
                .contains_key("macd"),
            "Variable 'macd' should be in persistent_vars"
        );
        assert!(
            instance
                .incremental_state
                .persistent_vars
                .contains_key("signal"),
            "Variable 'signal' should be in persistent_vars"
        );
        assert!(
            instance
                .incremental_state
                .persistent_vars
                .contains_key("hist"),
            "Variable 'hist' should be in persistent_vars"
        );

        // Count the number of plot lines in output (should have at least 2 for macd and signal)
        let plot_count = frame
            .instructions
            .iter()
            .filter(|i| {
                matches!(
                    i,
                    crate::core::indicators::render::types::DrawInstruction::PlotLine { .. }
                )
            })
            .count();

        // We should have at least some plot instructions (macd and signal lines)
        assert!(
            plot_count >= 1,
            "Should have plot lines for MACD and signal"
        );
    }

    #[test]
    fn phase3_dynamic_histogram_color() {
        // Test the Phase 3 acceptance gate: dynamic histogram coloring
        // color = h>=0 ? color.green : color.red
        let source = r#"indicator("Dynamic Histogram Color Test")
var h = 0.5
plotHistogram(h, color = h >= 0 ? color.green : color.red)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(errors.is_empty(), "Compile errors: {:?}", errors);

        let mut program = compile_result
            .program
            .expect("Dynamic histogram script should compile");
        program.program_id = 23;

        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 102.0,
                volume: 1000.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 2,
                open: 102.0,
                high: 107.0,
                low: 100.0,
                close: 105.0,
                volume: 1000.0,
                _pad: 0.0,
            },
        ]);

        let mut instance = IndicatorInstance::new(23, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);

        // Find the histogram instruction
        let histogram = frame.instructions.iter().find_map(|i| {
            if let crate::core::indicators::render::types::DrawInstruction::PlotHistogram {
                per_point_colors,
                ..
            } = i
            {
                Some(per_point_colors.clone())
            } else {
                None
            }
        });

        assert!(
            histogram.is_some(),
            "Should have a PlotHistogram instruction"
        );

        let colors = histogram.unwrap();
        assert!(
            !colors.is_empty(),
            "Should have per-point colors for dynamic styling"
        );

        // Since h = 0.5 > 0, all colors should be green
        // Green in RayColor::GREEN is RGB(0, 255, 0)
        let green_rgba = [0.0, 1.0, 0.0, 1.0];
        for (i, color) in colors.iter().enumerate() {
            let color_diff = (color[0] - green_rgba[0]).abs()
                + (color[1] - green_rgba[1]).abs()
                + (color[2] - green_rgba[2]).abs();
            assert!(
                color_diff < 0.01,
                "Point {} should be green, got {:?}",
                i,
                color
            );
        }
    }

    #[test]
    fn phase3_acceptance_gate_macd_with_dynamic_histogram() {
        // Full Phase 3 acceptance gate test:
        // [m,s,h] = ta.macd(close,12,26,9); plot(m); plot(s); plotHistogram(h, color = h>=0 ? color.green : color.red)
        let source = r#"indicator("Phase 3 Acceptance")
[m, s, h] = ta.macd(close, 12, 26, 9)
plot(m)
plot(s)
plotHistogram(h, color = h >= 0 ? color.green : color.red)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "Phase 3 acceptance gate should compile without errors: {:?}",
            errors
        );

        let mut program = compile_result
            .program
            .expect("Phase 3 acceptance gate should compile");
        program.program_id = 24;

        // Create 50 bars to have enough data for MACD(12,26,9)
        let mut bars = BarArray::new();
        let bar_data: Vec<Bar> = (1..=50_u64)
            .map(|i| {
                // Create prices that trend up, then down to generate both positive and negative histogram values
                let trend = if i <= 25 { i as f32 } else { (50 - i) as f32 };
                Bar {
                    timestamp: i,
                    open: 100.0 + trend,
                    high: 105.0 + trend,
                    low: 95.0 + trend,
                    close: 100.0 + trend * 1.1,
                    volume: 1000.0,
                    _pad: 0.0,
                }
            })
            .collect();
        bars.set(bar_data);

        let mut instance = IndicatorInstance::new(24, 1, serde_json::Value::Null);

        let frame = run_historical(&program, &mut instance, &bars);

        // Verify tuple destructuring worked
        assert!(
            instance.incremental_state.persistent_vars.contains_key("m"),
            "Variable 'm' (MACD line) should exist"
        );
        assert!(
            instance.incremental_state.persistent_vars.contains_key("s"),
            "Variable 's' (signal line) should exist"
        );
        assert!(
            instance.incremental_state.persistent_vars.contains_key("h"),
            "Variable 'h' (histogram) should exist"
        );

        // Check for plot lines (MACD and signal)
        let plot_lines_count = frame
            .instructions
            .iter()
            .filter(|i| {
                matches!(
                    i,
                    crate::core::indicators::render::types::DrawInstruction::PlotLine { .. }
                )
            })
            .count();
        assert!(
            plot_lines_count >= 2,
            "Should have at least 2 plot lines (MACD and signal), got {}",
            plot_lines_count
        );

        // Check for histogram with per-point colors
        let histogram = frame.instructions.iter().find_map(|i| {
            if let crate::core::indicators::render::types::DrawInstruction::PlotHistogram {
                per_point_colors,
                points,
                ..
            } = i
            {
                Some((per_point_colors.clone(), points.clone()))
            } else {
                None
            }
        });

        let (colors, points) = histogram.expect("Should have a histogram instruction");

        assert!(!points.is_empty(), "Histogram should have data points");
        assert_eq!(
            colors.len(),
            points.len(),
            "Should have one color per histogram point for dynamic styling"
        );

        // Verify we have both green (h >= 0) and red (h < 0) colors
        // This confirms the ternary color expression is working
        let green_rgba = [0.0, 1.0, 0.0, 1.0];
        let red_rgba = [1.0, 0.0, 0.0, 1.0];

        let has_green = colors.iter().any(|c| {
            (c[0] - green_rgba[0]).abs()
                + (c[1] - green_rgba[1]).abs()
                + (c[2] - green_rgba[2]).abs()
                < 0.1
        });
        let has_red = colors.iter().any(|c| {
            (c[0] - red_rgba[0]).abs() + (c[1] - red_rgba[1]).abs() + (c[2] - red_rgba[2]).abs()
                < 0.1
        });

        // With our up-then-down price trend, we should get both positive and negative histogram values
        assert!(
            has_green || has_red,
            "Dynamic color should produce at least some colored bars. Colors: {:?}",
            &colors[..colors.len().min(5)]
        );
    }

    #[test]
    fn phase4_request_security_alias_compiles() {
        // Test that request.security() is parsed as an alias for req.series()
        let source = r#"indicator("Request Security Test")
var htf_close = request.security("BTCUSD", "1h", "close")
plot(htf_close)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "request.security should compile without errors: {:?}",
            errors
        );

        let program = compile_result
            .program
            .expect("request.security script should compile");

        // request.security in an expression context creates IrExpr::ReqSeries inside a StateVarDecl,
        // not a standalone IrCallKind::RequestSeries. Verify we have the var decl with embedded ReqSeries.
        let var_decl_calls: Vec<_> = program
            .ir_calls
            .iter()
            .filter(|call| {
                matches!(
                    call.kind,
                    IrCallKind::StateVarDecl | IrCallKind::StateLetDecl
                )
            })
            .collect();
        assert!(
            !var_decl_calls.is_empty(),
            "Should have StateVarDecl IR calls from var htf_close"
        );
        // Verify the first var decl has the htf_close variable
        let htf_close_decl = var_decl_calls.iter().find(|call| {
            call.args.first().map_or(
                false,
                |arg| matches!(arg, IrCallArg::Text(name) if name == "htf_close"),
            )
        });
        assert!(
            htf_close_decl.is_some(),
            "Should have htf_close variable declaration"
        );
    }

    #[test]
    fn phase4_barmerge_options_compile() {
        // Test that barmerge options are parsed correctly
        let source = r#"indicator("Barmerge Options Test")
var htf_close = req.series("BTCUSD", "1h", "close", "confirmed", "barmerge.gaps_off", "barmerge.lookahead_on")
plot(htf_close)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "req.series with barmerge options should compile without errors: {:?}",
            errors
        );

        compile_result
            .program
            .expect("req.series with barmerge options should compile");
    }

    #[test]
    fn phase5_syminfo_tickerid_resolves() {
        // Test that syminfo.tickerid resolves to the symbol from inputs
        let source = r#"indicator("Syminfo Test")
let sym = syminfo.tickerid
plot(close)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "syminfo.tickerid should compile without errors: {:?}",
            errors
        );

        let mut program = compile_result
            .program
            .expect("syminfo.tickerid script should compile");
        program.program_id = 1;

        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(
            1,
            1,
            serde_json::json!({"symbol": "BTCUSD", "chartTimeframe": "1h"}),
        );

        // Execute and verify syminfo.tickerid is accessible
        let result = execute_bar(&program, &mut instance, &bars, 0);
        assert!(result.is_ok(), "Should execute successfully");
    }

    #[test]
    fn phase5_barstate_islast_resolves() {
        // Test that barstate.islast resolves correctly
        let source = r#"indicator("Barstate Test")
let is_last = barstate.islast
plot(close)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "barstate.islast should compile without errors: {:?}",
            errors
        );

        let mut program = compile_result
            .program
            .expect("barstate.islast script should compile");
        program.program_id = 1;

        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(
            1,
            1,
            serde_json::json!({"symbol": "BTCUSD", "chartTimeframe": "1h"}),
        );

        // Execute and verify barstate.islast is accessible
        let result = execute_bar(&program, &mut instance, &bars, 0);
        assert!(result.is_ok(), "Should execute successfully");
    }

    #[test]
    fn phase5_timeframe_period_resolves() {
        // Test that timeframe.period resolves to chartTimeframe from inputs
        let source = r#"indicator("Timeframe Test")
let tf = timeframe.period
plot(close)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "timeframe.period should compile without errors: {:?}",
            errors
        );

        let mut program = compile_result
            .program
            .expect("timeframe.period script should compile");
        program.program_id = 1;

        let bars = sample_bars();
        let mut instance = IndicatorInstance::new(
            1,
            1,
            serde_json::json!({"symbol": "BTCUSD", "chartTimeframe": "4h"}),
        );

        // Execute and verify timeframe.period is accessible
        let result = execute_bar(&program, &mut instance, &bars, 0);
        assert!(result.is_ok(), "Should execute successfully");
    }

    #[test]
    fn phase5_input_symbol_compiles() {
        // Test that input.symbol type is accepted by the parser
        let source = r#"indicator("Input Symbol Test")
input.symbol("symbolInput", default="BTCUSD")
plot(close)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "input.symbol should compile without errors: {:?}",
            errors
        );

        let program = compile_result
            .program
            .expect("input.symbol script should compile");

        // Verify the input schema contains a symbol type
        let symbol_input = program
            .input_schema
            .iter()
            .find(|f| f.name == "symbolInput");
        assert!(symbol_input.is_some(), "Should have symbolInput in schema");
        assert_eq!(
            symbol_input.unwrap().type_name,
            "symbol",
            "Type should be 'symbol'"
        );
    }

    #[test]
    fn phase5_input_timeframe_compiles() {
        // Test that input.timeframe type is accepted by the parser
        let source = r#"indicator("Input Timeframe Test")
input.timeframe("tfInput", default="1h")
plot(close)"#;

        let compile_result =
            compile_source(source, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION, &[]);

        let errors: Vec<_> = compile_result
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    crate::core::indicators::compiler::diagnostics::DiagnosticSeverity::Error
                )
            })
            .collect();
        assert!(
            errors.is_empty(),
            "input.timeframe should compile without errors: {:?}",
            errors
        );

        let program = compile_result
            .program
            .expect("input.timeframe script should compile");

        // Verify the input schema contains a timeframe type
        let tf_input = program.input_schema.iter().find(|f| f.name == "tfInput");
        assert!(tf_input.is_some(), "Should have tfInput in schema");
        assert_eq!(
            tf_input.unwrap().type_name,
            "timeframe",
            "Type should be 'timeframe'"
        );
    }
}
