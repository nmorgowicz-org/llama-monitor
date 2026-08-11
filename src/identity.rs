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
