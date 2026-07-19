use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSource {
    Managed,
    Homebrew,
    Pip,
    Pipx,
    Custom,
    PathUnknown,
}

pub struct RuntimeMetadata {
    pub executable_path: std::path::PathBuf,
    #[allow(dead_code)]
    pub source: RuntimeSource,
    #[allow(dead_code)]
    pub version: String,
}
