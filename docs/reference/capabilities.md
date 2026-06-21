# Capabilities

The capabilities object describes what metrics and features are available for the current session. It is included in WebSocket dashboard pushes (`capabilities` field) and is also available via `GET /api/capabilities` (in `src/web/api/sessions.rs`).

Authentication:
- `GET /api/capabilities` requires an `Authorization: Bearer <api-token>` header; without it, the endpoint returns 401.

Response envelope:
- Both the WebSocket push and `/api/capabilities` return the same core fields:
  - `capabilities` — the MetricsCapabilities object (see below)
  - `endpoint_kind` — Local / Remote / Unknown
  - `session_kind` — Spawn / Attach / None
  - `availability` — per-metric availability reasons
- `/api/capabilities` additionally returns:
  - `tray_mode` — Desktop / Headless / Failed (not included in WebSocket messages).

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
| `inference` | `bool` | True when an inference session is active or when no session exists (optimistic); false when a configured session cannot be reached |
| `system` | `bool` | CPU/RAM system metrics are available (includes macOS memory-pressure telemetry) |
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
`inference` is hard-coded to true when no active session exists (optimistic assumption you may start or attach to a server). All host metrics are false; they become true when a local session connects or when a remote agent is connected.

### Local Launch session
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
Basic. No host metrics because the server is on a different machine.

### Remote Attach session — agent connected
```
inference: true, system: true, gpu: true, cpu_temperature: true, memory: true, host_metrics: true, tray: true
```
Full capabilities. This is gated by `remote_agent_connected()` — the app periodically calls the agent’s `/metrics` endpoint; when it responds, all host-related flags are set true.

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
| `inference = true`, `host_metrics = false` | Basic | Yellow |
| `inference = true`, `host_metrics = true`, `system = false` or `gpu = false` | Partial | Yellow |
| `inference = true`, `host_metrics = true`, `system = true`, `gpu = true` | Full | Green (pulsing) |

The pill label is independent of `endpoint_kind` — a Remote session with the agent running shows green Full, the same as Local.

---

## Availability Reasons

Companion to capabilities, these explain *why* a metric is unavailable. Present in WebSocket pushes under `availability` and in the `/api/capabilities` response.

The `availability` object has three keys:
- `system`
- `gpu`
- `cpu_temp`

The runtime function `calculate_availability_reasons()` currently emits only two values:

```json
"availability": {
  "system":   "Available",
  "gpu":      "Available",
  "cpu_temp": "RemoteEndpoint"
}
```

| Reason | Meaning |
|--------|---------|
| `Available` | Metric is live |
| `RemoteEndpoint` | Remote connection without agent; host metrics blocked |

The `AvailabilityReason` enum also defines a broader set of variants that may be surfaced in the future or in other contexts:
`NoDisplay`, `TrayUnavailable`, `SensorUnavailable`, `BackendUnavailable`,
`CommandMissing`, `PermissionDenied`, `MetricsUnreachable`, `NotApplicable`.

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
