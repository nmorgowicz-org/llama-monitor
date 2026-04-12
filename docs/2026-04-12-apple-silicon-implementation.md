# Apple Silicon Backend Implementation

**Date:** 2026-04-12  
**Status:** Implementation complete, ready for testing on Apple Silicon Mac

## Summary

Implemented Apple Silicon GPU monitoring support for llama-monitor using `mactop` as the data source.

## Files Created/Modified

### New Files
1. **`src/gpu/apple.rs`** - Apple Silicon backend implementation
2. **`docs/2026-04-12-mactop-json-schema.md`** - JSON schema analysis and test results
3. **`docs/2026-04-12-mactop-test-script.sh`** - Test script for verifying mactop output
4. **`docs/2026-04-12-apple-silicon-implementation.md`** - This file

### Modified Files
1. **`src/gpu/mod.rs`** - Added apple module and Apple Silicon detection

## Implementation Details

### Backend: `src/gpu/apple.rs`

The Apple backend uses `mactop` to gather metrics:

```bash
mactop --headless --count 1 --format json
```

#### JSON Field Mappings

| llama-monitor Field | mactop Field | Notes |
|---------------------|--------------|-------|
| `temp` | `soc_metrics.gpu_temp` | GPU temperature in °C |
| `load` | `soc_metrics.gpu_active` | GPU utilization % |
| `power_consumption` | `soc_metrics.gpu_power` | GPU power in Watts |
| `power_limit` | N/A | Set to 0 (not available) |
| `vram_used` | `memory.used / (1024*1024)` | Unified memory used in MB |
| `vram_total` | `memory.total / (1024*1024)` | Total unified memory in MB |
| `sclk_mhz` | `soc_metrics.gpu_freq_mhz` | GPU clock in MHz |
| `mclk_mhz` | `(dram_read_bw + dram_write_bw) * 1000 / 16` | Estimated from DRAM bandwidth |

#### Unified Memory Handling

Apple Silicon uses unified memory (no separate VRAM). The `memory.used` and `memory.total` fields from mactop represent the total system memory pool that the GPU can access.

- **Memory is in bytes**, converted to MB for `GpuMetrics`
- Example: 32GB Mac = 34359738368 bytes = 32768 MB

#### Memory Clock Estimation

Since mactop doesn't expose raw memory clock, we estimate it from DRAM bandwidth:
```
MCLK (MHz) ≈ (dram_read_bw_gbs + dram_write_bw_gbs) * 1000 / 16
```

This is an approximation based on DDR memory bandwidth calculations.

### Detection: `src/gpu/mod.rs`

Added `is_apple_silicon()` function that:
1. Checks `sysctl -n machdep.cpu.brand_string` for "Apple" or "M" prefix
2. Falls back to checking if `mactop` is available

```rust
fn is_apple_silicon() -> bool {
    // Check CPU brand string for Apple Silicon
    // Fallback: check if mactop is available
}
```

### User Interface

Users can specify Apple Silicon backend via:

```bash
# Auto-detect (preferred)
llama-monitor

# Force Apple backend
llama-monitor --gpu-backend apple

# Force none (disable GPU monitoring)
llama-monitor --gpu-backend none
```

## Testing

### Prerequisites

1. Install mactop on macOS:
```bash
brew install mactop
```

2. Verify mactop works:
```bash
mactop --headless --count 1
mactop --headless --count 1 --format json | jq .
```

### Test Commands

```bash
# In llama-monitor directory
cd llama-monitor

# Build
cargo build --release

# Test with Apple backend
./target/release/llama-monitor --gpu-backend apple

# Check metrics output
./target/release/llama-monitor --gpu-backend apple --metrics-only
```

### Expected Output

```json
{
  "GPU0 Apple M1 Pro": {
    "temp": 64.0,
    "load": 37,
    "power_consumption": 0.96,
    "power_limit": 0,
    "vram_used": 16734,
    "vram_total": 32768,
    "sclk_mhz": 601,
    "mclk_mhz": 3943
  }
}
```

## Known Limitations

1. **Power Limit Not Available**: mactop doesn't expose power limits, so `power_limit` is set to 0.

2. **Memory Clock Estimated**: The `mclk_mhz` is estimated from DRAM bandwidth rather than directly read.

3. **Single GPU**: Currently assumes single GPU system (keyed as "GPU0 Apple M1 Pro"). Can be extended for multi-GPU if needed.

4. **M1/M2/M3 Specific**: Currently hardcoded for "Apple M1 Pro". Should work for any Apple Silicon but label may be inaccurate.

## Future Enhancements

1. **Dynamic GPU Name**: Read actual chip name from `system_info.name` field in mactop output.

2. **Per-Core Metrics**: Add E-core/P-core/S-core breakdown metrics.

3. **Fan Control**: Use `--fan-control` flag for interactive fan management.

4. **Temperature Groups**: Add more detailed temperature metrics from `temperatures` array.

## Dependencies

No new dependencies required. Uses:
- `anyhow` - Error handling (already in Cargo.toml)
- `serde` - JSON deserialization (already in Cargo.toml)
- `serde_json` - JSON parsing (already in Cargo.toml)
- `mactop` - External CLI tool (user must install via Homebrew)

## User Documentation

### Prerequisites

To use Apple Silicon monitoring:

1. **macOS with Apple Silicon** (M1, M2, M3, or later)
2. **Homebrew** installed
3. **mactop** installed:
   ```bash
   brew install mactop
   ```

### Usage

```bash
# Auto-detect and use Apple Silicon backend
llama-monitor

# Force Apple Silicon backend
llama-monitor --gpu-backend apple

# Disable GPU monitoring
llama-monitor --gpu-backend none
```

### Troubleshooting

**Issue**: "mactop failed: command not found"

**Solution**: Install mactop via Homebrew:
```bash
brew install mactop
```

**Issue**: Incorrect VRAM values

**Solution**: VRAM is unified memory on Apple Silicon. Values may differ from NVIDIA/AMD cards where VRAM is dedicated.

**Issue**: `power_limit` is 0

**Solution**: This is expected - mactop doesn't expose power limits on Apple Silicon.

## Acknowledgments

- **mactop** by Carsen Klock - https://github.com/metaspartan/mactop
- **macmon** by vladkens - https://github.com/vladkens/macmon (initial research)
