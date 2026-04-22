# Llama Monitor

Web dashboard for managing [llama.cpp](https://github.com/ggerganov/llama.cpp) servers with real-time GPU monitoring.

## Monitoring Modes

Llama Monitor supports two modes of operation:

### Local Mode (Spawn)
- Runs llama-server on your local machine
- Full hardware monitoring (CPU, RAM, GPU temp, VRAM, power, clocks)
- GPU monitoring auto-detected: AMD ROCm, NVIDIA, Apple Silicon
- Perfect for local development and testing

### Remote Mode (Attach)
- Connects to an existing llama-server instance
- Inference metrics only (prompt/gen speed, KV cache, slots)
- GPU/system sections auto-hidden when not available
- Remote agent provides backend metrics via HTTP endpoint

## Features

- **Capability-Aware Monitoring** -- Backend exposes metric capabilities and availability reasons
  - Local vs. remote monitoring mode clearly displayed
  - Unavailable metrics show concise reasons instead of empty tables
  - GPU/system sections hidden when not available
- **Multi-Session Support** -- Run multiple llama-server instances simultaneously with independent session management
  - Spawn new local servers on custom ports or attach to external servers
  - Session persistence across restarts (saved to `~/.config/llama-monitor/sessions.json`)
  - Quick session switching with sidebar navigation
- **Server Management** -- Start/stop llama.cpp server from configurable presets
- **Real-time GPU Monitoring** -- Temperature, load, VRAM, power, clock speeds (AMD ROCm + NVIDIA)
- **Inference Metrics** -- Prompt/generation speed, KV cache usage, slot status via Prometheus endpoint
- **Customizable Presets** -- Create, edit, copy, delete model presets with all llama.cpp parameters; persisted to disk
- **File Browser** -- Browse the filesystem to select llama-server binary and .gguf model files
- **Integrated Chat** -- Streaming chat UI with reasoning/thinking block support, proxied to the configured port
- **Persistent Settings** -- Selected preset, port, and server paths survive page reloads and app restarts
- **PWA Support** -- Installable as a standalone app on mobile and desktop

## Supported Hardware

| Vendor | Tool | Detection |
|--------|------|-----------|
| AMD | `rocm-smi` | Auto-detected |
| NVIDIA | `nvidia-smi` | Auto-detected |
| Apple Silicon | `mactop` | Auto-detected (Apple Silicon only) |

GPU backend is auto-detected at startup. Override with `--gpu-backend apple|rocm|nvidia|none`.

## Installation

### Pre-built Binaries

Download the latest release from the [Releases page](../../releases/latest).

| Platform | File |
|----------|------|
| Linux x86_64 | `llama-monitor-linux-x86_64` |
| Linux aarch64 | `llama-monitor-linux-aarch64` |
| Windows x86_64 | `llama-monitor-windows-x86_64.exe` |
| macOS Apple Silicon | `llama-monitor-macos-aarch64.tar.gz` |

#### macOS (Apple Silicon)

The macOS binary is distributed as a `.tar.gz` to preserve the executable bit. macOS also applies a quarantine flag to downloaded files that must be cleared before running.

```bash
# Extract the archive
tar -xzf llama-monitor-macos-aarch64.tar.gz

# Remove the macOS quarantine flag (required for unsigned binaries)
xattr -dr com.apple.quarantine ./llama-monitor-macos-aarch64

# Run
./llama-monitor-macos-aarch64
```

> **Note:** The quarantine flag is set by macOS on any file downloaded from the internet. Without removing it, macOS will refuse to run the binary with "cannot be opened because the developer cannot be verified".

#### Linux

```bash
chmod +x llama-monitor-linux-x86_64  # or llama-monitor-linux-aarch64
./llama-monitor-linux-x86_64
```

### From Source

```bash
# Install Rust if needed: https://rustup.rs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone https://github.com/nickveldrin/llama-monitor.git && cd llama-monitor
cargo build --release
```

The binary is at `target/release/llama-monitor`. It's a single self-contained executable (frontend is embedded at compile time).

### Dependencies

- **llama.cpp** -- `llama-server` binary (with `--metrics` and `--jinja` support)
- **GPU monitoring** (optional):
  - AMD: `rocm-smi`
  - NVIDIA: `nvidia-smi`
  - Apple Silicon: `mactop` (`brew install mactop`)

## Apple Silicon Support

On macOS with Apple Silicon (M1/M2/M3/M4/M5 or later), install `mactop` for GPU/system metrics:

```bash
brew install mactop
```

The backend is auto-detected. Override with `--gpu-backend apple`.

See [`docs/2026-04-12-apple-silicon-implementation.md`](docs/2026-04-12-apple-silicon-implementation.md) for details.

## Quick Start

```bash
# Basic usage (configure paths in the web UI)
./llama-monitor

# Or specify llama-server location via CLI
./llama-monitor \
  --llama-server-path /usr/local/bin/llama-server \
  --port 7778
```

Open `http://localhost:7778` in your browser. Click the gear icon to configure server paths, then create a preset to get started.

## Multi-Session Workflow

1. **Create a Session** -- Click `+ New Session` in the sidebar
   - **Spawn Mode**: Creates a session with a port; server runs locally
   - **Attach Mode**: Connects to an external server at a URL endpoint

2. **Spawn a Server** -- Use the "Spawn with Preset" button to start a llama-server instance with selected preset config

3. **Switch Sessions** -- Click any session in the sidebar to activate it; metrics/chat will update to show the active session's server

4. **Manage Sessions** -- Delete sessions or change the active session as needed

## CLI Reference

### Monitor Mode Flags

| Flag | Description |
|------|-------------|
| `--headless` | Disable tray and desktop UI. Serve web/API only. |
| `--no-tray` | Skip tray icon but otherwise behave normally. |

### Server & Session Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--llama-server-path` | `-s` | `llama-server` | Path to `llama-server` binary (uses `$PATH` if bare name) |
| `--llama-server-cwd` | | `.` | Working directory for llama-server |
| `--port` | `-p` | `7778` | Monitor web UI port |
| `--presets-file` | | `~/.config/llama-monitor/presets.json` | Custom presets file location |
| `--sessions-file` | | `~/.config/llama-monitor/sessions.json` | Custom sessions file location |
| `--gpu-backend` | | `auto` | Force GPU backend: `auto`, `rocm`, `nvidia`, `none` |
| `--gpu-arch` | | (from config) | GPU architecture for ROCm (e.g. `gfx906`, `gfx1100`, `auto`) |
| `--gpu-devices` | | (from config) | Visible GPU device indices (e.g. `0,1,2,3`) |

All paths can also be configured from the web UI via the Configuration modal (gear icon). UI settings override CLI defaults and persist to `~/.config/llama-monitor/ui-settings.json`.

## Configuration

### Server Paths

The llama-server binary path and working directory can be set via:
1. **Web UI** -- Click the gear icon in the header to open Configuration
2. **CLI flags** -- `--llama-server-path` and `--llama-server-cwd`

UI settings take precedence over CLI defaults. Both are persisted across restarts.

### GPU Environment

GPU architecture and device selection are configurable via the **Configuration** modal (GPU Environment section), or via CLI flags. Settings are persisted to `~/.config/llama-monitor/gpu-env.json`.

On startup, the monitor auto-detects GPUs via `rocminfo` (AMD) or `nvidia-smi` (NVIDIA) and pre-selects the detected architecture.

### Presets

Presets store all llama-server parameters and are managed through the web UI (New / Edit / Copy / Delete). They are persisted to `~/.config/llama-monitor/presets.json`.

The preset editor groups parameters into collapsible sections:

- **Model & Memory** -- Model path (with file browser), GPU layers, no-mmap, mlock
- **Context & KV Cache** -- Context size, KV quantization (f16/q8_0/turbo3), flash attention
- **Batching & Slots** -- Batch size, micro-batch, parallel slots
- **GPU Distribution** -- Tensor split, split mode, main GPU
- **Threading** -- Generation and batch thread counts
- **Rope Scaling** -- YaRN/linear scaling, frequency base/scale
- **Speculative Decoding** -- ngram-mod, draft model, draft min/max
- **Advanced** -- Seed, system prompt file, extra CLI args

## Web UI

### Sidebar (Session Manager)
Lists all sessions with mode (Spawn/Attach), status (Running/Stopped/Disconnected), and port. Click to switch active session. Sessions persist to disk.

### Server Tab
Control bar with preset selector and port. Start/stop the server. Live inference metrics (prompt/generation speed, context usage, slot status) and GPU monitoring table (temperature, load, VRAM, power, clocks).

### Chat Tab
Streaming chat interface that proxies to the running llama-server's `/v1/chat/completions` endpoint on the active session's port. Supports reasoning/thinking blocks and Markdown rendering.

### Logs Tab
Real-time server log output.

## Architecture

```
src/
  main.rs              -- Entry point: CLI parsing, wiring, tokio::main
  cli.rs               -- Clap argument definitions
  config.rs            -- AppConfig resolved from CLI args
  state.rs             -- Shared AppState (Arc<Mutex<...>>), Sessions, UiSettings persistence
  gpu/
    mod.rs             -- GpuMetrics, GpuBackend trait, auto-detection
    rocm.rs            -- AMD ROCm via rocm-smi JSON
    nvidia.rs          -- NVIDIA via nvidia-smi CSV
    env.rs             -- GPU environment config, architecture table
    dummy.rs           -- No-op backend for headless/testing
  llama/
    metrics.rs         -- Prometheus text format parser
    server.rs          -- Subprocess management (start/stop), validation
    poller.rs          -- Async polling loop for /health, /metrics, /slots
  presets/
    mod.rs             -- ModelPreset, CRUD, file persistence
  models/
    mod.rs             -- GGUF file discovery and filename parsing
  web/
    mod.rs             -- Warp route composition
    api.rs             -- REST API handlers, file browser, chat proxy, session management
    ws.rs              -- WebSocket real-time metrics push
    static_assets.rs   -- Embedded frontend (include_str!)
static/
  index.html           -- Dashboard HTML
  style.css            -- Nord-themed CSS
  app.js               -- Frontend JavaScript with session management
  manifest.json        -- PWA manifest
  sw.js                -- Service worker
  icon.svg             -- App icon
```

### Data Flow

```
GPU (rocm-smi/nvidia-smi)  -->  GPU Poller (500ms)  --> AppState
llama-server /metrics       -->  Llama Poller (1s)   --> AppState
                                                          |
                                                     WebSocket (500ms)
                                                          |
                                                       Browser
```

Sessions are stored to disk every 30 seconds and loaded on startup.

## API Reference

### Capabilities Endpoint

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/capabilities` | Get metric capabilities and availability reasons |

**Response Schema:**

```json
{
  "capabilities": {
    "inference": true,
    "system": false,
    "gpu": false,
    "cpu_temperature": false,
    "memory": false,
    "host_metrics": false,
    "tray": true
  },
  "endpoint_kind": "local",
  "session_kind": "spawn",
  "tray_mode": "desktop",
  "availability": {
    "system": "remote_endpoint",
    "gpu": "remote_endpoint",
    "cpu_temp": "remote_endpoint"
  }
}
```

- **`capabilities`**: Which metrics are available for the active session
- **`endpoint_kind`**: `"local"` or `"remote"` (whether host metrics apply)
- **`session_kind`**: `"spawn"`, `"attach"`, or `"none"`
- **`tray_mode`**: `"desktop"`, `"headless"`, or `"failed"`
- **`availability`**: Reasons for metric unavailability

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Dashboard HTML |
| GET | `/ws` | WebSocket (real-time metrics push) |
| POST | `/api/start` | Start llama-server with `ServerConfig` JSON body |
| POST | `/api/stop` | Stop running llama-server |
| GET | `/api/presets` | List all presets |
| POST | `/api/presets` | Create a new preset |
| PUT | `/api/presets/{id}` | Update a preset |
| DELETE | `/api/presets/{id}` | Delete a preset |
| POST | `/api/presets/reset` | Reset presets to defaults |
| GET | `/api/settings` | Get persisted UI settings |
| PUT | `/api/settings` | Save UI settings |
| GET | `/api/sessions` | List all sessions |
| POST | `/api/sessions` | Create a new session |
| DELETE | `/api/sessions/{id}` | Delete a session |
| GET | `/api/sessions/active` | Get active session info |
| POST | `/api/sessions/active` | Set active session |
| POST | `/api/sessions/spawn` | Spawn server with preset (port, name, preset_id) |
| GET | `/api/browse?path=&filter=` | Browse filesystem (filter: `gguf`, `executable`) |
| GET | `/api/gpu-env` | Get GPU environment config |
| PUT | `/api/gpu-env` | Save GPU environment config |
| POST | `/api/chat?port=` | Streaming proxy to llama-server `/v1/chat/completions` |

## Development

```bash
# Run in debug mode
cargo run

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

### Project Structure

- Frontend files in `static/` are embedded at compile time via `include_str!`
- No Node.js or build tooling required
- Single binary deployment -- no external assets needed
- Session data persists to `~/.config/llama-monitor/sessions.json`

## License

MIT
