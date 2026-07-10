use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CapabilitySet {
    pub vision: bool,
    pub dflash: bool,
    pub embeddings: bool,
    pub guided_generation: bool,
    pub audio: bool,
    pub chat_extras: bool,
    pub mtp: bool,
    pub suffix_decoding: bool,
    pub kv_quantization: bool,
    pub turbo_quant: bool,
    pub tool_parsing: bool,
    pub auto_tool_choice: bool,
    pub reasoning_parser: bool,
    pub mcp: bool,
    pub cache_telemetry: bool,
    pub cancellation: bool,
    pub status_memory: bool,
    pub doctor: bool,
    pub jlens: bool,
}
