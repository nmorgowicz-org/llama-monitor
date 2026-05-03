# mactop JSON Schema Investigation
**Date:** 2026-04-12  
**Last Updated:** 2026-04-12 (test results from M1 Pro)  
**Goal:** Determine if mactop provides GPU VRAM metrics or if estimation is needed

## Background

For llama-monitor Apple Silicon support, we need GPU metrics including:
- `temp` - GPU temperature (°C)
- `load` - GPU utilization (%)
- `power_consumption` - GPU power (W)
- `power_limit` - Power limit (W)
- `vram_used` - VRAM used (MB)
- `vram_total` - Total VRAM (MB)
- `sclk_mhz` - GPU clock (MHz)
- `mclk_mhz` - Memory clock (MHz)

## mactop Investigation

### Current Understanding

mactop is a Go-based TUI tool for Apple Silicon monitoring. It has a `--headless` mode that outputs JSON.

**Question:** Does mactop expose VRAM/DRAM usage metrics in its JSON output?

### What We Know

From mactop docs:
- "DRAM Bandwidth Monitoring: Real-time DRAM read/write bandwidth (GB/s)"
- "Comprehensive Temperature Sensors: All available SMC temperature sensors"
- "GPU frequency and usage percentage display"
- "Real-time CPU, GPU, ANE, DRAM, and system power wattage usage display"

## Test Results (M1 Pro, macOS)

### JSON Output Structure

Running `mactop --headless --count 1 --format json` produces this structure:

```json
{
  "timestamp": "2026-04-12T12:25:07-04:00",
  "soc_metrics": {
    "cpu_power": 5.531,
    "gpu_power": 0.96485899,
    "ane_power": 0,
    "dram_power": 1.512,
    "gpu_sram_power": 0.03,
    "system_power": 16.705930682851566,
    "total_power": 24.743789672851562,
    "gpu_freq_mhz": 601,
    "gpu_active": 37.98882452782923,
    "e_cluster_active": 56.543921839517864,
    "p_cluster_active": 75.72355717705965,
    "e_cluster_freq_mhz": 1446,
    "p_cluster_freq_mhz": 3224,
    "soc_temp": 69.50542,
    "cpu_temp": 69.50542,
    "gpu_temp": 64.27033,
    "dram_read_bw_gbs": 31.333405136,
    "dram_write_bw_gbs": 31.764176496,
    "dram_bw_combined_gbs": 63.097581632
  },
  "memory": {
    "total": 34359738368,        // 32GB in bytes
    "used": 17606017024,          // ~16.4GB used
    "available": 16753721344,
    "swap_total": 7516192768,
    "swap_used": 6453198848
  },
  "net_disk": {
    "read_kbytes_per_sec": 8710.887926715981,
    "write_kbytes_per_sec": 1378.2022105521469
  },
  "cpu_usage": 35.42535568494544,
  "gpu_usage": 37.98882452782923,
  "gpu_metrics": {
    "freq_mhz": 601,
    "active_percent": 37.98882452782923
  },
  "tflops_fp32": 4.644864,
  "tflops_fp16": 9.289728,
  "core_usages": [...],
  "system_info": {
    "name": "Apple M1 Pro",
    "core_count": 8,
    "e_core_count": 2,
    "p_core_count": 6,
    "gpu_core_count": 14
  },
  "thermal_state": "Normal",
  "processes": [...],
  "network_links": {...},
  "volumes": [...],
  "thunderbolt_info": {...},
  "tb_net_total_bytes_in_per_sec": 0,
  "tb_net_total_bytes_out_per_sec": 0,
  "rdma_status": {...},
  "fans": [
    {"id": 0, "name": "Fan 0", "rpm": 2323, "target_rpm": 2317, "min_rpm": 1200, "max_rpm": 5779, "mode": "auto"},
    {"id": 1, "name": "Fan 1", "rpm": 2495, "target_rpm": 2502, "min_rpm": 1200, "max_rpm": 6241, "mode": "auto"}
  ],
  "temperatures": [...]
}
```

### Mapped Fields for llama-monitor

| llama-monitor Field | mactop Field | Value | Notes |
|---------------------|--------------|-------|-------|
| `temp` | `soc_metrics.gpu_temp` | 64.27°C | GPU temp in Celsius |
| `load` | `gpu_usage` or `soc_metrics.gpu_active` | 37.99% | GPU utilization % |
| `power_consumption` | `soc_metrics.gpu_power` | 0.96W | GPU power in Watts |
| `power_limit` | N/A | `0` (unknown) | Not available in mactop |
| `vram_used` | `memory.used` | 16.4GB | Unified memory, not VRAM-specific |
| `vram_total` | `memory.total` | 32GB | Total unified memory |
| `sclk_mhz` | `soc_metrics.gpu_freq_mhz` | 601 MHz | GPU clock |
| `mclk_mhz` | N/A | `0` (estimated) | Not available - can derive from dram_bw |

### Missing Fields

| Field | Status | Notes |
|-------|--------|-------|
| `power_limit` | Not available | mactop doesn't expose power limits |
| `mclk_mhz` | Not available | Memory clock not exposed, but we have DRAM bandwidth |

### Key Findings

1. **Unified Memory = VRAM for Apple Silicon**: mactop provides `memory.used` and `memory.total` which represent unified memory usage. For Apple Silicon, this IS the "VRAM" pool.

2. **Memory is shared**: On Apple Silicon, there's no separate VRAM - the GPU shares system DRAM. So `memory.used` is the closest equivalent to `vram_used`.

3. **Need to calculate total memory**: The `memory.total` is in bytes (34359738368 = 32GB). We need to convert to MB for GpuMetrics compatibility.

4. **Fans and IO available**: mactop provides `fans` array with RPM values and `net_disk` for network/disk I/O.

## Implementation Strategy

### Option A: Use mactop only (simpler)

Map mactop metrics to GpuMetrics:

```rust
#[derive(Deserialize)]
struct MactopMetrics {
    timestamp: String,
    soc_metrics: SocMetrics,
    memory: MemoryMetrics,
    net_disk: NetDiskMetrics,
    fans: Vec<Fan>,
    // ... other fields
}

#[derive(Deserialize)]
struct SocMetrics {
    gpu_power: f64,
    gpu_freq_mhz: f64,
    gpu_temp: f64,
    dram_read_bw_gbs: f64,
    dram_write_bw_gbs: f64,
    // ... other fields
}

#[derive(Deserialize)]
struct MemoryMetrics {
    total: u64,  // bytes
    used: u64,   // bytes
}

impl GpuBackend for AppleBackend {
    fn read_metrics(&self) -> Result<BTreeMap<String, GpuMetrics>> {
        // Run mactop --headless --count 1 --format json
        // Parse JSON
        // Map fields to GpuMetrics
        
        // vram_total = memory.total in MB
        // vram_used = memory.used in MB
    }
}
```

### Hybrid Approach (if macmon is preferred for VRAM)

Use mactop for everything + macmon just for VRAM metrics. Not recommended since mactop's `memory.used` is sufficient.

## Implementation Plan

### Step 1: Update `GpuMetrics` struct

The current `GpuMetrics` struct in `src/gpu/mod.rs` already has all required fields:
- `temp` - GPU temperature (°C) - ✅ available via `soc_metrics.gpu_temp`
- `load` - GPU utilization (%) - ✅ available via `gpu_usage`
- `power_consumption` - GPU power (W) - ✅ available via `soc_metrics.gpu_power`
- `power_limit` - Power limit (W) - ❌ not available, set to 0
- `vram_used` - VRAM used (MB) - ✅ available via `memory.used` (bytes → MB)
- `vram_total` - Total VRAM (MB) - ✅ available via `memory.total` (bytes → MB)
- `sclk_mhz` - GPU clock (MHz) - ✅ available via `soc_metrics.gpu_freq_mhz`
- `mclk_mhz` - Memory clock (MHz) - ❌ not available, estimate from dram_bw

### Step 2: Implement `src/gpu/apple.rs`

```rust
use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::process::Command;

use super::{GpuBackend, GpuMetrics};

#[derive(Deserialize)]
struct MactopOutput {
    soc_metrics: SocMetrics,
    memory: MemoryMetrics,
    net_disk: NetDiskMetrics,
    fans: Vec<Fan>,
    temperatures: Vec<TemperatureGroup>,
}

#[derive(Deserialize)]
struct SocMetrics {
    gpu_power: f64,
    gpu_freq_mhz: f64,
    gpu_temp: f64,
    dram_read_bw_gbs: f64,
    dram_write_bw_gbs: f64,
}

#[derive(Deserialize)]
struct MemoryMetrics {
    total: u64,  // bytes
    used: u64,   // bytes
}

#[derive(Deserialize)]
struct NetDiskMetrics {
    read_kbytes_per_sec: f64,
    write_kbytes_per_sec: f64,
}

#[derive(Deserialize)]
struct Fan {
    id: u64,
    name: String,
    rpm: u64,
    target_rpm: u64,
    min_rpm: u64,
    max_rpm: u64,
    mode: String,
}

#[derive(Deserialize)]
struct TemperatureGroup {
    group: String,
    avg_celsius: f64,
    min_celsius: f64,
    max_celsius: f64,
    sensor_count: u64,
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

        let mactop_output: MactopOutput = serde_json::from_slice(&output.stdout)?;

        // Convert bytes to MB
        let vram_total_mb = mactop_output.memory.total / (1024 * 1024);
        let vram_used_mb = mactop_output.memory.used / (1024 * 1024);

        // Estimate memory clock from DRAM bandwidth
        // Approximate: MCLK = (dram_bw_gbs * 1000) / 8 / 2 (DDR)
        let mclk_mhz = (mactop_output.soc_metrics.dram_read_bw_gbs + 
                       mactop_output.soc_metrics.dram_write_bw_gbs) * 1000.0 / 16.0;

        let metrics = GpuMetrics {
            temp: mactop_output.soc_metrics.gpu_temp as i32,
            load: mactop_output.soc_metrics.gpu_active as i32,  // Use active% if available
            power_consumption: mactop_output.soc_metrics.gpu_power as i32,
            power_limit: 0,  // Not available from mactop
            vram_used: vram_used_mb as i32,
            vram_total: vram_total_mb as i32,
            sclk_mhz: mactop_output.soc_metrics.gpu_freq_mhz as i32,
            mclk_mhz: mclk_mhz as i32,
        };

        let mut map = BTreeMap::new();
        map.insert("GPU0 Apple M1 Pro".to_string(), metrics);
        Ok(map)
    }

    fn name(&self) -> &str {
        "apple"
    }
}
```

### Step 3: Update `src/gpu/mod.rs`

Add Apple backend detection:

```rust
pub fn detect_backend() -> Result<Box<dyn GpuBackend>> {
    // Check for Apple Silicon first
    if is_apple_silicon()? {
        return AppleBackend::new().detect();
    }
    
    // ... existing nvidia/rocm detection
}

fn is_apple_silicon() -> Result<bool> {
    let output = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    Ok(output_str.contains("Apple") || output_str.contains("M"))
}
```

### Step 4: Add to `Cargo.toml`

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Step 5: Test

```bash
cd llama-monitor
cargo build
./target/debug/llama-monitor --gpu-backend apple
```

## Action Items

1. Implement `src/gpu/apple.rs` using mactop JSON output
2. Handle unified memory mapping (memory.used/memory.total as VRAM)
3. Add fan/RPM metrics as separate GpuMetrics entries if needed
4. Test on M1/M2/M3 hardware
5. Update `README.md` with Apple Silicon monitoring instructions

## Files

- **Test script**: `docs/2026-04-12-mactop-test-script.sh`
- **Test output**: See test results above (M1 Pro, 32GB unified memory)
- **Schema doc**: `docs/2026-04-12-mactop-json-schema.md`

## Files

- **Test script**: `docs/2026-04-12-mactop-test-script.sh`
- **Test output**: See test results above (M1 Pro, 32GB unified memory)
