use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CapabilitySet {
    pub vision: bool,
    pub mtp: bool,
    pub cancellation: bool,
    pub embeddings: bool,
    pub guided_generation: bool,
    pub audio: bool,
    pub tool_parsing: bool,
    pub automatic_tool_choice: bool,
    pub reasoning_parser: bool,
    pub thinking_controls: bool,
    pub mcp: bool,
    pub cache_telemetry: bool,
    pub status_memory_telemetry: bool,
    pub self_diagnostic: bool,
    pub interpretability: bool,
    pub one_shot_launch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityProfile {
    pub version_range: String,
    pub source_commit: String,
    pub core_cli_flags: Vec<String>,
    pub endpoint_schemas: Vec<String>,
    pub optional_feature_flags: Vec<String>,
    pub known_incompatibilities: Vec<String>,
    pub fixture_identifiers: Vec<String>,
    pub classification: ProfileClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProfileClassification {
    Verified,
    Provisional,
    Legacy,
    Incompatible,
}
