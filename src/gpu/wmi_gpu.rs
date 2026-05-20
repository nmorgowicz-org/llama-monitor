use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use wmi::{Variant, WMIConnection};

use super::{GpuBackend, GpuMetrics};

pub struct WmiGpuBackend;

impl GpuBackend for WmiGpuBackend {
    fn read_metrics(&self) -> Result<BTreeMap<String, GpuMetrics>> {
        let wmi =
            WMIConnection::new().map_err(|e| anyhow::anyhow!("WMI connection failed: {e}"))?;

        let rows: Vec<HashMap<String, Variant>> = wmi
            .raw_query::<HashMap<String, Variant>>(
                "SELECT Name, AdapterRAM FROM Win32_VideoController",
            )
            .map_err(|e| anyhow::anyhow!("Win32_VideoController query failed: {e}"))?;

        let mut metrics = BTreeMap::new();
        let mut idx = 0usize;

        for row in &rows {
            let name = match row.get("Name") {
                Some(Variant::String(s)) => s.trim().to_string(),
                _ => continue,
            };

            // Skip virtual/remote display adapters
            if name.is_empty() || name.contains("Remote Display") || name.contains("Virtual") {
                continue;
            }

            // AdapterRAM is UINT32 in WMI (bytes, wraps at ~4 GB).
            // Convert to MiB to match other backends. Treat 0 as unknown.
            let vram_total = match row.get("AdapterRAM") {
                Some(Variant::UI4(bytes)) if *bytes > 0 => *bytes as u64 / (1024 * 1024),
                Some(Variant::UI8(bytes)) if *bytes > 0 => *bytes / (1024 * 1024),
                _ => 0,
            };

            metrics.insert(
                format!("GPU{idx} {name}"),
                GpuMetrics {
                    temp: 0.0,
                    load: 0,
                    power_consumption: 0.0,
                    power_limit: 0,
                    vram_used: 0,
                    vram_total,
                    sclk_mhz: 0,
                    mclk_mhz: 0,
                },
            );
            idx += 1;
        }

        Ok(metrics)
    }

    fn name(&self) -> &str {
        "wmi"
    }
}
