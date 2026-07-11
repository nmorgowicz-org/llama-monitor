use serde::{Deserialize, Serialize};

pub mod backend;
pub mod capabilities;
pub mod metrics;
pub mod supervisor;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    LlamaCpp,
    RapidMlx,
}
