# Rapid-MLX Integration for llama-monitor

**Date**: 2026-06-11  
**Author**: Iris (research)  
**Status**: Proposal  
**Hardware Target**: M5 Max MacBook Pro (Apple Silicon)

## Overview

Rapid-MLX is the fastest local AI inference engine for Apple Silicon, running 2x faster than Ollama with full OpenAI API compatibility. This doc outlines the research findings and integration plan for adding Rapid-MLX monitoring support to llama-monitor.

## Current State

### llama-monitor (existing)
- Rust-based web dashboard for llama.cpp monitoring
- Polls llama.cpp endpoints: `/metrics`, `/slots`, `/health`, `/v1/models`
- GPU monitoring via `mactop` on Apple Silicon
- Supports llama.cpp presets with batch/ubatch size configuration

### Rapid-MLX
- Python-based OpenAI API server built on Apple's MLX framework
- Uses MLX native Metal inference (no GGUF quantization overhead)
- Supports Qwen3.5/3.6, Gemma 4, Llama 4, and other models
- Full prompt caching, DeltaNet state snapshots, DFlash speculative decoding

## Research Findings

### Performance Comparison (M3 Ultra 256GB, B=4 concurrent)

| Model | llama.cpp | Ollama (MLX) | Rapid-MLX | Improvement |
|---|---|---|---|---|
| Qwen3.6-35B-A3B | ~45 tok/s | 87 tok/s | 176 tok/s | **2.02x** |
| Qwen3.5-35B-A3B 8bit | ~50 tok/s | 87 tok/s | 151 tok/s | **1.74x** |
| Qwen3.5-27B | ~30 tok/s | 27 tok/s | 66 tok/s | **2.43x** |
| GPT-OSS 20B | ~60 tok/s | 97 tok/s | 221 tok/s | **2.29x** |

### Key MLX Advantages on Apple Silicon

1. **Native Metal acceleration** - No GGUF conversion overhead, direct Metal runtime
2. **Unified memory architecture** - MLX uses unified memory directly (no copy overhead)
3. **Lazy evaluation** - MLX uses compute graph optimization
4. **M5 Neural Accelerator support** - Hardware acceleration on M5 chips
5. **Speculative decoding** - DFlash and SuffixDecoding for additional speedups

### Rapid-MLX Monitoring Endpoints

Rapid-MLX does NOT have a native Prometheus endpoint. Instead, it uses:

| Endpoint | Description | Auth Required |
|---|---|---|
| `GET /health` | Health check | No |
| `GET /health/ready` | Readiness probe (503 until ready) | No |
| `GET /v1/status` | Full real-time status | Yes (if `--api-key` set) |
| `GET /v1/cache/stats` | Cache statistics | Yes |
| `POST /v1/requests/{id}/cancel` | Cancel running request | Yes |

### /v1/status Response Format

```json
{
  "status": "generating" | "idle",
  "model": "qwen3.6-35b-4bit",
  "uptime_s": 123.4,
  "steps_executed": 456,
  "num_running": 2,
  "num_waiting": 0,
  "total_requests_processed": 12,
  "total_prompt_tokens": 5000,
  "total_completion_tokens": 2000,
  "generation_tps": 93.0,
  "prompt_tps": 120.0,
  "metal": {
    "active_memory_gb": 20.5,
    "peak_memory_gb": 25.0,
    "cache_memory_gb": 2.0
  },
  "cache": {
    "enabled": true,
    "hit_rate": 0.85,
    "utilization": 0.75
  },
  "requests": [
    {
      "id": "req_123",
      "status": "running" | "waiting",
      "model": "qwen3.6-35b-4bit",
      "prompt_tokens": 100,
      "completion_tokens": 50,
      "started_at": 1234567890.0,
      "finished_at": null
    }
  ]
}
```

### Metal GPU Metrics (from mactop)

Already supported in llama-monitor via `mactop`:

| Metric | Type | Source |
|---|---|---|
| GPU temperature | °C | `mactop --headless` |
| GPU load | % | `mactop --headless` |
| GPU power | W | `mactop --headless` |
| GPU frequency | MHz | `mactop --headless` |
| VRAM used/total | MB | `mactop --headless` |
| Memory bandwidth | GB/s | `mactop --headless` |
| CPU cluster frequencies | MHz | `mactop --headless` |
| CPU cluster utilization | % | `mactop --headless` |

## Integration Plan

### Architecture

Add Rapid-MLX as a new backend type alongside existing llama.cpp support:

```
llama-monitor
├── llama backend (existing)
│   ├── /metrics polling (Prometheus format)
│   ├── /slots polling (JSON)
│   └── /health polling
└── mlx backend (new)
    ├── /v1/status polling (JSON)
    ├── /health polling
    └── /v1/cache/stats polling
```

### Implementation Steps

1. **Add MLX backend detection**
   - Detect if server is Rapid-MLX (check response format)
   - Or allow user to specify backend type in preset

2. **Write MLX metrics parser**
   - Parse `/v1/status` JSON response
   - Map to existing LlamaMetrics struct
   - Handle throughput calculation from `generation_tps`/`prompt_tps`

3. **Add MLX poller**
   - Similar to existing llama_poller.rs
   - Poll `/v1/status` instead of `/metrics`
   - Compute throughput from counters

4. **Update dashboard**
   - Show backend type indicator (llama.cpp vs MLX)
   - Display MLX-specific metrics (cache hit rate, Metal memory)
   - Maintain backward compatibility

5. **Add MLX preset support**
   - Allow Rapid-MLX model presets
   - Map Rapid-MLX aliases to model paths
   - Support MLX-specific options

### Code Structure

```rust
// src/llama/mlx_poller.rs
pub async fn mlx_metrics_poller(state: AppState, poll_interval: u64) {
    // Poll /v1/status instead of /metrics
    let status = client.get(format!("{base}/v1/status"))
        .send()
        .await?
        .json::<MlxStatus>()
        .await?;
    
    // Map to existing metrics
    let mut m = state.llama_metrics.lock().unwrap();
    m.generation_tokens_per_sec = status.generation_tps;
    m.prompt_tokens_per_sec = status.prompt_tps;
    m.prompt_tokens_total = status.total_prompt_tokens;
    m.generation_tokens_total = status.total_completion_tokens;
    m.metal_memory_active_gb = status.metal.active_memory_gb;
    // ... etc
}

// src/llama/metrics.rs
pub struct MlxStatus {
    pub status: String,
    pub model: String,
    pub uptime_s: f64,
    pub steps_executed: u64,
    pub num_running: u32,
    pub num_waiting: u32,
    pub total_requests_processed: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub generation_tps: f64,
    pub prompt_tps: f64,
    pub metal: MlxMetalMetrics,
    pub cache: MlxCacheMetrics,
    pub requests: Vec<MlxRequest>,
}

pub struct MlxMetalMetrics {
    pub active_memory_gb: f64,
    pub peak_memory_gb: f64,
    pub cache_memory_gb: f64,
}

pub struct MlxCacheMetrics {
    pub enabled: bool,
    pub hit_rate: f64,
    pub utilization: f64,
}
```

### Preset Format

```yaml
# llama-monitor preset (new MLX support)
backend: "mlx"  # or "llama.cpp" (default)
model_alias: "qwen3.6-35b-4bit"  # Rapid-MLX model alias
port: 8000
api_key: "not-needed"  # for auth

# MLX-specific options
mlx:
  prefill_step_size: 8192
  max_tokens: 32768
  enable_dflash: false
  kv_cache_quantization: false
  turboquant: false
```

### Dashboard Components

| Component | MLX Metrics | Source |
|---|---|---|
| Throughput tok/s | generation_tps, prompt_tps | `/v1/status` |
| Context pressure | N/A (no slots) | N/A |
| Request activity | num_running, num_waiting | `/v1/status` |
| GPU metrics | Metal memory | `/v1/status` + mactop |
| Cache stats | hit rate, utilization | `/v1/status` |
| Model info | model name, uptime | `/v1/status` |

## Comparison: llama.cpp vs MLX Monitoring

### llama.cpp endpoints
- `/metrics` - Prometheus format (text-based)
- `/slots` - JSON with per-slot state
- `/health` - JSON status
- `/v1/models` - Model metadata

### MLX endpoints
- `/v1/status` - JSON with full status
- `/health` - JSON status
- `/v1/cache/stats` - Cache details
- `/v1/models` - Model metadata (same as llama.cpp)

## Next Steps

1. [ ] Create MLX backend module in llama-monitor
2. [ ] Write MLX poller and metrics parser
3. [ ] Test with Rapid-MLX server running
4. [ ] Update dashboard to show MLX metrics
5. [ ] Add MLX preset support
6. [ ] Write tests
7. [ ] Document usage

## References

- Rapid-MLX GitHub: https://github.com/raullenchai/Rapid-MLX
- mlx-lm GitHub: https://github.com/ml-explore/mlx-lm
- llama-monitor GitHub: https://github.com/arte-fact/llama-monitor
- MLX docs: https://ml-explore.github.io/mlx/
