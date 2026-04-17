use crate::core::data::BarArray;
use crate::core::indicators::render::types::ObjectMutation;
use crate::core::indicators::runtime::events::RuntimeEvent;
use crate::core::indicators::runtime::instance::IndicatorInstance;
use crate::core::indicators::runtime::mtf::MtfResolver;
use crate::core::indicators::runtime::vm::execute_bar_with_resolver;
use crate::core::indicators::IndicatorProgram;

#[derive(Debug, Default, Clone)]
pub struct Scheduler;

impl Scheduler {
    pub fn run_historical(
        &self,
        program: &IndicatorProgram,
        instance: &mut IndicatorInstance,
        bars: &BarArray,
        mtf_resolver: &dyn MtfResolver,
    ) -> Result<(), RuntimeEvent> {
        if bars.is_empty() {
            instance.last_good_frame = None;
            return Ok(());
        }
        // Reset accumulated state for a full re-run.
        instance.reset_incremental_state();
        for bar_index in 0..bars.len() {
            let frame =
                execute_bar_with_resolver(program, instance, bars, bar_index, mtf_resolver)?;
            let projected_objects = projected_object_count(instance, &frame.object_mutations);
            if projected_objects > instance.limits.max_objects_per_instance {
                return Err(RuntimeEvent::LimitsExceeded {
                    code: "INDL-2005".to_string(),
                    message: "max objects per instance exceeded".to_string(),
                    bar_index,
                });
            }
            let previous_registry = instance.object_registry.clone();
            let previous_frame = instance.last_good_frame.clone();
            instance.apply_object_mutations(&frame.object_mutations);
            instance.last_good_frame = Some(frame);
            if instance.estimate_memory_bytes() > instance.limits.max_memory_bytes_per_instance {
                instance.object_registry = previous_registry;
                instance.last_good_frame = previous_frame;
                return Err(RuntimeEvent::LimitsExceeded {
                    code: "INDL-2006".to_string(),
                    message: "max memory per instance exceeded".to_string(),
                    bar_index,
                });
            }
        }
        instance.updates_applied = instance.updates_applied.saturating_add(1);
        Ok(())
    }

    pub fn run_incremental(
        &self,
        program: &IndicatorProgram,
        instance: &mut IndicatorInstance,
        bars: &BarArray,
        mtf_resolver: &dyn MtfResolver,
    ) -> Result<(), RuntimeEvent> {
        if bars.is_empty() {
            return Ok(());
        }
        let bar_index = bars.len().saturating_sub(1);
        let frame = execute_bar_with_resolver(program, instance, bars, bar_index, mtf_resolver)?;
        let projected_objects = projected_object_count(instance, &frame.object_mutations);
        if projected_objects > instance.limits.max_objects_per_instance {
            return Err(RuntimeEvent::LimitsExceeded {
                code: "INDL-2005".to_string(),
                message: "max objects per instance exceeded".to_string(),
                bar_index,
            });
        }
        let previous_registry = instance.object_registry.clone();
        let previous_frame = instance.last_good_frame.clone();
        instance.apply_object_mutations(&frame.object_mutations);
        instance.last_good_frame = Some(frame);
        if instance.estimate_memory_bytes() > instance.limits.max_memory_bytes_per_instance {
            instance.object_registry = previous_registry;
            instance.last_good_frame = previous_frame;
            return Err(RuntimeEvent::LimitsExceeded {
                code: "INDL-2006".to_string(),
                message: "max memory per instance exceeded".to_string(),
                bar_index,
            });
        }
        instance.updates_applied = instance.updates_applied.saturating_add(1);
        Ok(())
    }
}

fn projected_object_count(instance: &IndicatorInstance, mutations: &[ObjectMutation]) -> usize {
    let mut existing: std::collections::BTreeSet<u64> =
        instance.object_registry.keys().copied().collect();
    for mutation in mutations {
        match mutation {
            ObjectMutation::Create { id, .. } => {
                existing.insert(*id);
            }
            ObjectMutation::Delete { id } => {
                existing.remove(id);
            }
            ObjectMutation::Update { .. } => {}
        }
    }
    existing.len()
}

#[cfg(test)]
mod tests {
    use super::Scheduler;
    use crate::core::data::{Bar, BarArray};
    use crate::core::indicators::compiler::compile_source;
    use crate::core::indicators::runtime::events::RuntimeEvent;
    use crate::core::indicators::runtime::instance::IndicatorInstance;
    use crate::core::indicators::runtime::mtf::NoopMtfResolver;
    use crate::core::indicators::{
        IndicatorProgram, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION,
    };

    fn sample_program(line: &str) -> IndicatorProgram {
        let source = format!("indicator(\"sched\")\n{line}");
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
                timestamp: 100,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                volume: 100.0,
            },
            Bar {
                timestamp: 200,
                open: 11.0,
                high: 13.0,
                low: 10.0,
                close: 12.0,
                volume: 200.0,
            },
        ])
        .unwrap();
        bars
    }

    #[test]
    fn enforces_max_objects_limit() {
        let program = sample_program("box.new(bar_index, 0, low, 1, high)");
        let mut instance = IndicatorInstance::new(1, 1, serde_json::Value::Null);
        instance.limits.max_objects_per_instance = 1;
        let scheduler = Scheduler;
        let bars = sample_bars();
        let mtf = NoopMtfResolver;

        let result = scheduler.run_historical(&program, &mut instance, &bars, &mtf);
        match result {
            Err(RuntimeEvent::LimitsExceeded { code, .. }) => assert_eq!(code, "INDL-2005"),
            other => panic!("expected limits exceeded, got {:?}", other),
        }
    }

    #[test]
    fn enforces_max_memory_limit() {
        let program = sample_program("label.new(bar_index, close, \"memory-heavy-label\")");
        let mut instance = IndicatorInstance::new(2, 1, serde_json::json!({"a": "b"}));
        instance.limits.max_memory_bytes_per_instance = 32;
        let scheduler = Scheduler;
        let bars = sample_bars();
        let mtf = NoopMtfResolver;

        let result = scheduler.run_incremental(&program, &mut instance, &bars, &mtf);
        match result {
            Err(RuntimeEvent::LimitsExceeded { code, .. }) => assert_eq!(code, "INDL-2006"),
            other => panic!("expected limits exceeded, got {:?}", other),
        }
    }
}
