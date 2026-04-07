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
        "rocm" => Arc::new(rocm::RocmBackend),
        "nvidia" => Arc::new(nvidia::NvidiaBackend),
        "none" => Arc::new(dummy::DummyBackend),
        _ => {
            // Auto-detect: check which GPU tool is available
            if command_exists("rocm-smi") {
                Arc::new(rocm::RocmBackend)
            } else if command_exists("nvidia-smi") {
                Arc::new(nvidia::NvidiaBackend)
            } else {
                eprintln!("[warn] No GPU monitoring tool found (rocm-smi / nvidia-smi)");
                Arc::new(dummy::DummyBackend)
            }
        }
    }
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
