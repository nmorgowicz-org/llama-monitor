pub mod apple;
pub mod dummy;
pub mod env;
pub mod nvidia;
pub mod rocm;

use anyhow::Result;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize)]
pub struct GpuMetrics {
    pub temp: f32,
    pub load: u32,
    pub power_consumption: f32,
    pub power_limit: u32,
    pub vram_used: u64,
    pub vram_total: u64,
    pub sclk_mhz: u32,
    pub mclk_mhz: u32,
}

pub trait GpuBackend: Send + Sync + 'static {
    fn read_metrics(&self) -> Result<BTreeMap<String, GpuMetrics>>;
    #[allow(dead_code)]
    fn name(&self) -> &str;
}

pub fn detect_backend(force: &str) -> Arc<dyn GpuBackend> {
    match force {
        "apple" => Arc::new(apple::AppleBackend::new()),
        "rocm" => Arc::new(rocm::RocmBackend),
        "nvidia" => Arc::new(nvidia::NvidiaBackend),
        "none" => Arc::new(dummy::DummyBackend),
        _ => {
            // Auto-detect: check which GPU tool is available
            if is_apple_silicon() {
                Arc::new(apple::AppleBackend::new())
            } else if command_exists("rocm-smi") {
                Arc::new(rocm::RocmBackend)
            } else if command_exists("nvidia-smi") {
                Arc::new(nvidia::NvidiaBackend)
            } else {
                eprintln!(
                    "[warn] No GPU monitoring tool found (apple/mactop, rocm-smi, nvidia-smi)"
                );
                Arc::new(dummy::DummyBackend)
            }
        }
    }
}

fn is_apple_silicon() -> bool {
    // Check for Apple Silicon by looking at the CPU brand string
    let output = std::process::Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok();

    if let Some(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout.contains("Apple")
            || stdout.contains("M")
            || stdout.contains("M1")
            || stdout.contains("M2")
            || stdout.contains("M3")
            || stdout.contains("M4");
    }

    // Fallback: check if mactop is available
    command_exists("mactop")
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
