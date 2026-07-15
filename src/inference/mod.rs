use serde::{Deserialize, Serialize};

pub mod backend;
pub mod capabilities;
pub mod llama_cpp;
pub mod metrics;
pub mod rapid_mlx;
pub mod supervisor;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    LlamaCpp,
    RapidMlx,
}
