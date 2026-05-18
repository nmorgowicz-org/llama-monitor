# REST API Reference

Llama Monitor serves its REST API on the same port as the web UI, typically `http://localhost:7778`.

This page documents the live handlers in [`src/web/api.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs) and the persisted data shapes in [`src/state.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/state.rs), [`src/presets/mod.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/presets/mod.rs), and [`src/chat_storage.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/chat_storage.rs).

## Base URL

```text
http://localhost:7778
```

## Rate Limiting and DoS Protections

- A global rate limiter is applied to all non-index routes:
  - 200 requests/second base, with a burst allowance up to 700 per second.
  - Excess requests are rejected with 429 Too Many Requests.
- WebSocket connections are limited to 50 concurrent.
  - Beyond that, new connections receive 429 with { "error": "too many connections" }.
- The /api/db/query endpoint:
  - Enforces a 256 KB HTTP body limit.
  - Enforces a 16 KB SQL string limit.
- These limits are global and not per-client.

## Sessions

All session endpoints require authentication via `Authorization: Bearer <token>`.
Most require the `api-token`; a few elevated operations require the `db-admin-token`.
Without a valid token, the endpoint returns 401 with `{ "ok": false, "error": "unauthorized" }`.

### `GET /api/sessions`
Auth: api-token.
Returns the persisted session list from `sessions.json`.

```json
[
  {
    "id": "session_1746000000000",
    "name": "Default Session",
    "mode": { "Spawn": { "port": 8001 } },
    "status": "Stopped",
    "preset_id": "",
    "created_at": 1746000000,
    "last_active": 1746000000
  }
]
```

`mode` is serde's enum shape:
- spawn session: `{ "Spawn": { "port": 8001 } }`
- attached session: `{ "Attach": { "endpoint": "http://192.168.1.50:8001" } }`

`status` is one of:
- `"Stopped"`
- `"Running"`
- `"Disconnected"`
- `{ "Error": "message" }`

### `POST /api/sessions`
Auth: api-token.
Creates a session record only. It does not start a server.

Request body must be a full `Session` object:

```json
{
  "id": "session_custom",
  "name": "Remote Box",
  "mode": { "Attach": { "endpoint": "http://192.168.1.50:8001" } },
  "status": "Disconnected",
  "preset_id": "",
  "created_at": 1746000000,
  "last_active": 1746000000
}
```

Response:

```json
{ "ok": true }
```

On failure:

```json
{ "ok": false, "error": "Maximum sessions reached" }
```

### `DELETE /api/sessions/{id}`
Auth: db-admin-token.

```json
{ "ok": true }
```

### `GET /api/sessions/active`
Auth: api-token.
Returns a compact active-session summary, not the full `Session` object.

```json
{
  "id": "session_1746000000000",
  "name": "Default Session",
  "mode": "Spawn:8001",
  "status": "Running",
  "last_active": 1746001000
}
```

If there is no active session:

```json
{ "error": "No active session" }
```

### `POST /api/sessions/active`
Auth: api-token.

```json
{ "id": "session_1746000000000" }
```

Response:

```json
{ "ok": true }
```

### `POST /api/sessions/spawn`
Auth: db-admin-token.
Creates a spawn session, starts `llama-server` from a saved preset, and makes it active.

Request:

```json
{
  "preset_id": "default-1",
  "name": "Session on port 8001",
  "port": 8001
}
```

`preset_id` is required. `name` defaults to `Session on port {port}`. `port` defaults to `8001`.

Success response:

```json
{
  "ok": true,
  "session_id": "session_1746000000000"
}
```

### `POST /api/attach`
Auth: api-token.
Attaches to a reachable private-network or loopback endpoint.

Request:

```json
{
  "endpoint": "http://192.168.1.50:8001"
}
```

Success response:

```json
{
  "ok": true,
  "warning": null
}
```

If `/health` is unavailable the attach still succeeds, but `warning` explains that inference metrics will be missing.

### `POST /api/detach`
Auth: api-token.
Detaches only if the active session is an attach session.

```json
{ "ok": true }
```

### `GET /api/capabilities`
Returns the current metrics capability state.

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

## Server Control

### `POST /api/start`
Starts `llama-server` for the active spawn session. Request body is the full server config payload used by the launcher.

### `POST /api/stop`
Stops the managed `llama-server`.

### `POST /api/kill-llama`
Emergency process kill for `llama-server`.

## Presets

Presets are stored in `presets.json` and use the `ModelPreset` struct.

### `GET /api/presets`

```json
[
  {
    "id": "default-1",
    "name": "Example: Small Model 128K context",
    "model_path": "",
    "context_size": 128000,
    "ctk": "f16",
    "ctv": "f16",
    "tensor_split": "",
    "batch_size": 2048,
    "ubatch_size": 2048,
    "no_mmap": true,
    "ngram_spec": true,
    "parallel_slots": 1,
    "temperature": null,
    "top_p": null,
    "top_k": null,
    "min_p": null,
    "repeat_penalty": null,
    "n_cpu_moe": null,
    "gpu_layers": null,
    "mlock": false,
    "flash_attn": "",
    "split_mode": "",
    "main_gpu": null,
    "threads": null,
    "threads_batch": null,
    "rope_scaling": "",
    "rope_freq_base": null,
    "rope_freq_scale": null,
    "draft_model": "",
    "draft_min": null,
    "draft_max": null,
    "spec_ngram_size": null,
    "seed": null,
    "system_prompt_file": "",
    "extra_args": ""
  }
]
```

### `POST /api/presets`
Creates a preset from a full `ModelPreset` payload. If `id` is omitted, serde supplies one.

Response:

```json
{ "ok": true, "preset": { "...": "..." } }
```

### `PUT /api/presets/{id}`
Updates the preset matched by the path `id`.

### `DELETE /api/presets/{id}`

```json
{ "ok": true }
```

### `POST /api/presets/reset`
Replaces the in-memory and on-disk preset list with factory defaults.

```json
{ "ok": true }
```

## Templates

Templates are stored in `templates.json`. Built-in personas live in the frontend and are merged client-side; this API only returns user-stored entries.

### `GET /api/templates`

```json
[
  {
    "id": "t1746000000000",
    "name": "Helpful Assistant",
    "prompt": "You are a helpful assistant.",
    "explicit_policies": {
      "level1": "Soft policy text",
      "level2": "Unrestricted policy text"
    }
  }
]
```

`explicit_policies` is optional, and each level field is optional.

### `POST /api/templates`
Creates a template from a full `SystemPromptTemplate` payload.

```json
{ "ok": true, "template": { "...": "..." } }
```

### `PUT /api/templates/{id}`
Updates the template matched by the path `id`.

### `DELETE /api/templates/{id}`

```json
{ "ok": true }
```

## Models

### `GET /api/models`
Returns the current scan result for the configured `models_dir`.

```json
[
  {
    "path": "/models/Qwen3.5-27B-Q4_0.gguf",
    "filename": "Qwen3.5-27B-Q4_0.gguf",
    "size_bytes": 4680000000,
    "size_display": "4.4 GB",
    "quant_type": "Q4_0",
    "model_name": "Qwen3.5-27B",
    "is_split": false
  }
]
```

### `POST /api/models/refresh`
Rescans `models_dir`.

Success:

```json
{ "ok": true, "count": 12 }
```

Failure when no model directory is configured:

```json
{ "ok": false, "error": "no models directory configured (use --models-dir)" }
```

## Settings

### `GET /api/settings`
Returns the persisted `UiSettings` object from `ui-settings.json`, with sensitive fields masked.

Security:
- No authentication is required.
- `remote_agent_token` is masked (e.g., `"••••••••"`) to reduce exposure when consumed by the browser.

Example:

```json
{
  "preset_id": "",
  "port": 8001,
  "llama_server_path": "",
  "llama_server_cwd": "",
  "models_dir": "",
  "server_endpoint": "",
  "llama_poll_interval": 1,
  "remote_agent_url": "",
  "remote_agent_token": "••••••••",
  "remote_agent_ssh_autostart": false,
  "remote_agent_ssh_target": "",
  "remote_agent_ssh_command": "",
  "explicit_mode_policy": "",
  "context_card_view": "gauge",
  "ws_push_interval_ms": 500,
  "chat_input_height": "",
  "enabled_context_notes": true,
  "enabled_suggestions": true,
  "enabled_quick_guide": true,
  "default_sidebar_width": 280,
  "suggestion_prompts": {},
  "suggestion_count": 5,
  "context_depth": 10
}
```

### `GET /api/settings/full`
Returns the same `UiSettings` object, but with the real `remote_agent_token` value instead of a masked placeholder.

Security:
- Requires `api-token` authentication:
  - Header: `Authorization: Bearer <api-token>`
- Intended for trusted clients (e.g., internal tools, remote-agent.js) that need the actual token.

Example:

```json
{
  "preset_id": "",
  "port": 8001,
  "llama_server_path": "",
  "llama_server_cwd": "",
  "models_dir": "",
  "server_endpoint": "",
  "llama_poll_interval": 1,
  "remote_agent_url": "",
  "remote_agent_token": "actual-token-value",
  "remote_agent_ssh_autostart": false,
  "remote_agent_ssh_target": "",
  "remote_agent_ssh_command": "",
  "explicit_mode_policy": "",
  "context_card_view": "gauge",
  "ws_push_interval_ms": 500,
  "chat_input_height": "",
  "enabled_context_notes": true,
  "enabled_suggestions": true,
  "enabled_quick_guide": true,
  "default_sidebar_width": 280,
  "suggestion_prompts": {},
  "suggestion_count": 5,
  "context_depth": 10
}
```

### `PUT /api/settings`
Saves the `UiSettings` object to disk.

Response:

```json
{ "ok": true }
```

Notes:
- The live handler expects the current full `UiSettings` shape.
- The frontend also uses a narrow partial-update path for `ws_push_interval_ms`; external clients should prefer sending the full object.

## GPU Environment

### `GET /api/gpu-env`

```json
{
  "env": {
    "arch": "auto",
    "devices": "",
    "rocm_path": "/opt/rocm",
    "extra_env": []
  },
  "architectures": [
    { "id": "auto", "name": "Auto-detect", "hsa_version": "" }
  ],
  "detected": {
    "arch": "gfx1100",
    "count": 1,
    "names": ["gfx1100"]
  }
}
```

`detected` can be `null` if no GPU probe succeeds.

### `PUT /api/gpu-env`
Request body is the persisted `GpuEnv` object:

```json
{
  "arch": "auto",
  "devices": "",
  "rocm_path": "/opt/rocm",
  "extra_env": [["HSA_ENABLE_SDMA", "0"]]
}
```

Response:

```json
{ "ok": true }
```

## File Browser

### `GET /api/browse`
Auth: api-token.
Browses a local directory.

Query params:
- `path`: absolute or relative path; if omitted, starts at the current user's home directory
- `filter`: optional, currently `gguf` or `executable`

Example:

```text
/api/browse?path=/models&filter=gguf
```

Response:

```json
{
  "path": "/models",
  "parent": "/",
  "entries": [
    {
      "name": "Qwen3.5-27B-Q4_0.gguf",
      "is_dir": false,
      "size": 4680000000,
      "size_display": "4680 MB",
      "path": "/models/Qwen3.5-27B-Q4_0.gguf"
    }
  ]
}
```

On invalid input the API returns a JSON error payload such as:

```json
{ "path": "/missing", "error": "Path not found" }
```

## Chat Transport

### `POST /api/chat`
Auth: api-token.
Pass-through streaming proxy to the active session's `/v1/chat/completions`.

The request body is forwarded as raw bytes. The server does not validate or reshape the OpenAI-compatible payload before forwarding it upstream.

The response is an SSE stream that forwards upstream `data: ...` events.

### `POST /api/chat/abort`
Auth: api-token.
Current no-op acknowledgement endpoint:

```json
{ "ok": true }
```

### `POST /api/chat/suggestions`
Auth: api-token.
Generates guided-generation suggestions using either supplied chat context or a fallback tab lookup.

Request:

```json
{
  "tab_id": "tab_1746000000000",
  "category": "general",
  "count": 5,
  "context_depth": 10,
  "messages": [
    { "role": "user", "content": "..." },
    { "role": "assistant", "content": "..." }
  ],
  "system_prompt": "You are ...",
  "context_notes": [
    { "section": "plot", "content": "..." }
  ],
  "quick_guide_active": "",
  "prompt": null
}
```

Response:

```json
{
  "suggestions": ["..."],
  "cards": [],
  "category": "general",
  "count": 5
}
```

For `category: "director"`, `cards` can contain structured entries:

```json
{
  "type": "pressure",
  "title": "Tighten The Net",
  "effect": "More pressure now.",
  "detail": "Force the next reply into a narrower choice."
}
```

### `POST /api/keywords/generate`
Auth: api-token.

```json
{ "category": "noir" }
```

Response:

```json
{ "keywords": ["rain", "diner", "tension"] }
```

### `POST /api/context-notes/analyze`
Auth: api-token.

```json
{
  "messages": [
    { "role": "user", "content": "..." },
    { "role": "assistant", "content": "..." }
  ],
  "system_prompt": "...",
  "existing_notes": [
    { "section": "character", "content": "Kira is a cynical detective.", "created_at": 1746000000000 }
  ],
  "sections": ["character", "setting", "plot", "tone"]
}
```

Response:

```json
{
  "sections": [
    {
      "section": "character",
      "suggested": "Kira is now cooperating with the suspect.",
      "status": "stale",
      "reason": "Recent messages contradict the earlier note."
    }
  ]
}
```

`status` is `new`, `current`, or `stale`.

## Chat Persistence API

The live chat persistence layer is SQLite-backed and centered on `chat.db`. Chat tabs are no longer stored as one big JSON array.

### `GET /api/chat/tabs`
Auth: api-token.
Returns tab metadata only, without message bodies.

```json
[
  {
    "id": "tab_1746000000000",
    "name": "Noir Scene",
    "explicit_level": 0,
    "active_template_id": null,
    "pinned": false,
    "tab_order": 0,
    "last_ctx_pct": null,
    "total_input_tokens": 0,
    "total_output_tokens": 0,
    "message_count": 12,
    "created_at": 1746000000000,
    "updated_at": 1746000100000
  }
]
```

### `POST /api/chat/tabs`
Auth: api-token.
Creates one tab. Request body is a full `ChatTabRow`. If `id` is empty, the server generates one. `created_at` and `updated_at` are overwritten server-side.

Response is the created tab object.

### `GET /api/chat/tabs/{id}`
Auth: api-token.
Returns the full tab row plus messages.

```json
{
  "id": "tab_1746000000000",
  "name": "Noir Scene",
  "system_prompt": "",
  "ai_name": null,
  "user_name": null,
  "explicit_level": 0,
  "active_template_id": null,
  "auto_compact": true,
  "auto_compact_summarize": false,
  "compact_mode": "percent",
  "compact_threshold": 0.8,
  "model_params": {},
  "context_notes": [],
  "sidebar_width": 280,
  "tab_order": 0,
  "pinned": false,
  "last_ctx_pct": null,
  "total_input_tokens": 0,
  "total_output_tokens": 0,
  "created_at": 1746000000000,
  "updated_at": 1746000100000,
  "messages": [
    {
      "id": 1,
      "tab_id": "tab_1746000000000",
      "role": "user",
      "content": "Hello",
      "timestamp_ms": 1746000001000,
      "input_tokens": null,
      "output_tokens": null,
      "cumulative_input_tokens": null,
      "cumulative_output_tokens": null,
      "compaction_marker": false,
      "variants": null,
      "variant_index": null,
      "seq": 0
    }
  ]
}
```

### `PUT /api/chat/tabs/{id}`
Auth: api-token.
Full save for one tab. The handler updates tab metadata and then replaces all messages for that tab.

Response:

```json
{ "ok": true }
```

Important behavior:
- `id` is taken from the path.
- `updated_at` is overwritten server-side.
- message order is rewritten from array order
- during this replace path, only `role`, `content`, `timestamp_ms`, `input_tokens`, `output_tokens`, and `compaction_marker` are persisted for each message; cumulative token fields and variant fields are not preserved by this route

### `PATCH /api/chat/tabs/{id}/meta`
Auth: api-token.
Metadata-only update for one tab. Request body uses the same tab shape, but `messages` are ignored.

### `POST /api/chat/tabs/{id}/messages`
Auth: api-token.
Appends one or more messages.

Request:

```json
{
  "messages": [
    {
      "tab_id": "ignored-and-overwritten",
      "role": "assistant",
      "content": "Hello back",
      "timestamp_ms": 1746000002000,
      "input_tokens": 10,
      "output_tokens": 25,
      "cumulative_input_tokens": 10,
      "cumulative_output_tokens": 25,
      "compaction_marker": false,
      "variants": null,
      "variant_index": null,
      "seq": 999
    }
  ]
}
```

Response:

```json
{ "ok": true, "last_id": 42 }
```

Important behavior:
- `tab_id` in each message is overwritten from the path
- `seq` is ignored and assigned automatically
- this append route does persist `cumulative_input_tokens`, `cumulative_output_tokens`, `variants`, and `variant_index`

### `PATCH /api/chat/tabs/order`
Auth: api-token.

```json
{
  "tab_order": ["tab_a", "tab_b", "tab_c"]
}
```

Response:

```json
{ "ok": true }
```

### `DELETE /api/chat/tabs/{id}`
Auth: api-token.

```json
{ "ok": true }
```

## Chat Search

### `GET /api/chat/search`
Auth: api-token.
Full-text search over non-compaction-marker messages in `chat.db`.

Query params:
- `q`: required search string
- `limit`: optional page size, default `20`, max `100`
- `offset`: optional result offset, default `0`

Example:

```text
/api/chat/search?q=slow%20endpoint&limit=20&offset=0
```

Response:

```json
{
  "results": [
    {
      "tab_id": "tab_1746000000000",
      "tab_name": "Debug Session",
      "message_id": 17,
      "role": "assistant",
      "snippet": "Check <mark>slow</mark> HTTP <mark>endpoint</mark> first.",
      "timestamp_ms": 1746000002000
    }
  ],
  "total": 37,
  "limit": 20,
  "offset": 0,
  "has_more": true
}
```

Search notes:
- punctuation is normalized before FTS lookup
- prefix matching is used internally
- empty or unparseable queries return an empty paged result object

## Database Admin

All `/api/db/*` routes operate on the SQLite chat database.

Authentication:
- `GET /api/auth/status`:
  - Public endpoint used by the frontend to discover whether auth is enabled.
  - Returns enabled methods, whether auth is managed by startup flags, a local recovery command,
    and whether the current request is already authenticated.
- `POST /api/auth/login`:
  - Public endpoint used by in-app form auth.
  - Expects JSON:
      { "username": "<user>", "password": "<password>" }
  - On success, sets an HttpOnly session cookie.
- `POST /api/auth/logout`:
  - Public endpoint that clears the in-app form-auth session cookie.
- `GET /api/auth/config`:
  - Protected endpoint for the Security tab.
  - Requires `api-token`.
  - Returns the current dashboard auth mode, username, whether it is CLI-managed, and local recovery metadata.
- `PUT /api/auth/config`:
  - Protected endpoint for updating dashboard auth.
  - Requires `api-token`.
  - Expects JSON:
      {
        "basic_enabled": true|false,
        "form_enabled": true|false,
        "username": "<user>",
        "current_password": "<old-password-or-empty>",
        "new_password": "<new-password-or-empty>"
      }
  - Use `new_password` when setting a password for the first time or changing it.
  - Sending both mode flags as `false` disables dashboard auth and clears `auth-config.json`.
- Most endpoints require one of two tokens:
  - `api-token`: general API token for routine admin operations.
  - `db-admin-token`: elevated token for destructive or high-risk operations.
- Tokens are provided via:
  - Header: `Authorization: Bearer <token>`
- `GET /api/db/admin-token`:
  - Returns the db-admin-token for use by the in-browser DB Admin UI.
  - Allowed whenever a request has already passed configured auth, or when the server is bound to loopback with no auth configured.
  - Example:
      { "token": "<db-admin-token>" }
- `GET /api/internal/api-token` follows the same bootstrap policy for the main UI.

### `GET /api/db/stats`
Requires `api-token`.

```json
{
  "tab_count": 3,
  "message_count": 248,
  "fts_index_count": 248
}
```

### `GET /api/db/integrity`
Requires `api-token`.

```json
{
  "status": "healthy",
  "detail": "ok"
}
```

If `detail` is not `"ok"`, `status` is `"corrupted"`.

### `POST /api/db/maintenance`
Requires `api-token`.

```json
{ "operation": "checkpoint" }
```

Supported operations:
- `checkpoint`
- `vacuum`
- `rebuild_fts`
- `analyze`

Responses:

```json
{ "backfilled": 0, "deleted": 0, "log": 0 }
```

```json
{ "status": "vacuumed" }
```

```json
{ "status": "fts_rebuilt" }
```

```json
{ "status": "analyzed" }
```

### `POST /api/db/backup`
Requires `api-token`.

Creates a manual backup in `~/.config/llama-monitor/backups/chat_<timestamp>.db`.

```json
{
  "status": "backup_created",
  "path": "/Users/nick/.config/llama-monitor/backups/chat_1746000000000.db",
  "size_bytes": 40960
}
```

Manual backups are pruned to the 7 newest `chat_*.db` files.

### `DELETE /api/db/backup`
Requires `db-admin-token`.

```json
{
  "backup_name": "chat_1746000000000.db"
}
```

Response:

```json
{
  "status": "deleted",
  "backup": "chat_1746000000000.db"
}
```

### `GET /api/db/backups`
Requires `api-token`.

Lists both manual backups (`chat_*.db`) and automatic hourly backups (`chat_auto_*.db`).

```json
{
  "backups": [
    {
      "name": "chat_auto_1746000000000.db",
      "size": 40960,
      "modified": 1746000000000
    }
  ],
  "total_size": 40960
}
```

### `POST /api/db/restore`
Requires `db-admin-token`.

Validates backup_name:
- No path traversal (no ‘..’, leading ‘/’, or backslashes).
- Resolved path must be inside the backups directory.

Before restore, creates a safety backup as pre_restore_<timestamp>.db.
After restore, runs an integrity check; returns a note if it fails.

```json
{
  "backup_name": "chat_1746000000000.db"
}
```

Success:

```json
{
  "status": "restored",
  "backup": "chat_1746000000000.db"
}
```

Before restore, the server creates `pre_restore_<timestamp>.db` in the same `backups/` directory.

### `POST /api/db/repair`
Requires `db-admin-token`.

```json
{ "operation": "repair_indexes" }
```

Supported operations:
- `repair_indexes`
- `emergency_recovery`

Responses:

```json
{ "status": "indexes_repaired" }
```

```json
{ "status": "recovery_attempted" }
```

### `GET /api/db/indexes`
Requires `api-token`.

```json
[
  {
    "name": "idx_messages_tab",
    "table": "messages",
    "sql": "CREATE INDEX IF NOT EXISTS idx_messages_tab ON messages(tab_id, seq)",
    "rebuildable": false
  }
]
```

### `POST /api/db/query`
Requires `db-admin-token`.

Runs admin queries, limited to `SELECT` and `PRAGMA`.

```json
{
  "sql": "SELECT id, name FROM tabs ORDER BY updated_at DESC LIMIT 10"
}
```

Response:

```json
{
  "columns": ["id", "name"],
  "rows": [
    { "id": "tab_1", "name": "Noir Scene" }
  ],
  "row_count": 1
}
```

## TLS / ACME

Endpoints for managing TLS and ACME-based certificate provisioning.

Authentication:
- GET /api/tls/config: requires api-token.
- PUT /api/tls/config: requires api-token.
- POST /api/tls/acme/request: requires api-token.
- POST /api/tls/acme/renew: requires api-token.
- All TLS/ACME endpoints reject requests without a valid Bearer api-token.

### `GET /api/tls/config`
Returns the current TLS mode and ACME summary (without secrets).

Header:
- `Authorization: Bearer <api-token>`

Response:

```json
{
  "mode": "acme",
  "customCertPath": null,
  "customKeyPath": null,
  "acme": {
    "enabled": true,
    "fqdn": "llama-monitor.example.com",
    "environment": "staging",
    "dnsProvider": "cloudflare",
    "validationDelay": 300,
    "lastRenewal": null,
    "certPath": "/path/to/cert.pem",
    "keyPath": "/path/to/key.pem"
  }
}
```

`mode` can be:
- `"none"`
- `"self-signed"`
- `"custom"`
- `"acme"`

### `PUT /api/tls/config`
Updates TLS configuration, including ACME settings.

Header:
- `Authorization: Bearer <api-token>`
- `Content-Type: application/json`

Example body (ACME):

```json
{
  "mode": "acme",
  "acme": {
    "enabled": true,
    "fqdn": "llama-monitor.example.com",
    "environment": "staging",
    "dnsProvider": "cloudflare",
    "dnsConfig": {
      "CF_API_TOKEN": "your-token-here"
    },
    "validationDelay": 300
  }
}
```

Response:

```json
{ "ok": true }
```

### `POST /api/tls/acme/request`
Triggers an ACME certificate request.

Header:
- `Authorization: Bearer <api-token>`

Response:

```json
{ "status": "requested" }
```

### `POST /api/tls/acme/renew`
Triggers an ACME certificate renewal.

Header:
- `Authorization: Bearer <api-token>`

Response:

```json
{ "status": "renewed" }
```

## Legacy Chat Fields

The project previously used a flat-file chat format. The live SQLite-backed API no longer persists several legacy fields that still appear in old docs, old exports, or migration code.

Not part of the live `ChatTabRow` API:
- `ai_gender`
- `role_boundary_custom`
- `quick_guide_active`
- `armed_story_beats`
- `context_custom_sections`
- `quick_guide_draft`

Not part of the live `MessageRow` API:
- `thinking_content`
- `summarized`
- `dropped_count`
- `dropped_preview`
- `tokens_freed_estimate`
- `ctx_pct_before`
- `memory_version`
- `memory_domain`
- `summary_kind`
- `compacted_at`
- `compacted_message_count_total`
- `recent_tail_kept`

Compatibility note:
- startup migration from legacy `chat-tabs.json` still reads some of those older fields
- the live REST persistence API does not return or preserve them

## Errors

Most handlers return JSON error payloads rather than relying on HTTP status alone. Common shapes are:

```json
{ "ok": false, "error": "Preset not found" }
```

```json
{ "error": "No active session" }
```

## WebSocket

For realtime metrics and capability pushes, use:

```text
ws://localhost:7778/ws
```

Limits:
- Maximum 50 concurrent connections.
- Excess connections are rejected with 429 Too Many Requests.

See `docs/reference/realtime-communication.md` and `docs/reference/capabilities.md`.

## Self-Update

POST /api/self-update:
- Requires db-admin-token (elevated operation).
- Requires explicit confirmation: { "confirm": "update" }.
- Cooldown: 5 minutes between calls; returns 429 with seconds_remaining if too soon.
- On success, schedules exit(0); OS/user must relaunch.
- Example request:
    { "confirm": "update" }
- Example success:
    { "ok": true, "tag_name": "v0.3.0", "restart_required": true }

## Kill-Llama

POST /api/kill-llama:
- Emergency kill for llama-server.
- Requires db-admin-token (elevated operation).
- Requires confirmation field: { "confirm": "kill" }.
- Cooldown: 30 seconds between calls; returns 429 with seconds_remaining if too soon.
- Example:
    Request:  { "confirm": "kill" }
    Success:  { "ok": true }
    Too soon: { "error": "too soon; please wait", "seconds_remaining": 12 }

## Remote Agent

All `/api/remote-agent/*` endpoints require a bearer token.

- api-token: `Authorization: Bearer <api-token>`
- db-admin-token: `Authorization: Bearer <db-admin-token>`

Without a valid token, endpoints return 401 with `{ "ok": false, "error": "unauthorized; <token-type> required" }`.

### `GET /api/remote-agent/releases/latest`

- Auth: api-token.
- Returns the latest GitHub release and assets.

### `POST /api/remote-agent/detect`

- Auth: api-token.
- Body: `{ ssh_target, ssh_connection?, agent_url? }`
- Detects remote OS, architecture, installed version, and matching release asset via SSH.

### `POST /api/remote-agent/ssh/host-key`

- Auth: api-token.
- Body: `{ ssh_target, ssh_connection }`
- Scans the SSH host key for the given target.

### `POST /api/remote-agent/ssh/trust`

- Auth: api-token.
- Body: `{ ssh_target, ssh_connection, key_hex }`
- Trusts the scanned host key for future SSH operations.

### `POST /api/remote-agent/status`

- Auth: api-token.
- Body: `{ ssh_target, ssh_connection? }`
- Checks the status of a managed remote agent (installed, running, task health).

### `POST /api/remote-agent/start`

- Auth: api-token.
- Body: `{ ssh_target, ssh_connection?, install_path?, start_command? }`
- Starts the remote agent on the target host.

### `POST /api/remote-agent/update`

- Auth: api-token.
- Body: `{ ssh_target, ssh_connection?, agent_url? }`
- Stops the existing agent, installs the latest release, and restarts it.

### `POST /api/remote-agent/stop`

- Auth: api-token.
- Body: `{ ssh_target, ssh_connection? }`
- Stops the remote agent process.

### `POST /api/remote-agent/install`

- Auth: db-admin-token (elevated).
- Body: `{ ssh_target, ssh_connection?, asset, install_path? }`
- Installs the remote agent binary and writes a `remote-agent-config.json` with the api-token.

### `POST /api/remote-agent/remove`

- Auth: db-admin-token (elevated).
- Body: `{ ssh_target, ssh_connection? }`
- Removes the managed remote agent (stops process, deletes startup task, removes binary).

### `GET /api/remote-agent/tls-status`

- Auth: api-token.
- Returns mTLS certificate status (CA, server, client).

For full details on the remote agent flow, see [Remote Agent](remote-agent.md).
