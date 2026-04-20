# Remote Metrics Agent API Specification

**Date:** 2026-04-20  
**Status:** Draft - Planning phase  
**Related:** docs/20260419-ui-ux-and-monitoring-roadmap.md

---

## Overview

This document defines the API specification for a lightweight remote metrics collection agent. The agent runs on remote hosts and exposes system/GPU metrics that cannot be obtained from the llama.cpp server endpoint alone.

### Architecture

```
llama-monitor desktop/web UI
    |
    |-- llama.cpp metrics endpoint (/health, /metrics, /slots)
    |
    `-- llama-monitor-agent on remote host
          |-- GET /metrics/system
          |-- GET /metrics/gpu
          |-- GET /metrics/temperatures
          `-- GET /metrics/processes (optional)
```

### Security First

- Authentication is **required** (token-based)
- No metrics exposed without explicit user configuration
- Agent should only listen on localhost by default
- TLS is recommended for remote deployments

---

## Core Metrics Endpoints

### 1. System Metrics

**Endpoint:** `GET /metrics/system`  
**Response:** JSON

```json
{
  "timestamp": "2026-04-20T12:34:56Z",
  "cpu": {
    "load_percent": 45.2,
    "cores": 8,
    "usage_per_core": [42.1, 51.3, 38.7, 47.9, 44.2, 49.6, 43.8, 46.1]
  },
  "memory": {
    "total_gb": 64.0,
    "used_gb": 21.5,
    "free_gb": 42.5,
    "swap_used_gb": 2.1
  },
  "uptime_seconds": 86400,
  "hostname": "gpu-server-01"
}
```

**Schema:**

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | string (ISO 8601) | When metrics were collected |
| `cpu.load_percent` | float | Overall CPU utilization |
| `cpu.cores` | int | Number of CPU cores |
| `cpu.usage_per_core` | array[float] | Per-core utilization (0-100) |
| `memory.total_gb` | float | Total system RAM in GB |
| `memory.used_gb` | float | Used RAM in GB |
| `memory.free_gb` | float | Free RAM in GB |
| `memory.swap_used_gb` | float | Swap space used in GB |
| `uptime_seconds` | int | System uptime in seconds |
| `hostname` | string | Remote host identifier |

---

### 2. GPU Metrics

**Endpoint:** `GET /metrics/gpu`  
**Response:** JSON

```json
{
  "timestamp": "2026-04-20T12:34:56Z",
  "gpus": [
    {
      "index": 0,
      "name": "NVIDIA A100-SXM4-80GB",
      "pci_bus_id": "0000:00:05.0",
      "temperature_celsius": 68,
      "load_percent": 71,
      "memory": {
        "total_gb": 80.0,
        "used_gb": 18.4,
        "free_gb": 61.6
      },
      "power_watts": 180.5,
      "clocks": {
        "graphics_mhz": 1200,
        "sm_mhz": 1410,
        "memory_mhz": 1215
      }
    }
  ]
}
```

**Schema:**

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | string (ISO 8601) | When metrics were collected |
| `gpus` | array[object] | List of GPU devices |
| `gpus[].index` | int | Device index (0-based) |
| `gpus[].name` | string | GPU model name |
| `gpus[].pci_bus_id` | string | PCI bus identifier |
| `gpus[].temperature_celsius` | int | GPU temperature |
| `gpus[].load_percent` | int | GPU compute utilization (0-100) |
| `gpus[].memory.total_gb` | float | Total GPU VRAM in GB |
| `gpus[].memory.used_gb` | float | Used VRAM in GB |
| `gpus[].memory.free_gb` | float | Free VRAM in GB |
| `gpus[].power_watts` | float | Current power draw |
| `gpus[].clocks.graphics_mhz` | int | Graphics clock speed |
| `gpus[].clocks.sm_mhz` | int | Streaming multiprocessor clock |
| `gpus[].clocks.memory_mhz` | int | Memory clock speed |

---

### 3. Temperature Metrics

**Endpoint:** `GET /metrics/temperatures`  
**Response:** JSON

```json
{
  "timestamp": "2026-04-20T12:34:56Z",
  "cpu_package_celsius": 62,
  "cpu_cores_max_celsius": 68,
  "gpu_celsius": 68,
  "ambient_celsius": 24,
  "board_celsius": 45
}
```

**Schema:**

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | string (ISO 8601) | When metrics were collected |
| `cpu_package_celsius` | int | CPU package/core temperature |
| `cpu_cores_max_celsius` | int | Maximum core temperature |
| `gpu_celsius` | int | GPU temperature |
| `ambient_celsius` | int | Ambient room temperature |
| `board_celsius` | int | Motherboard temperature |

---

## Extended Metrics (Optional)

### 4. Process Metrics

**Endpoint:** `GET /metrics/processes`  
**Response:** JSON

```json
{
  "timestamp": "2026-04-20T12:34:56Z",
  "processes": [
    {
      "pid": 12345,
      "name": "llama-server",
      "cpu_percent": 45.2,
      "memory_percent": 28.5,
      "gpu_memory_gb": 18.4,
      "uptime_seconds": 3600
    }
  ]
}
```

**Schema:**

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | string (ISO 8601) | When metrics were collected |
| `processes` | array[object] | List of monitored processes |
| `processes[].pid` | int | Process ID |
| `processes[].name` | string | Process name |
| `processes[].cpu_percent` | float | CPU utilization |
| `processes[].memory_percent` | float | Memory utilization |
| `processes[].gpu_memory_gb` | float | GPU VRAM usage |
| `processes[].uptime_seconds` | int | Process uptime |

---

## Authentication

All endpoints require authentication via Bearer token:

```http
GET /metrics/system
Authorization: Bearer <token>
```

### Token Management

- Tokens are generated by the main llama-monitor app
- Tokens are configured in the web UI (Settings → Remote Agent)
- Tokens have configurable expiration
- Revoked tokens immediately stop access

---

## Agent Deployment

### Default Configuration

```yaml
# Default agent configuration (generated on first run)
listen_address: "127.0.0.1"
listen_port: 7779
authentication:
  enabled: true
  token: "<auto-generated>"
  expires_at: "2026-05-20T00:00:00Z"
metrics:
  system: enabled
  gpu: enabled
  temperatures: enabled
  processes: disabled
```

### Command Line Flags

```bash
llama-monitor-agent \
  --listen-address 0.0.0.0 \
  --listen-port 7779 \
  --token-file ~/.config/llama-monitor-agent/token \
  --gpu-backend auto
```

---

## Integration with Main App

### Agent Connection Flow

```
1. User clicks "Connect Remote Agent" in Settings
2. User enters agent URL and authentication token
3. Main app tests /metrics/system endpoint
4. If successful, agent is saved to ~/.config/llama-monitor/agents.json
5. Main app periodically polls agent endpoints (every 30s)
6. Agent metrics are displayed with "Remote Agent" source label
```

### UI Affordance

- Add "Remote Agent" toggle in Settings
- Show connection status (● Connected / ○ Disconnected)
- Display latency metric (e.g., "Remote Agent: 12ms")
- Optional: Auto-connect to saved agents on startup

---

## Implementation Notes

### Platform Support

| Platform | Dependencies |
|----------|-------------|
| Linux | sysinfo, nvidia-smi/rocm-smi (optional) |
| macOS | sysinfo, mactop |
| Windows | sysinfo, nvidia-smi (optional) |

### Error Handling

All endpoints return consistent error format:

```json
{
  "error": {
    "code": "METRICS_UNAVAILABLE",
    "message": "GPU metrics not available - no supported GPU detected",
    "timestamp": "2026-04-20T12:34:56Z"
  }
}
```

---

## Future Enhancements

1. **Metrics aggregation** - Support multiple agents and aggregate metrics
2. **Alerting** - Threshold-based alerts via agent
3. **Process monitoring** - Detect llama.cpp process health
4. **Log streaming** - Forward llama-server logs from remote host
5. **Metric history** - Store historical metrics for trend analysis

---

**Document version:** 0.1  
**Last updated:** 2026-04-20  
**Status:** Draft - Implementation deferred to future phase
