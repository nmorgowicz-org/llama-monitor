# WebSocket Schema

The WebSocket endpoint streams the full dashboard state to every connected client every 500 ms.

## Connection

```
ws://localhost:7778/ws
```

The server pushes messages on a fixed 500 ms interval. The client never sends messages; the connection is receive-only. When no active session exists the server waits silently until one is established before resuming pushes.

---

## Top-Level Message Shape

```json
{
  "llama":        { ...LlamaMetrics },
  "gpu":          { ...GpuMetrics },
  "system":       { ...SystemMetrics } | null,
  "logs":         ["string", ...],
  "server_running":            true | false,
  "local_server_running":      true | false,
  "session_mode":              "spawn" | "attach" | "",
  "active_session_id":         "session_1746...",
  "active_session_endpoint":   "http://127.0.0.1:8001",
  "local_metrics_available":   true | false,
  "host_metrics_available":    true | false,
  "remote_agent_connected":    true | false,
  "remote_agent_health_reachable": true | false,
  "remote_agent_url":          "http://...",
  "capabilities":              { ...MetricsCapabilities },
  "endpoint_kind":             "Local" | "Remote" | "Unknown",
  "session_kind":              "spawn" | "attach" | "none",  // lowercase from Serde #[serde(rename_all = "lowercase")]
  "availability": {
    "system":   "Available" | "RemoteEndpoint" | ...,
    "gpu":      "Available" | "BackendUnavailable" | ...,
    "cpu_temp": "Available" | "SensorUnavailable" | ...
  }
}
```

`system` is `null` when `host_metrics_available` is false.  
`gpu` is an empty object (`{}`) when `host_metrics_available` is false.

---

## LlamaMetrics (`llama`)

Populated by polling the llama.cpp server's `/metrics`, `/slots`, and `/v1/models` endpoints.

| Field | Type | Description |
|-------|------|-------------|
| `prompt_tokens_per_sec` | `f64` | Live prompt throughput (0 when idle) |
| `generation_tokens_per_sec` | `f64` | Live generation throughput (0 when idle) |
| `last_prompt_tokens_per_sec` | `f64` | Retained peak prompt throughput |
| `last_generation_tokens_per_sec` | `f64` | Retained peak generation throughput |
| `last_prompt_throughput_unix_ms` | `u64` | Epoch ms when last prompt rate was recorded |
| `last_generation_throughput_unix_ms` | `u64` | Epoch ms when last generation rate was recorded |
| `prompt_throughput_active` | `bool` | True when prompt rate > 0 this tick |
| `generation_throughput_active` | `bool` | True when generation rate > 0 this tick |
| `throughput_source` | `string` | `"interval_delta"` (always) |
| `prompt_tokens_total` | `u64` | Lifetime prompt token counter from Prometheus |
| `generation_tokens_total` | `u64` | Lifetime generation token counter from Prometheus |
| `context_capacity_tokens` | `u64` | Total KV cache capacity (sum of `n_ctx` across slots) |
| `context_live_tokens` | `u64` | Tokens currently in KV cache; 0 when unavailable |
| `context_live_tokens_available` | `bool` | True when server exposes per-slot token counts |
| `context_live_tokens_source` | `string` | Slot field used: `n_tokens`, `n_past`, `n_ctx_used`, or `n_cache_tokens` |
| `context_high_water_tokens` | `u64` | Peak context usage seen (from Prometheus `n_tokens_max`) |
| `slots_idle` | `u32` | Idle slot count |
| `slots_processing` | `u32` | Active slot count |
| `active_task_id` | `u64 \| null` | Task ID on the currently active slot |
| `last_task_id` | `u64 \| null` | Most recently seen task ID |
| `slot_generation_tokens` | `u64` | Tokens decoded in the active generation |
| `slot_generation_remaining` | `u64` | Tokens remaining in output budget |
| `slot_generation_limit` | `u64` | Total output budget (`n_predict` / `max_tokens`) |
| `slot_generation_active` | `bool` | True while generation is in progress |
| `slot_generation_available` | `bool` | True when generation progress data is exposed |
| `requests_processing` | `u32` | In-flight requests (from Prometheus) |
| `status` | `string` | Currently unused; reserved |
| `model_name` | `string` | Model ID from `/v1/models` |
| `model_params` | `u64 \| null` | Parameter count from model metadata |
| `model_ctx_train` | `u64 \| null` | Training context length from model metadata |
| `slots` | `SlotSnapshot[]` | Per-slot detail (see below) |

### SlotSnapshot

One entry per slot returned by `/slots`.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u64 \| null` | Slot index |
| `n_ctx` | `u64` | Context window size for this slot |
| `is_processing` | `bool` | Whether the slot is currently active |
| `id_task` | `u64 \| null` | Task ID assigned to this slot |
| `output_tokens` | `u64` | Tokens generated so far |
| `output_remaining` | `u64` | Tokens remaining in budget |
| `output_limit` | `u64` | Total output budget |
| `output_active` | `bool` | Generation in progress on this slot |
| `output_available` | `bool` | True when `next_token` progress data present |
| `context_live_tokens` | `u64 \| null` | Current KV cache tokens for this slot |
| `context_live_tokens_source` | `string \| null` | Source field name (see `context_live_tokens_source` above) |
| `speculative_enabled` | `bool` | Whether speculative decoding is active |
| `speculative_type` | `string \| null` | e.g. `"ngram_map_k"` |
| `speculative_config` | `{label, value}[]` | Speculative decoding parameters |
| `sampler_stack` | `string[]` | Active sampler names in order |
| `sampler_config` | `{label, value}[]` | Key sampler parameter values |

---

## GpuMetrics (`gpu`)

Empty object when `host_metrics_available` is false.

| Field | Type | Description |
|-------|------|-------------|
| `temp` | `f32` | GPU temperature °C |
| `load` | `u32` | GPU utilization % |
| `power_consumption` | `f32` | Current power draw (W) |
| `power_limit` | `u32` | TDP / power limit (W) |
| `vram_used` | `u64` | VRAM used (bytes) |
| `vram_total` | `u64` | VRAM total (bytes) |
| `sclk_mhz` | `u32` | Shader clock MHz |
| `mclk_mhz` | `u32` | Memory clock MHz |

`gpu` is a `BTreeMap<String, GpuMetrics>` keyed by device name (e.g. `"Apple M3 Max"`). Most setups have a single key.

---

## SystemMetrics (`system`)

`null` when `host_metrics_available` is false.

| Field | Type | Description |
|-------|------|-------------|
| `cpu_name` | `string` | CPU model string |
| `cpu_temp` | `f32` | CPU temperature °C |
| `cpu_temp_available` | `bool` | False on platforms without sensor access |
| `cpu_load` | `u32` | CPU utilization % |
| `cpu_clock_mhz` | `u32` | Current CPU clock MHz |
| `ram_total_gb` | `f64` | Total RAM (GB) |
| `ram_used_gb` | `f64` | RAM in use (GB) |
| `motherboard` | `string` | Board name (empty string when unavailable) |

---

## Availability Reasons

Used in `availability.system`, `availability.gpu`, and `availability.cpu_temp`.

| Value | Meaning |
|-------|---------|
| `"Available"` | Metric is live and working |
| `"RemoteEndpoint"` | No host metrics over remote connection without agent |
| `"NoDisplay"` | No graphical session (headless system) |
| `"TrayUnavailable"` | Tray not supported on this build |
| `"SensorUnavailable` | Hardware sensor not present |
| `"BackendUnavailable"` | GPU backend not detected |
| `"CommandMissing` | Required system utility not installed |
| `"PermissionDenied"` | Insufficient OS permissions |
| `"MetricsUnreachable` | Metrics endpoint not responding |
| `"NotApplicable"` | Metric does not apply in this configuration |

**Note:** The enum values are PascalCase strings (e.g., `"Available"` not `"available"`).

---

## Notes

- **Push interval:** 500 ms fixed. There is no on-demand request mechanism.
- **Host metrics gating:** `gpu` and `system` are only populated when `host_metrics_available` is `true`. This is true for local spawn/attach sessions and remote sessions where the remote agent is connected.
- **Context live tokens:** `context_live_tokens_available` is only `true` when the llama.cpp server exposes per-slot token counts (`n_tokens`, `n_past`, `n_ctx_used`, or `n_cache_tokens` in the `/slots` response). Many servers expose `n_ctx` (capacity) but not current usage.
- **kv_cache_* fields:** Internal fields on the Rust side. They are NOT serialized into the WebSocket message; only the `context_*` equivalents are.
