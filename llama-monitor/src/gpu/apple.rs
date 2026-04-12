use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::process::Command;

use super::{GpuBackend, GpuMetrics};

#[derive(Deserialize)]
struct MactopOutput {
    soc_metrics: SocMetrics,
    memory: MemoryMetrics,
    // gpu_usage is redundant with soc_metrics.gpu_active
}

#[derive(Deserialize)]
struct SocMetrics {
    gpu_power: f64,
    gpu_freq_mhz: f64,
    gpu_temp: f64,
    gpu_active: f64,
    dram_read_bw_gbs: f64,
    dram_write_bw_gbs: f64,
}

#[derive(Deserialize)]
struct MemoryMetrics {
    total: u64,  // bytes
    used: u64,   // bytes
}

pub struct AppleBackend;

impl AppleBackend {
    pub fn new() -> Self {
        AppleBackend
    }
}

impl GpuBackend for AppleBackend {
    fn read_metrics(&self) -> Result<BTreeMap<String, GpuMetrics>> {
        let output = Command::new("mactop")
            .args(["--headless", "--count", "1", "--format", "json"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "mactop failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let mactop_output: MactopOutput = serde_json::from_slice(&output.stdout).map_err(|e| {
            anyhow::anyhow!("Failed to parse mactop JSON: {}", e)
        })?;

        // Convert bytes to MB
        let vram_total_mb = mactop_output.memory.total / (1024 * 1024);
        let vram_used_mb = mactop_output.memory.used / (1024 * 1024);

        // Estimate memory clock from DRAM bandwidth
        // Approximate: MCLK = (dram_bw_gbs * 1000) / 8 / 2 (DDR)
        let mclk_mhz = (mactop_output.soc_metrics.dram_read_bw_gbs + 
                       mactop_output.soc_metrics.dram_write_bw_gbs) * 1000.0 / 16.0;

        let metrics = GpuMetrics {
            temp: mactop_output.soc_metrics.gpu_temp,
            load: mactop_output.soc_metrics.gpu_active as u32,
            power_consumption: mactop_output.soc_metrics.gpu_power,
            power_limit: 0,  // Not available from mactop
            vram_used: vram_used_mb as u64,
            vram_total: vram_total_mb as u64,
            sclk_mhz: mactop_output.soc_metrics.gpu_freq_mhz as u32,
            mclk_mhz: mclk_mhz as u32,
        };

        let mut map = BTreeMap::new();
        map.insert("GPU0 Apple M1 Pro".to_string(), metrics);
        Ok(map)
    }

    fn name(&self) -> &str {
        "apple"
    }
}
