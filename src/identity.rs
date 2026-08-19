//! Product identity and compatibility constants.
//!
//! Keep technology names (llama.cpp, GGUF, MLX, Rapid-MLX, and LHM) out of
//! this module: those are runtime technologies, not product identity.

#[allow(dead_code)] // consumed by later frontend/package phases
pub const PRODUCT_NAME: &str = "Local LLM Foundry";
#[allow(dead_code)] // consumed by later frontend/package phases
pub const PRODUCT_SHORT_NAME: &str = "Foundry";
pub const PRODUCT_SLUG: &str = "local-llm-foundry";
#[allow(dead_code)] // consumed by compatibility copy and migration phases
pub const LEGACY_PRODUCT_NAME: &str = "Llama Monitor";
pub const LEGACY_PRODUCT_SLUG: &str = "llama-monitor";
#[allow(dead_code)] // consumed by release and repository phases
pub const REPOSITORY: &str = "nmorgowicz-org/local-llm-foundry";
#[allow(dead_code)] // consumed by release and repository phases
pub const LEGACY_REPOSITORY: &str = "nmorgowicz-org/llama-monitor";
#[allow(dead_code)] // consumed by release and repository phases
pub const CANONICAL_RELEASE_ASSET_PREFIX: &str = "local-llm-foundry";
#[allow(dead_code)] // consumed by release and repository phases
pub const LEGACY_RELEASE_ASSET_PREFIX: &str = "llama-monitor";
pub const RELEASE_API_URL: &str =
    "https://api.github.com/repos/nmorgowicz-org/local-llm-foundry/releases/latest";
pub const RELEASE_USER_AGENT: &str = "local-llm-foundry";
pub const AGENT_LOG_RELATIVE_PATH: &str = "agent.log";
pub const CANONICAL_AGENT_TASK_NAME: &str = "LocalLLMFoundryAgent";
pub const LEGACY_AGENT_TASK_NAME: &str = "llama-monitor-agent";
pub const CANONICAL_SENSOR_TASK_NAME: &str = "LocalLLMFoundrySensorBridge";
pub const LEGACY_SENSOR_TASK_NAME: &str = "LlamaMonitorSensorBridge";
pub const CANONICAL_PROCESS_NAME: &str = "local-llm-foundry";
pub const LEGACY_PROCESS_NAME: &str = "llama-monitor";
pub const CANONICAL_AGENT_TOKEN_PREFIX: &str = "local-llm-foundry-agent-token-";
pub const LEGACY_AGENT_TOKEN_PREFIX: &str = "llama-monitor-agent-token-";
pub const UPDATE_STAGE_PREFIX: &str = ".local-llm-foundry-update-";

#[allow(dead_code)] // consumed by package and updater phases
pub fn binary_name(windows: bool) -> &'static str {
    if windows {
        "local-llm-foundry.exe"
    } else {
        "local-llm-foundry"
    }
}

#[allow(dead_code)] // consumed by package and updater phases
pub fn legacy_binary_name(windows: bool) -> &'static str {
    if windows {
        "llama-monitor.exe"
    } else {
        "llama-monitor"
    }
}

pub fn install_path(windows: bool) -> &'static str {
    if windows {
        "%APPDATA%\\local-llm-foundry\\bin\\local-llm-foundry.exe"
    } else {
        "~/.config/local-llm-foundry/bin/local-llm-foundry"
    }
}

pub fn legacy_install_path(windows: bool) -> &'static str {
    if windows {
        "%APPDATA%\\llama-monitor\\bin\\llama-monitor.exe"
    } else {
        "~/.config/llama-monitor/bin/llama-monitor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_identity_is_stable() {
        assert_eq!(PRODUCT_NAME, "Local LLM Foundry");
        assert_eq!(PRODUCT_SLUG, "local-llm-foundry");
        assert_eq!(binary_name(false), "local-llm-foundry");
        assert_eq!(binary_name(true), "local-llm-foundry.exe");
        assert_eq!(legacy_binary_name(false), "llama-monitor");
        assert_eq!(legacy_binary_name(true), "llama-monitor.exe");
    }
}
