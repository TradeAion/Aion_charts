//! Study manager — orchestrates study calculation and caching.
//!
//! Manages study instances, incremental calculation, and result caching.
//! Studies are calculated on-demand and cached until new data arrives.

use std::collections::HashMap;
use crate::core::data::BarArray;
use crate::core::series::LineDataArray;

/// Unique identifier for a study instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StudyId(pub u32);

/// Study input source — which price data to use for calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudyInput {
    /// Use close prices from the main bar data.
    Close,
    /// Use open prices from the main bar data.
    Open,
    /// Use high prices from the main bar data.
    High,
    /// Use low prices from the main bar data.
    Low,
    /// Use volume from the main bar data.
    Volume,
    /// Use data from another study (by StudyId).
    StudyOutput(StudyId, usize), // study_id, output_index
}

/// Study output — the result of a study calculation.
#[derive(Debug, Clone)]
pub struct StudyOutput {
    pub name: String,
    pub data: LineDataArray,
    pub visible: bool,
}

/// A study instance with its configuration and results.
pub struct Study {
    pub id: StudyId,
    pub study_type: String,
    pub inputs: Vec<StudyInput>,
    pub parameters: HashMap<String, f64>,
    pub outputs: Vec<StudyOutput>,
    pub last_calculated_index: usize, // last bar index processed
}

impl Study {
    pub fn new(id: StudyId, study_type: String) -> Self {
        Self {
            id,
            study_type,
            inputs: Vec::new(),
            parameters: HashMap::new(),
            outputs: Vec::new(),
            last_calculated_index: 0,
        }
    }

    pub fn add_input(&mut self, input: StudyInput) {
        self.inputs.push(input);
    }

    pub fn set_parameter(&mut self, key: String, value: f64) {
        self.parameters.insert(key, value);
    }

    pub fn add_output(&mut self, name: String, visible: bool) -> usize {
        let index = self.outputs.len();
        self.outputs.push(StudyOutput {
            name,
            data: LineDataArray::new(),
            visible,
        });
        index
    }

    pub fn get_output(&self, index: usize) -> Option<&StudyOutput> {
        self.outputs.get(index)
    }

    pub fn get_output_mut(&mut self, index: usize) -> Option<&mut StudyOutput> {
        self.outputs.get_mut(index)
    }
}

/// Study calculation trait — implemented by built-in studies.
pub trait StudyCalculator {
    fn name(&self) -> &str;
    fn calculate(&self, study: &mut Study, bars: &BarArray, start_index: usize, end_index: usize);
}

/// Manages all studies on a chart.
pub struct StudyManager {
    studies: HashMap<StudyId, Study>,
    calculators: HashMap<String, Box<dyn StudyCalculator>>,
    next_id: u32,
}

impl StudyManager {
    pub fn new() -> Self {
        Self {
            studies: HashMap::new(),
            calculators: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a built-in study calculator.
    pub fn register_calculator(&mut self, calculator: Box<dyn StudyCalculator>) {
        self.calculators.insert(calculator.name().to_string(), calculator);
    }

    /// Create a new study instance.
    pub fn create_study(&mut self, study_type: &str) -> Option<StudyId> {
        if !self.calculators.contains_key(study_type) {
            return None;
        }

        let id = StudyId(self.next_id);
        self.next_id += 1;

        let mut study = Study::new(id, study_type.to_string());

        // Initialize study with default configuration
        if let Some(_calculator) = self.calculators.get(study_type) {
            // For now, just set up basic inputs. In a full implementation,
            // this would be configurable per study type.
            match study_type {
                "sma" => {
                    study.add_input(StudyInput::Close);
                    study.set_parameter("period".to_string(), 20.0);
                    study.add_output("SMA".to_string(), true);
                }
                "ema" => {
                    study.add_input(StudyInput::Close);
                    study.set_parameter("period".to_string(), 20.0);
                    study.add_output("EMA".to_string(), true);
                }
                "rsi" => {
                    study.add_input(StudyInput::Close);
                    study.set_parameter("period".to_string(), 14.0);
                    study.add_output("RSI".to_string(), true);
                }
                "macd" => {
                    study.add_input(StudyInput::Close);
                    study.set_parameter("fast_period".to_string(), 12.0);
                    study.set_parameter("slow_period".to_string(), 26.0);
                    study.set_parameter("signal_period".to_string(), 9.0);
                    study.add_output("MACD".to_string(), true);
                    study.add_output("Signal".to_string(), true);
                    study.add_output("Histogram".to_string(), true);
                }
                _ => {}
            }
        }

        self.studies.insert(id, study);
        Some(id)
    }

    /// Remove a study.
    pub fn remove_study(&mut self, id: StudyId) -> bool {
        self.studies.remove(&id).is_some()
    }

    /// Get a study by ID.
    pub fn get_study(&self, id: StudyId) -> Option<&Study> {
        self.studies.get(&id)
    }

    /// Get a mutable study by ID.
    pub fn get_study_mut(&mut self, id: StudyId) -> Option<&mut Study> {
        self.studies.get_mut(&id)
    }

    /// Update all studies with new bar data.
    pub fn update_studies(&mut self, bars: &BarArray) {
        for study in self.studies.values_mut() {
            if let Some(calculator) = self.calculators.get(&study.study_type) {
                let start_index = study.last_calculated_index;
                let end_index = bars.len().saturating_sub(1);
                if start_index <= end_index {
                    calculator.calculate(study, bars, start_index, end_index);
                    study.last_calculated_index = end_index + 1;
                }
            }
        }
    }

    /// Get all studies.
    pub fn all_studies(&self) -> impl Iterator<Item = &Study> {
        self.studies.values()
    }

    /// Get study count.
    pub fn study_count(&self) -> usize {
        self.studies.len()
    }

    /// Get mutable iterator over all studies (for resetting calculation indices).
    pub fn studies_iter_mut(&mut self) -> impl Iterator<Item = &mut Study> {
        self.studies.values_mut()
    }
}
