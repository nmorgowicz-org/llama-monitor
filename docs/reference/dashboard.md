# Dashboard Capabilities & Metrics

Llama Monitor provides real-time visibility into your inference stack through a nav cockpit, inference metrics cards, and GPU/system hardware monitoring.

## Nav Cockpit

The nav cockpit replaces the old search bar with a live metrics strip visible on every tab:

| Chip | Description |
|------|-------------|
| **State** | Current server state: idle, attach, prompting, generating |
| **Throughput** | Prompt processing speed (P) and generation speed (G) in tokens/sec |
| **Context** | Highest context pressure percentage across all active chat tabs |
| **GPU** | Temperature of the hottest GPU in the system |
| **Sparkline** | Mini throughput chart showing recent generation speed over time |

Click the cockpit to navigate to the Server tab for full metrics. On narrow viewports, GPU and sparkline hide at 980px and context hides at 820px.

## Inference Metrics

The Server tab displays inference metrics sourced from llama.cpp's Prometheus `/metrics` endpoint and `/slots` JSON:

### Throughput Card
- Prompt processing speed (tokens/sec) with live sparkline
- Generation speed (tokens/sec) with live sparkline
- Prompt/generation ratio bar showing relative speeds
- Activity timeline showing recent task history

### Generation Card
- Current output token count and remaining context
- Prompt ingest and output generation progress stages
- Live generation estimate based on recent throughput samples
- Task metadata: task ID, max context, tokens remaining

### Context Window Card
Two views toggle between gauge and fleet:

**Gauge view** — Single large ring showing context pressure for the most loaded chat tab.

**Fleet view** — Per-chat context usage bars showing:
- Tab name and context percentage
- Stale chat indicators (tabs with no recent activity)
- Total context window size from the active server
- Falls back to chat-derived estimates when the server doesn't expose per-slot token counts

### Slot Activity Card
Per-slot status showing:
- Slot state: idle, loading, processing
- Current task output token count
- Context tokens in use
- Slot utilization sparkline

### Request Activity Card
Recent task history with:
- Request count and average duration
- Completion markers for finished tasks
- Activity timeline with 5-minute rolling window

### Model & Decoding Card
- Model name, parameter count, and quantization
- Speculative decoding state (ngram-mod, draft model)
- Sampler chain configuration

## GPU & System Metrics

Local sessions show real-time hardware monitoring. Remote sessions require a remote agent.

### GPU Metrics
| Metric | Source |
|--------|--------|
| GPU utilization (%) | `rocm-smi`, `nvidia-smi`, `mactop` |
| Power draw (W) | `rocm-smi`, `nvidia-smi` |
| VRAM usage (GB) | `rocm-smi`, `nvidia-smi`, `mactop` |
| Core clock (GHz) | `rocm-smi`, `nvidia-smi` |
| Memory clock (GHz) | `rocm-smi`, `nvidia-smi` |
| Temperature (°C) | `rocm-smi`, `nvidia-smi`, `mactop` |

Each metric displays a live sparkline, current value, and peak indicator.

### System Metrics
| Metric | Source |
|--------|--------|
| CPU model and load (%) | sysinfo crate |
| CPU temperature (°C) | Linux thermal zones, `mactop`, or `sensor_bridge.exe` on Windows |
| CPU clock speed (GHz) | Linux `/proc/cpuinfo`, `mactop` |
| RAM usage (GB) | sysinfo crate |
| Motherboard model | Linux `dmidecode`, Apple system profiler |

## Capability-Aware UI

The top nav status pill reflects the live telemetry level:

| State | Color | Meaning |
|-------|-------|---------|
| **Full telemetry** | Green | All metrics available (local session or remote with agent) |
| **Inference only** | Yellow | Only llama.cpp metrics (remote attach without agent) |
| **Limited** | Orange | Partial metrics (agent connected but some sensors unavailable) |
| **Error** | Red | Connection lost or metrics endpoint unreachable |

## Refresh Rate

Dashboard WebSocket refresh rate is configurable from 200ms to 10s via Settings > Performance. The default is 500ms. Network quality detection can auto-adjust the interval based on observed connection conditions.
