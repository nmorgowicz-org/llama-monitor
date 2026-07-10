use serde::Serialize;
use std::time::SystemTime;
use crate::inference::InferenceBackend;

#[derive(Debug, Clone, Serialize)]
pub struct InferenceMetricsSnapshot {
    pub sampled_at: SystemTime,
    pub backend: InferenceBackend,
    // Health
    pub health: Option<HealthState>,
    pub ready: Option<bool>,
    // Identity
    pub model: Option<String>,
    pub uptime_seconds: Option<f64>,
    // Throughput
    pub generation_tokens_per_second: Option<f64>,
    pub prompt_tokens_per_second: Option<f64>,
    // Queue
    pub running_requests: Option<u64>,
    pub waiting_requests: Option<u64>,
    // Totals (cumulative)
    pub completed_requests_total: Option<u64>,
    pub prompt_tokens_total: Option<u64>,
    pub completion_tokens_total: Option<u64>,
    pub steps_executed: Option<u64>,
    pub global_cache_hit_rate: Option<f64>,
    pub global_cache_entries: Option<u64>,
    // Memory (always in bytes)
    pub active_memory_bytes: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub cache_memory_bytes: Option<u64>,
    // Structured opaque payloads
    pub cache_metrics: Option<serde_json::Value>,
    pub active_requests: Option<Vec<serde_json::Value>>,
    pub backend_details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub enum HealthState {
    Ok,
    Degraded,
    NotLoaded,
    Unreachable,
}
