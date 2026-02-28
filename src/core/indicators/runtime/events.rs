use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    RuntimeError {
        code: String,
        message: String,
        bar_index: usize,
    },
    LimitsExceeded {
        code: String,
        message: String,
        bar_index: usize,
    },
}
