# Backend-Neutral Inference

> **Status: Shipped.** Phase 3-4 complete. Backend-neutral orchestration layer, Rapid-MLX chat routing, cancellation, and discovery/launch logic.

Llama Monitor uses a backend-neutral inference abstraction that allows launching and managing both llama.cpp and Rapid-MLX servers through a common interface. This enables a unified dashboard, consistent preset management, and engine-aware spawn wizard.

## InferenceBackend enum

The `InferenceBackend` enum (`src/inference/mod.rs`) defines supported backends:

```rust
pub enum InferenceBackend {
    LlamaCpp,
    RapidMlx,
}
```

## Orchestration layer

The orchestration layer (`src/inference/`) provides:

### BackendObserver trait

Defines callbacks for backend events:

| Method | Description |
|--------|-------------|
| `on_log_line(&self, line: &str)` | Called for each log line from the backend |
| `on_crash(&self, status: ExitStatus, tail: Vec<String>)` | Called when the backend crashes |

### SupervisedLaunch struct

Represents a launch configuration:

| Field | Description |
|-------|-------------|
| `cmd` | The command to run |
| `env` | Environment variables |
| `redacted_summary` | Human-readable summary for UI |

### Supervisor struct

Manages the lifecycle of a backend server:

| Method | Description |
|--------|-------------|
| `start()` | Launch the server |
| `stop()` | Stop the server gracefully |
| `kill()` | Force-kill the server |

## llama.cpp adapter

The llama.cpp adapter (`src/inference/llama_cpp.rs`) implements the backend traits for llama.cpp:

- `build_launch()` — Constructs the `llama-server` command
- `await_ready()` — Waits for `/health` to return 200
- `cancel_request()` — Sends a cancel request to the running server
- `poll_metrics()` — Polls `/metrics` for Prometheus data

### Key settings

| Setting | Flag | Description |
|---------|------|-------------|
| GPU layers | `-ngl` | Number of layers to offload to GPU |
| KV cache type | `-ctk`, `-ctv` | KV cache quantization type |
| MoE offload | `-n-cpu-moe` | Number of MoE layers to offload to CPU |
| mlock | `-mlock` | Lock model in memory |
| Threads | `-t`, `-tb` | Number of threads |
| Speculative decoding | `-s`, `-c` | Speculative decoding parameters |
| mmproj | `--mmproj` | Multimodal projector |
| Flash attention | `--flash-attn` | Enable Flash Attention |
| Context size | `-c` | Context size |
| Batch size | `-b` | Batch size |
| Ubatch | `-ub` | Micro-batch size |

## Rapid-MLX adapter

The Rapid-MLX adapter (`src/inference/rapid_mlx/mod.rs`) implements the backend traits for Rapid-MLX:

- `build_launch()` — Constructs the Rapid-MLX serve command
- `await_ready()` — Waits for `/health/ready` to return 200
- `cancel_request()` — Sends a cancel request to the running server
- `poll_metrics()` — Polls `/v1/status` and `/v1/cache/stats`

### Key settings

| Setting | Flag | Description |
|---------|------|-------------|
| Host | `--host` | Bind address |
| Port | `--port` | Bind port |
| Served model name | `--served-model-name` | Model name in API responses |
| Log level | `--log-level` | Logging verbosity |
| Timeout | `--timeout` | Server timeout |
| KV cache dtype | `--kv-cache-dtype` | int4/int8/bf16 |
| Prefix cache | `--enable-prefix-cache` / `--disable-prefix-cache` | Enable/disable prefix cache |
| Retained cache | `--cache-memory-mb` | Retained cache size |
| Hybrid mode | `--force-hybrid` / `--no-hybrid` | Hybrid DeltaNet control |
| Tool-call parser | `--tool-call-parser` | Tool call parser override |
| Reasoning parser | `--reasoning-parser` | Reasoning parser override |
| Enable auto tool choice | `--enable-auto-tool-choice` | Enable auto tool choice |
| No thinking | `--no-thinking` | Disable thinking |
| Reasoning | `--reasoning` | Enable reasoning mode |
| Prefill step size | `--prefill-step-size` | Prefill step size |
| TurboQuant | `--kv-cache-turboquant` | TurboQuant mode |

## Chat routing

Chat requests are routed to the appropriate backend based on the session's engine setting:

- **llama.cpp**: Requests go to the llama-server API (`/v1/chat/completions`)
- **Rapid-MLX**: Requests go to the Rapid-MLX API (`/v1/chat/completions`)

Both backends expose the same OpenAI-compatible API, so the chat interface doesn't need backend-specific code.

## Cancellation

Cancellation is backend-specific:

- **llama.cpp**: Sends a POST to `/completion` with `cancel: true`
- **Rapid-MLX**: Sends a POST to `/v1/cancel` with the request ID

The cancel request is capability-gated: `capabilities.cancellation` must be true.

## Telemetry

Both backends emit `InferenceMetricsSnapshot` objects:

- `HealthState` — `Ok`, `NotLoaded`, `Degraded`
- `generation_tokens_per_second` — Throughput
- `prompt_tokens_per_second` — Prompt throughput
- `active_memory_bytes` — Memory usage
- `peak_memory_bytes` — Peak memory usage
- `cache_hit_rate` — Cache hit rate

The dashboard renders different cards based on the backend type.
