use serde::{Deserialize, Serialize};

pub mod backend;
pub mod capabilities;
pub mod metrics;
pub mod supervisor;
pub mod llama_cpp;
pub mod rapid_mlx;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    LlamaCpp,
    RapidMlx,
}
