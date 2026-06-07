# Capabilities

The capabilities object describes what metrics and features are available for the current session. It is included in every WebSocket push (`capabilities` field) and is also available via `GET /api/capabilities`.

The `/api/capabilities` endpoint adds a `tray_mode` field not present in WebSocket messages.

---

## MetricsCapabilities Object

```json
{
  "inference":                    true,
  "system":                       true,
  "gpu":                          true,
  "cpu_temperature":              true,
  "memory":                       true,
  "host_metrics":                 true,
  "tray":                         true,
  "sensor_bridge_setup_available": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `inference` | `bool` | llama.cpp server is reachable and responding |
| `system` | `bool` | CPU/RAM system metrics are available |
| `gpu` | `bool` | GPU metrics are available |
| `cpu_temperature` | `bool` | CPU temperature sensor is readable |
| `memory` | `bool` | RAM metrics are available |
| `host_metrics` | `bool` | Any host-level metrics are available (true when system or gpu is true) |
| `tray` | `bool` | System tray icon is active |
| `sensor_bridge_setup_available` | `bool` | Windows-only: LibreHardwareMonitor sensor bridge can be installed |

---

## How Capabilities Are Determined

Capabilities are computed from the active session type and connection state:

### No active session
```
inference: true, system: false, gpu: false, host_metrics: false, tray: true
```
Inference is optimistically set to true; all host metrics are false until a session connects.

### Local Spawn session
```
inference: true, system: true, gpu: true, cpu_temperature: true, memory: true, host_metrics: true, tray: true
```
Full capabilities. The server is managed by llama-monitor on the local machine.

### Local Attach session (endpoint resolves to localhost)
```
inference: true, system: true, gpu: true, cpu_temperature: true, memory: true, host_metrics: true, tray: true
```
Full capabilities. Attaching to a local server still has access to local host metrics.

### Remote Attach session — no agent
```
inference: true, system: false, gpu: false, cpu_temperature: false, memory: false, host_metrics: false, tray: true
```
Inference-only. No host metrics because the server is on a different machine.

### Remote Attach session — agent connected
```
inference: true, system: true, gpu: true, cpu_temperature: true, memory: true, host_metrics: true, tray: true
```
Full capabilities. The remote agent running on the target machine provides host metrics over the agent connection.

---

## Endpoint and Session Kind

Included alongside capabilities in the WebSocket message and `/api/capabilities` response.

### `endpoint_kind`

| Value | Meaning |
|-------|---------|
| `"Local"` | Active endpoint resolves to localhost or was spawned locally |
| `"Remote"` | Active endpoint is on a different machine |
| `"Unknown"` | No active session or kind cannot be determined |

### `session_kind`

| Value | Meaning |
|-------|---------|
| `"Spawn"` | llama-monitor started the llama.cpp server process |
| `"Attach"` | llama-monitor attached to an already-running server |
| `"None"` | No active session |

### `tray_mode` (API-only, not in WebSocket)

| Value | Meaning |
|-------|---------|
| `"Desktop"` | Running in desktop environment with tray icon support |
| `"Headless"` | No graphical session; tray not available |
| `"Failed"` | Tray initialization failed |

In desktop mode, clicking the tray icon toggles the compact metrics popover. The popover closes
when its X button is clicked or when focus moves to another app or window.

---

## Status Pill Mapping

The top-nav status pill derives its label and color directly from capabilities:

| Condition | Text | Color |
|-----------|------|-------|
| `inference = false` | Error | Red |
| `inference = true`, `host_metrics = false` | Inference only | Yellow |
| `inference = true`, `host_metrics = true`, `system = false` or `gpu = false` | Limited | Yellow |
| `inference = true`, `host_metrics = true`, `system = true`, `gpu = true` | OK | Green (pulsing) |

The pill label is independent of `endpoint_kind` — a Remote session with the agent running shows green OK, the same as Local.

---

## Availability Reasons

Companion to capabilities, these explain *why* a metric is unavailable. Present in the WebSocket message under `availability` and in the `/api/capabilities` response.

```json
"availability": {
  "system":   "Available",
  "gpu":      "BackendUnavailable",
  "cpu_temp": "SensorUnavailable"
}
```

| Reason | Meaning |
|--------|---------|
| `Available` | Metric is live |
| `RemoteEndpoint` | Remote connection without agent; host metrics blocked |
| `NoDisplay` | Headless system; no graphical session |
| `TrayUnavailable` | Tray icon not supported on this build/platform |
| `SensorUnavailable` | Hardware sensor not present or not readable |
| `BackendUnavailable` | GPU backend (Apple/NVIDIA/ROCm) not detected |
| `CommandMissing` | Required CLI utility not installed (e.g. `nvidia-smi`) |
| `PermissionDenied` | Insufficient OS-level permissions |
| `MetricsUnreachable` | Metrics endpoint not responding |
| `NotApplicable` | Metric does not apply to this configuration |

---

## Capability Popover

Hovering the status pill opens a capability popover with per-feature rows. Each row shows a label, a status string, and a colored LED:

| Row | Live when |
|-----|-----------|
| Inference | `capabilities.inference` |
| Slots | Slot processing + idle count > 0 |
| Metrics | At least one throughput or token counter > 0 |
| Generation progress | `slot_generation_available` |
| Throughput | Same as Metrics |
| Context capacity | `context_capacity_tokens > 0` |
| Context usage | `context_capacity_tokens > 0` or `context_live_tokens_available` |
| Host metrics | `host_metrics_available` |
| Remote agent | `remote_agent_connected` |
