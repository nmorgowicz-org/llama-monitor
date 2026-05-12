# REST API Reference

Llama Monitor exposes a REST API on the same port as the web UI (default **7778**).

For real-time data use the WebSocket endpoint documented in `websocket-schema.md`. The REST API is for configuration, session management, and one-off queries.

## Base URL

```
http://localhost:7778
```

---

## Sessions

### `GET /api/sessions`
List all known sessions.

**Response:**
```json
[
  {
    "id": "session_1746000000000",
    "name": "Default Session",
    "mode": { "Spawn": { "port": 8001 } },
    "status": "Running",
    "preset_id": "my-preset",
    "created_at": 1746000000,
    "last_active": 1746001000
  }
]
```

**Session status values:** `Stopped` · `Running` · `Disconnected` · `Error(message)`  
**Session mode:** Nested object from Serde serialization of `SessionMode` enum:  
- Spawn: `{ "Spawn": { "port": 8001 } }`  
- Attach: `{ "Attach": { "endpoint": "http://..." } }`

---

### `POST /api/sessions`
Create a new session record (does not start it).

**Request:**
```json
{
  "name": "My Session",
  "mode": "spawn",
  "endpoint": "http://127.0.0.1:8001"
}
```

---

### `DELETE /api/sessions/{id}`
Delete a session record.

---

### `GET /api/sessions/active`
Get the currently active session.

**Response:**
```json
{
  "id": "session_1746000000000",
  "name": "Default Session",
  "mode": { "Spawn": { "port": 8001 } },
  "status": "Running",
  "preset_id": "",
  "created_at": 1746000000,
  "last_active": 1746001000
}
```

---

### `POST /api/sessions/active`
Set the active session by ID.

**Request:**
```json
{ "id": "session_1746000000000" }
```

---

### `POST /api/sessions/spawn`
Spawn a new llama.cpp server with a preset and make it the active session.

**Request:**
```json
{
  "preset_id": "my-preset",
  "name": "My Session",
  "port": 8001
}
```

Fields:
- `preset_id` (required): Preset ID to use for server launch
- `name` (optional): Session name; defaults to `"Session on port {port}"`
- `port` (optional): Server port; defaults to `8001`

**Response:**
```json
{
  "ok": true,
  "session_id": "session_1746000000000",
  "port": 8001
}
```

---

### `POST /api/attach`
Attach to an existing llama.cpp server endpoint.

**Request:**
```json
{
  "endpoint": "http://192.168.1.50:8001",
  "session_name": "Remote GPU Box"
}
```

**Response:**
```json
{
  "session_id": "session_1746000000001",
  "endpoint": "http://192.168.1.50:8001",
  "status": "attached"
}
```

---

### `POST /api/detach`
Detach from the current endpoint (stops polling, clears active session).

---

### `GET /api/capabilities`
Get current capabilities and availability reasons. Mirrors the `capabilities` / `endpoint_kind` / `availability` fields from the WebSocket push.

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
    "tray": true,
    "sensor_bridge_setup_available": false
  },
  "endpoint_kind": "Local",
  "session_kind": "Spawn",
  "tray_mode": "Desktop",
  "availability": {
    "system": "Available",
    "gpu": "Available",
    "cpu_temp": "Available"
  }
}
```

Fields:
- `capabilities`: MetricsCapabilities object (see `capabilities.md`)
- `endpoint_kind`: `"Local"` | `"Remote"` | `"Unknown"`
- `session_kind`: `"Spawn"` | `"Attach"` | `"None"`
- `tray_mode`: `"Desktop"` | `"Headless"` | `"Failed"`
- `availability`: Availability reasons for system/gpu/cpu_temp

See `capabilities.md` for value enumerations.

---

## Server Control

### `POST /api/start`
Start the llama.cpp server for the active spawn session.

### `POST /api/stop`
Stop the llama.cpp server for the active spawn session.

### `POST /api/kill-llama`
Force-kill any running llama.cpp process (emergency stop).

---

## Presets

Model presets store llama.cpp server launch parameters.

### `GET /api/presets`
```json
[
  {
    "id": "my-preset",
    "name": "My 7B Preset",
    "model_path": "/models/llama-3-8b.Q4_K_M.gguf",
    "n_gpu_layers": 99,
    "n_threads": 8,
    "n_ctx": 32768,
    "n_batch": 512,
    "n_parallel": 1,
    "extra_args": []
  }
]
```

### `POST /api/presets`
Create a preset. Body is a preset object (without `id`).

### `PUT /api/presets/{id}`
Update a preset.

### `DELETE /api/presets/{id}`
Delete a preset.

### `POST /api/presets/reset`
Reset all presets to factory defaults.

---

## Templates (Personas)

System prompt templates / personas for the chat interface.

### `GET /api/templates`
```json
[
  {
    "id": "helpful-assistant",
    "name": "Helpful Assistant",
    "system_prompt": "You are a helpful assistant.",
    "ai_name": "Assistant",
    "user_name": "User"
  }
]
```

### `POST /api/templates`
Create a template.

### `PUT /api/templates/{id}`
Update a template.

### `DELETE /api/templates/{id}`
Delete a template.

---

## Models

### `GET /api/models`
Discover available GGUF model files from the configured models directory.

**Response:**
```json
[
  {
    "name": "llama-3-8b.Q4_K_M.gguf",
    "path": "/models/llama-3-8b.Q4_K_M.gguf",
    "size_bytes": 4680000000
  }
]
```

### `POST /api/models/refresh`
Re-scan the models directory.

---

## Settings

### `GET /api/settings`
Retrieve persisted UI settings.

```json
{
  "preset_id": "my-preset",
  "port": 8001,
  "llama_server_path": "/usr/local/bin/llama-server",
  "llama_server_cwd": "",
  "models_dir": "/models",
  "server_endpoint": "http://127.0.0.1:8001",
  "llama_poll_interval": 1,
  "remote_agent_url": "",
  "remote_agent_token": "",
  "remote_agent_ssh_autostart": false,
  "remote_agent_ssh_target": "",
  "remote_agent_ssh_command": "",
"explicit_mode_policy": "",
   "context_card_view": "gauge",
   "enabled_context_notes": false,
   "enabled_suggestions": false,
   "enabled_quick_guide": false,
   "default_sidebar_width": 320,
   "suggestion_count": 5,
   "context_depth": 10,
   "suggestion_prompts": {}
}
```

Fields from `ui-settings.json`:
- `preset_id`, `port`, `llama_server_path`, `llama_server_cwd`, `models_dir`
- `server_endpoint`, `llama_poll_interval`
- `remote_agent_*`: Remote agent configuration
- `explicit_mode_policy`: Policy text appended when explicit mode is enabled
- `context_card_view`: `"gauge"` | `"text"` (UI preference)

### Guided Generation Settings

These settings control the guided generation features (context notes, suggestions, quick guide).

- `enabled_context_notes` (boolean): Enable/disable context notes in chat
- `enabled_suggestions` (boolean): Enable/disable suggestion generation
- `enabled_quick_guide` (boolean): Enable/disable quick guide panel
- `default_sidebar_width` (integer): Sidebar width in pixels (default: 320)
- `suggestion_count` (integer): Number of suggestions to generate per request (default: 5)
- `context_depth` (integer): Number of recent messages to include in context for suggestions (default: 10)
- `suggestion_prompts` (object): Category-specific prompt templates for suggestion generation. Keys are category names (e.g. `"general"`, `"coding"`, `"writing"`), values are prompt strings.

**Note:** Only `GET` is currently implemented; `PUT` returns 404. Settings are persisted via the `POST /api/settings/save` endpoint (deprecated in favor of direct file writes from the UI).

### `PUT /api/settings`
Save UI settings. Body is the same shape as GET response.

---

## GPU Environment

### `GET /api/gpu-env`
Get detected GPU backend configuration.

### `PUT /api/gpu-env`
Override GPU backend configuration.

---

## Chat

### `POST /api/chat`
Send a message to the active llama.cpp server via the OpenAI-compatible `/v1/chat/completions` endpoint. Streams the response.

**Request:**
```json
{
  "messages": [
    { "role": "system", "content": "You are helpful." },
    { "role": "user", "content": "Hello!" }
  ],
  "temperature": 0.7,
  "max_tokens": 512
}
```

### `POST /api/chat/abort`
Abort the currently streaming chat response.

### `POST /api/chat/suggestions`
Generate suggestions for a given category. Uses the current chat context to produce relevant follow-up prompts.

**Request:**
```json
{
  "category": "general",
  "count": 5
}
```

**Response:**
```json
{
  "suggestions": [
    "What's the weather like today?",
    "Can you explain quantum computing?",
    "Write a short poem about coding"
  ],
  "category": "general",
  "count": 3
}
```

### `POST /api/chat/suggestions/rewrite`
Rewrite a suggestion using the current chat context. Useful for refining or rephrasing a suggestion.

**Request:**
```json
{
  "suggestion": "Explain quantum computing",
  "category": "general"
}
```

**Response:**
```json
{
  "content": "Can you explain the basics of quantum computing in simple terms?"
}
```

### `GET /api/chat/tabs`
Load all persisted chat tabs from disk.

**Response:** Array of `ChatTab` objects (see below).

### `PUT /api/chat/tabs`
Save all chat tabs to disk. Body is an array of `ChatTab` objects.

### ChatTab Object

```json
{
  "id": "tab_abc123",
  "name": "My Chat",
  "system_prompt": "You are helpful.",
  "ai_name": null,
  "user_name": null,
  "explicit_level": null,
  "messages": [
    {
      "role": "user",
      "content": "Hello",
      "timestamp_ms": 1746000000000,
      "input_tokens": null,
      "output_tokens": null,
      "cumulative_input_tokens": null,
      "cumulative_output_tokens": null,
      "compaction_marker": null
    }
  ],
  "totalInputTokens": 0,
  "totalOutputTokens": 0,
  "model_params": {
    "temperature": 0.7,
    "top_p": 0.9,
    "top_k": 40,
    "min_p": 0.01,
    "repeat_penalty": 1.0,
    "max_tokens": null
  },
  "created_at": 1746000000000,
  "updated_at": 1746001000000,
  "auto_compact": null,
  "compact_threshold": null,
  "lastCtxPct": null,
  "activeTemplateId": null
}
```

**Notes:**
- `totalInputTokens` / `totalOutputTokens` use camelCase in both GET and PUT bodies.
- `lastCtxPct` is the last known context window pressure (0–100), persisted by the client so the context card can show a value before the server responds.
- `activeTemplateId` links the tab to a template/persona by ID.

---

## File Browser

### `GET /api/browse?path=/some/dir`
Browse the local filesystem for model files. Returns directory entries.

---

## Remote Agent

The remote agent runs on a target machine (typically Linux/Windows) and provides host metrics + server control over SSH or direct HTTP.

### `GET /api/remote-agent/releases/latest`
Fetch the latest agent release info from GitHub.

### `POST /api/remote-agent/detect`
Probe a target machine via SSH to check agent install state.

**Request:**
```json
{
  "ssh_target": "ssh://user@192.168.1.50:22",
  "ssh_connection": {
    "host": "192.168.1.50",
    "username": "user",
    "port": 22
  }
}
```

### `POST /api/remote-agent/ssh/host-key`
Retrieve the SSH host key for a target.

### `POST /api/remote-agent/ssh/trust`
Add a host key to the trusted hosts list.

### `GET /api/remote-agent/status`
Get the current remote agent connection status.

### `POST /api/remote-agent/install`
Install the remote agent on the target machine via SSH.

### `POST /api/remote-agent/start`
Start the remote agent on the target machine.

### `POST /api/remote-agent/update`
Update the remote agent binary on the target machine.

### `POST /api/remote-agent/stop`
Stop the remote agent on the target machine.

### `DELETE /api/remote-agent/remove`
Uninstall the remote agent from the target machine.

---

## Windows: LibreHardwareMonitor (LHM)

LHM provides hardware sensor data on Windows. These endpoints are Windows-only; non-Windows platforms return `{"available": false}` or equivalent.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/lhm/check` | Check if LHM is running/installed |
| `GET` | `/api/lhm/status` | Get disabled/enabled state |
| `GET` | `/api/lhm/progress` | Get install progress |
| `POST` | `/api/lhm/start` | Start LHM service |
| `POST` | `/api/lhm/install` | Download and install LHM |
| `POST` | `/api/lhm/uninstall` | Uninstall LHM |
| `POST` | `/api/lhm/disable` | Disable LHM without uninstalling |

---

## Windows: Sensor Bridge

The sensor bridge is a C# sidecar that forwards LHM sensor data.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/sensor-bridge/status` | Check bridge status |
| `POST` | `/api/sensor-bridge/install` | Install the bridge |
| `POST` | `/api/sensor-bridge/uninstall` | Uninstall the bridge |

---

## Self-Update

### `POST /api/self-update`
Trigger a self-update of the llama-monitor binary from GitHub releases.

---

## Error Responses

All endpoints return standard HTTP status codes.

| Code | Meaning |
|------|---------|
| `200` | Success |
| `400` | Bad request / invalid parameters |
| `404` | Resource not found |
| `500` | Internal server error |

---

## WebSocket

See `websocket-schema.md` for the real-time metrics stream.

```
ws://localhost:7778/ws
```

---

**Last updated:** 2026-05-03
