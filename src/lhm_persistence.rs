use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct LhmDisabledFlag {
    disabled: bool,
}

pub fn save_lhm_disabled(path: &Path, disabled: bool) -> Result<(), String> {
    let flag = LhmDisabledFlag { disabled };
    let json = serde_json::to_string_pretty(&flag)
        .map_err(|e| format!("Failed to serialize LHM disabled flag: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write LHM disabled flag: {}", e))?;
    Ok(())
}

#[allow(dead_code)]
pub fn load_lhm_disabled(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read LHM disabled flag: {}", e))?;
    let flag: LhmDisabledFlag = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse LHM disabled flag: {}", e))?;
    Ok(flag.disabled)
}
