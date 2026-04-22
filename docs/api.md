# Llama Monitor API Documentation

## Overview

Llama Monitor provides a RESTful API for retrieving metrics data and system information. This document describes all available endpoints.

## Base URL

```
http://localhost:7778
```

## Endpoints

### GET `/api/metrics`

Retrieve all current metrics.

**Response:**
```json
{
  "inference": {
    "tokens_per_second": 42.5,
    "prompt_tokens": 1024,
    "generated_tokens": 256,
    "context_usage": 1280,
    "context_window": 4096,
    "gpu_memory_used": 2048,
    "gpu_memory_total": 8192
  },
  "system": {
    "cpu_usage": 45.2,
    "memory_used": 4096,
    "memory_total": 16384
  },
  "gpu": {
    "gpu_usage": 78.5,
    "gpu_temperature": 72,
    "gpu_power": 125.0
  },
  "cpu_temperature": 65.0,
  "timestamp": "2026-04-20T16:00:00Z"
}
```

---

### GET `/api/capabilities`

Retrieve system capabilities and availability reasons.

**Response:**
```json
{
  "capabilities": {
    "inference": true,
    "system": true,
    "gpu": true,
    "cpu_temperature": true,
    "memory": true,
    "host_metrics": true,
    "tray": true
  },
  "endpoint_kind": "local",
  "session_kind": "spawn",
  "availability": {
    "system_reason": "available",
    "gpu_reason": "available",
    "cpu_temp_reason": "available"
  }
}
```

**Endpoint Kinds:**
- `local` - Running on same machine as server
- `remote` - Connected to remote llama.cpp server
- `unknown` - Unknown or undetermined

**Session Kinds:**
- `spawn` - Server spawned by llama-monitor (full metrics available)
- `attach` - Attached to existing server (inference only)
- `none` - No active session

**Availability Reasons:**
- `available` - Metric is available and working
- `remote_endpoint` - Remote connection limits metrics
- `no_display` - No graphical session (headless)
- `tray_unavailable` - Tray not supported
- `sensor_unavailable` - Hardware sensor missing
- `backend_unavailable` - GPU backend not detected
- `command_missing` - Required utility not installed
- `permission_denied` - Insufficient permissions
- `metrics_unreachable` - Metrics server not responding

---

### GET `/api/sessions`

Retrieve active sessions.

**Response:**
```json
{
  "sessions": [
    {
      "id": "session-1",
      "mode": "spawn",
      "endpoint": "http://localhost:8080",
      "model": "llama-2-7b.Q4_K_M.gguf",
      "spawn_options": {
        "n_gpu_layers": 32,
        "n_threads": 8,
        "n_ctx": 2048
      }
    }
  ]
}
```

---

### GET `/api/presets`

Retrieve saved model presets.

**Response:**
```json
{
  "presets": [
    {
      "id": "default",
      "name": "Default Settings",
      "model_path": "/models/llama-2-7b.Q4_K_M.gguf",
      "n_gpu_layers": 32,
      "n_threads": 8,
      "n_ctx": 2048,
      "n_batch": 512,
      "n_parallel": 1
    }
  ]
}
```

---

### GET `/api/state`

Retrieve current application state.

**Response:**
```json
{
  "server_port": 7778,
  "ui_port": 8080,
  "gpu_backend": "vulkan",
  "gpu_device_index": 0,
  "tray_enabled": true,
  "headless": false
}
```

---

### POST `/api/sessions`

Create a new session.

**Request:**
```json
{
  "mode": "spawn",
  "model_preset": "default",
  "server_port": 8080,
  "extra_args": []
}
```

**Response:**
```json
{
  "session_id": "session-1",
  "endpoint": "http://localhost:8080",
  "status": "starting"
}
```

---

### POST `/api/sessions/attach`

Attach to an existing llama.cpp server.

**Request:**
```json
{
  "endpoint": "http://remote-server:8080",
  "session_name": "Remote Server"
}
```

**Response:**
```json
{
  "session_id": "session-2",
  "endpoint": "http://remote-server:8080",
  "status": "attached"
}
```

---

### DELETE `/api/sessions/{id}`

Stop a session.

**Response:**
```json
{
  "session_id": "session-1",
  "status": "stopped"
}
```

---

## Websocket API

### `/ws/metrics`

Real-time metrics streaming.

**Connection:**
```
ws://localhost:7778/ws/metrics
```

**Messages:**
```json
{
  "type": "metrics",
  "data": {
    "inference": {...},
    "system": {...},
    "gpu": {...},
    "cpu_temperature": 65.0,
    "capabilities": {...},
    "availability": {...}
  }
}
```

---

## Error Responses

All endpoints return standard HTTP status codes:

- `200 OK` - Success
- `400 Bad Request` - Invalid parameters
- `404 Not Found` - Resource not found
- `500 Internal Server Error` - Server error

**Error format:**
```json
{
  "error": "Invalid session ID",
  "code": "INVALID_SESSION"
}
```

---

## Monitoring Modes

### Local Spawn Mode

- Full metrics (inference, system, GPU, temperatures)
- Server controlled by llama-monitor
- All capabilities available

### Local Attach Mode

- Inference metrics only
- Connected to existing llama.cpp server
- Host metrics unavailable

### Remote Attach Mode

- Inference metrics only
- Connected to remote llama.cpp server
- Host metrics unavailable
- Endpoint shows `remote` kind

### Headless Mode

- No tray icon
- Web API available
- Use `--headless` or `--no-tray` flags

---

## Examples

### Check capabilities before connecting

```bash
curl http://localhost:7778/api/capabilities | jq .
```

### Start a session with a preset

```bash
curl -X POST http://localhost:7778/api/sessions \
  -H "Content-Type: application/json" \
  -d '{"mode": "spawn", "model_preset": "default"}' | jq .
```

### Attach to remote server

```bash
curl -X POST http://localhost:7778/api/sessions/attach \
  -H "Content-Type: application/json" \
  -d '{"endpoint": "http://192.168.1.100:8080"}' | jq .
```

### Monitor metrics via websocket

```bash
wscat -c ws://localhost:7778/ws/metrics
```

---

## CLI Flags Reference

See `docs/cli-flags.md` for complete CLI flag documentation.

---

**Version:** 1.0  
**Last updated:** 2026-04-20
