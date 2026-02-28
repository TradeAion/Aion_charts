use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_ops_per_bar: u64,
    pub max_wall_time_per_bar_ms: u64,
    pub max_memory_bytes_per_instance: usize,
    pub max_objects_per_instance: usize,
    pub max_vertices_per_frame: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_ops_per_bar: 250_000,
            max_wall_time_per_bar_ms: 4,
            max_memory_bytes_per_instance: 32 * 1024 * 1024,
            max_objects_per_instance: 1_000,
            max_vertices_per_frame: 2_000_000,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceCounters {
    pub ops_used: u64,
    pub last_elapsed_micros: u64,
    pub peak_objects: usize,
    pub peak_vertices: usize,
}
