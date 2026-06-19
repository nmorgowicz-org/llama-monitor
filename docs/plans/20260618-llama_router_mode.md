# llama.cpp Router Mode Integration

Date: 2026-06-18
Status: Analysis / proposal — implementation deferred to MLX integration (see §7)

Goal: Add an optional **router mode** to llama-monitor so a single llama-server process can manage and serve multiple models, switching between them automatically based on the `"model"` field in API requests. Enables zero-friction model switching from external tools (SillyTavern, opencode, hermes via LiteLLM) without touching the app UI.

---

## 1. Background

### 1.1 What router mode is

llama-server enters router mode when launched with no `-m` flag. It becomes a lightweight dispatcher:

- Discovers models from `--models-dir` (filesystem), `--models-preset` (INI file), or `LLAMA_CACHE`
- Spawns child llama-server processes on demand when a model is requested
- Routes `/v1/chat/completions` and other POST endpoints by the `"model"` field in the request body
- Routes GET endpoints by `?model=` query param
- Manages model lifecycle: `unloaded → loading → loaded → sleeping → unloaded`
- Exposes SSE at `GET /models/sse` for real-time lifecycle events

### 1.2 Child process I/O (key finding from source audit)

Each child's stdout/stderr is piped and forwarded to the router's log stream, prefixed with the child's port:

```c
LOG("[%5d] %s", port, buffer);  // server-models.cpp ~L896
```

Router ↔ child coordination uses stdin/stdout as a structured command channel:
- Router → child stdin: `cmd_router_to_child:exit`
- Child → router stdout: `cmd_child_to_router:ready`, `cmd_child_to_router:sleep`, `cmd_child_to_router:info:<json>`

These control messages appear in the log stream and must be filtered out from display.

### 1.3 Model switching flow

```
Client sends: POST /v1/chat/completions {"model": "qwen3-roleplay", ...}
  → router: model not loaded
  → sends exit command to current child (if --models-max reached)
  → waits for child to exit (or force-kills after stop_timeout seconds)
  → spawns new child with preset args for "qwen3-roleplay"
  → waits for cmd_child_to_router:ready on child stdout
  → forwards original request to new child
```

Switching is triggered purely by the model field — no API call or UI interaction required.

### 1.4 VRAM behaviour on memory-constrained systems

`--models-max N` (default: 4) caps simultaneous loaded children. On a near-maxed system (e.g. 5090 + Qwen3.6-27B), set `--models-max 1`. The "switch" is a full unload + reload — no VRAM sharing. Load time from NVMe is the only latency. `--sleep-idle-seconds N` proactively evicts idle models to free VRAM before a competing request arrives.

---

## 2. Current Architecture (single-model)

| Component | Role |
|---|---|
| `src/llama/server.rs` | Spawns/kills one llama-server child; owns PID, captures stdout/stderr, death watcher |
| `src/llama/poller.rs` | Polls `/metrics` on one endpoint |
| `src/llama/spawn_wizard.rs` | UI wizard that builds one `ServerConfig` |
| `src/state.rs` | `server_running`, `server_config`, `server_child` — all singular |
| `ServerConfig` | Flat struct of args passed to one child |
| JS: `nav.js` | Start/Stop controls tied to single server state |
| JS: console log viewer | Filters one log stream; strips `[monitor]` prefix |

---

## 3. Architecture Changes Required

### 3.1 Spawn layer (`src/llama/server.rs`)

Add a `start_router()` function alongside the existing `start_server()`. It builds a router-mode command (no `-m`, adds `--models-dir`, `--models-preset`, `--models-max`, `--sleep-idle-seconds`) and uses the same process ownership pattern (PID, death watcher, stdout/stderr capture). `start_server()` stays unchanged — single-model mode is not removed.

New `RouterConfig` struct (separate from `ServerConfig`):
```rust
pub struct RouterConfig {
    pub models_dir: Option<PathBuf>,
    pub preset_file: Option<PathBuf>,
    pub models_max: u32,          // default 1 for VRAM-constrained, 4 otherwise
    pub sleep_idle_seconds: i32,  // -1 = disabled
    pub port: u16,
    pub bind_host: Option<String>,
    pub api_key: Option<String>,
    pub base_args: String,        // passthrough GPU/threading flags inherited by all children
}
```

### 3.2 Log capture (`src/llama/server.rs` + state)

Router log stream format: `[PORT] <line>` or `[ PORT] <line>` (5-char port field).

Parser changes:
- Extract port prefix → route to per-model log buffer keyed by port/model name
- Strip `cmd_child_to_router:*` control lines from display (still process them for state updates if needed)
- `[monitor]` prefix behaviour unchanged for router-level lines

New state: `router_model_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>` — keyed by model name, capped same as current `server_logs`.

### 3.3 State (`src/state.rs`)

Add a `server_mode` enum:
```rust
pub enum ServerMode {
    SingleModel,
    Router,
}
```

Router state additions:
```rust
pub router_running: Mutex<bool>,
pub router_models: Mutex<HashMap<String, RouterModelState>>,
```

```rust
pub struct RouterModelState {
    pub status: RouterModelStatus,  // Unloaded / Loading / Loaded / Sleeping / Failed
    pub port: u16,
    pub last_activity: Option<Instant>,
}

pub enum RouterModelStatus { Unloaded, Loading, Loaded, Sleeping, Downloading, Failed(String) }
```

State is updated from two sources:
1. Log stream parsing (`cmd_child_to_router:*` commands)
2. `GET /models/sse` SSE subscription (real-time lifecycle events from router API)

### 3.4 Poller (`src/llama/poller.rs`)

In router mode, poll `/v1/models` to enumerate loaded children. For each loaded child, poll its individual metrics endpoint (the child's port is known from the model list response or log prefix). Aggregate into a per-model metrics map. Dashboard shows metrics for the "active" model (the one that last served a request, or user-selected).

### 3.5 Preset file management (`src/llama/` — new file: `router_preset.rs`)

Generates and manages the `preset.ini` file that the router reads. Maps existing `ServerConfig` fields to INI key/value pairs (long-form arg names, no leading dashes).

INI format:
```ini
version = 1

[*]
ngl = all
fa = on
jinja = on
ctx-size = 8192

[Qwen3-27B-Q4_K_M]
m = /path/to/Qwen3-27B-Q4_K_M.gguf
ctx-size = 32768
n-cpu-moe = 20

[gemma-3-4b-it]
hf = ggml-org/gemma-3-4b-it-GGUF:Q4_K_M
ctx-size = 8192
```

Per-model entries are editable from the UI. The file is written to `app_data_dir/router_preset.ini` and passed to the router via `--models-preset`.

### 3.6 Web API (`src/web/api.rs`)

New endpoints:
- `GET /api/router/status` — running state, mode, active models list
- `POST /api/router/start` — starts router mode with `RouterConfig`
- `POST /api/router/stop` — stops router (kills all children)
- `GET /api/router/models` — list of models + status
- `POST /api/router/models/load` — trigger manual load
- `POST /api/router/models/unload` — trigger manual unload
- `GET /api/router/logs/:model_name` — per-model log stream

Existing `/api/server/*` endpoints for single-model mode are unchanged.

---

## 4. UI/UX Design

### 4.1 Mode toggle

A toggle in the server control area: **Single Model** | **Router**. Switching modes stops any running server (with confirmation if active). Default: Single Model — existing behaviour is preserved.

### 4.2 Router dashboard panel

When in router mode, the main server panel is replaced by a **Router** panel showing:

```
┌─ Router  ●  Running on :8080 ──────────────────────────────┐
│                                                              │
│  Qwen3-27B-Q4_K_M          ● Loaded    8081   [Unload]     │
│  gemma-3-4b-it              ○ Sleeping  8082   [Load]       │
│  Llama-3.3-70B              ○ Unloaded  —      [Load]       │
│                                                  [+ Add]    │
│                                                              │
│  Active metrics: Qwen3-27B  ▸ 14.2 t/s  ctx 12k/32k       │
└──────────────────────────────────────────────────────────────┘
```

Status indicators use the same colour language as the existing endpoint health strip (green = loaded, amber = sleeping/loading, grey = unloaded, red = failed).

### 4.3 Per-model configuration

Each model row expands or opens a side-panel with its preset INI overrides — essentially a slimmer `SpawnWizard` showing only the fields that differ from the global defaults. Fields map to INI keys. Save writes the preset file and optionally triggers a reload of that model.

### 4.4 Console log viewer

Add a model selector above the log viewer (only visible in router mode). Options: **All** (interleaved, port-tagged) | per-model name. In per-model view, the `[PORT]` prefix is stripped and replaced with a faint model name chip, matching the current `[monitor]` treatment. Control protocol lines (`cmd_child_to_router:*`) are always hidden.

### 4.5 Model switching UX for external tools

No UI change needed for external clients — model switching is fully automatic via the model field in API requests. The router panel's status indicators update in real time via the SSE feed, so the user can observe switches happening without any interaction.

A "Currently serving" indicator (model name + port + t/s) sits at the top of the router panel and updates on each `model_status: loaded` SSE event.

### 4.6 Preset editor (future)

A dedicated preset editor page (similar to Settings) lets users manage the full `preset.ini` visually: drag-to-reorder models, set `load-on-startup`, configure `stop-timeout`, and preview the generated INI. Out of scope for initial implementation.

---

## 5. Implementation Phases

**Phase 1 — Router spawn + log capture**
- `RouterConfig` struct
- `start_router()` / `stop_router()` in `server.rs`
- Port-tagged log parser + per-model log buffers in state
- `/api/router/start`, `/api/router/stop`, `/api/router/status`

**Phase 2 — Model state tracking**
- SSE subscription to router's `/models/sse`
- `RouterModelState` map in `AppState`
- `/api/router/models` endpoint
- Poller adaptation for multi-child metrics

**Phase 3 — Preset management**
- `router_preset.rs` — INI generator
- `/api/router/models/load`, `/api/router/models/unload`
- Per-model config editing in UI

**Phase 4 — UI**
- Mode toggle in server control area
- Router dashboard panel with live status
- Per-model log view in console
- Per-model config expand/side-panel

---

## 7. Relationship to Rapid-MLX Integration and Sequencing

The Rapid-MLX integration plan (`20260611-rapid_mlx_integration.md`) establishes a
backend-neutral `src/inference/` layer with a shared supervisor and per-backend adapters.
Router mode implementation depends on this infrastructure and should not be built before it.

### Why not now

The current codebase has a `src/llama/server.rs` monolith that the MLX plan explicitly
calls out as the thing not to extend further. Building router mode on top of it now means
doing the work twice — once as a bolted-on parallel system, then again during MLX Milestone
2 when the shared supervisor boundary is extracted and llama.cpp routes through enum dispatch.

### Correct placement in the new architecture

Router mode is a **launch mode variant within the llama.cpp backend adapter**, not a
separate top-level backend:

```rust
// src/inference/llama_cpp.rs
pub enum LlamaCppLaunchMode {
    SingleModel(ServerConfig),
    Router(RouterConfig),
}
```

Both modes share `src/inference/supervisor.rs` for process lifecycle (spawn, stdout/stderr
capture, exit watching, stop escalation). The router variant adds its specifics — preset
file generation, port-tagged log routing, per-model `RouterModelState` tracking, SSE
subscription — entirely inside the llama.cpp adapter boundary.

### Preset schema placement

`RouterConfig` belongs in the llama.cpp backend section of the new `ModelPreset` schema
(parallel to `rapid_mlx`), not as a top-level peer of `InferenceBackend`:

```
ModelPreset
  backend: llama_cpp
  llama_cpp:
    launch_mode: router          ← new field
    router: RouterConfig         ← new section, present only when launch_mode = router
    single_model: ServerConfig   ← existing section
  rapid_mlx: ...
```

### UI placement

The mode selector is a sub-option within the llama.cpp engine card ("Single model /
Router"), not a third engine card alongside llama.cpp and Rapid-MLX.

### Recommended implementation timing

- **MLX Milestone 2** (shared supervisor extracted, llama.cpp in enum dispatch): add
  `LlamaCppLaunchMode` enum and `RouterConfig` struct. Wire the router launch path through
  the supervisor. No UI yet.
- **MLX Milestone 3 or 4**: add preset schema support for `launch_mode: router`.
- **After MLX Milestone 7** (engine-aware wizard complete): add router sub-mode toggle to
  the llama.cpp engine card and the router dashboard panel described in §4 of this doc.

---

## 8. Key Constraints and Risks

| | |
|---|---|
| **VRAM on constrained systems** | Default `--models-max` to 1 for single-GPU setups; expose it as a router setting with a clear label ("Max simultaneous models") |
| **Port allocation** | Router assigns child ports automatically (sequential from base+1). We don't control them — we discover them from the log prefix or `/v1/models` response |
| **Router mode is experimental** | llama.cpp README explicitly warns this. Should surface a banner in the UI when router mode is active |
| **Preset file write race** | If the user edits a preset while a child is loading, the INI changes don't apply until the next child spawn. Make clear that edits take effect on next load |
| **Single-model mode must stay default** | Router mode is opt-in. All existing `ServerConfig` / `SpawnWizard` / preset flows remain unchanged |
| **Windows path handling** | `--models-dir` and preset paths need proper cross-platform handling; use `PathBuf` throughout |
