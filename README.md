# Llama Monitor

Web dashboard for managing [llama.cpp](https://github.com/ggerganov/llama.cpp) servers with real-time GPU and system monitoring. Supports local and remote deployments, multi-session management, and a lightweight agent mode for headless machines.

## Quick Start

```bash
# Download the latest release and run
./llama-monitor

# Open http://localhost:7778 in your browser
```

On first launch, you'll see the welcome screen to attach to an existing server or spawn a new one:

![Welcome Screen](docs/screenshots/01-welcome.png)

Configure your `llama-server` path and model directory in the web UI (gear icon), create a preset, and spawn or attach to a server.

## Modes of Operation

### Dashboard Mode (default)
Full web UI with session management, GPU/system monitoring, chat, and server controls.

### Agent Mode (`--agent`)
Lightweight remote metrics endpoint. Runs on headless machines and reports GPU + system metrics via HTTP. The dashboard polls the agent for real-time metrics on remote sessions.

```bash
# Run as a remote agent
./llama-monitor --agent --agent-host 0.0.0.0 --agent-port 7779

# With authentication
./llama-monitor --agent --agent-token "your-secret-token"
```

## Sessions

Manage multiple llama-server instances simultaneously:

| Mode | Description | Metrics |
|------|-------------|---------|
| **Spawn** | Starts a local llama-server on a configured port | Full: inference + GPU + system |
| **Attach** | Connects to an existing server at a URL | Inference only (or full if agent is running) |

Sessions persist to `~/.config/llama-monitor/sessions.json` and survive restarts. Old inactive sessions are auto-pruned after 7 days. Maximum 10 sessions at a time.

## Features

### Monitoring
- **GPU Metrics** — Temperature, load, VRAM, power, clock speeds (AMD ROCm, NVIDIA, Apple Silicon)
- **System Metrics** — CPU name, temperature, load, clock speed, RAM usage, motherboard model
- **Inference Metrics** — Prompt/generation speed, KV cache usage, slot status via Prometheus endpoint
- **Capability-Aware UI** — Shows available metrics with reasons for unavailability; sections hide automatically when not applicable

![Inference Metrics](docs/screenshots/02-inference-metrics.gif)

### Server Management
- **Spawn & Control** — Start/stop llama-server from the UI with configurable presets
- **Customizable Presets** — All llama.cpp parameters grouped into collapsible sections; persisted to disk
- **File Browser** — Browse filesystem for `llama-server` binary and `.gguf` model files
- **Auto-Discovery** — Models in the configured directory are discovered automatically

### Remote Agents
- **SSH-Based Management** — Detect, install, start, stop, update, and remove agents on remote machines
- **Auto-Start** — Attempts SSH autostart once when a remote agent becomes unreachable; if it fails, the header shows a Fix button to open the agent menu for manual intervention
- **Version Detection** — Compares installed version against latest release; update available indicator
- **Windows Task Scheduler** — Both the agent and sensor_bridge are installed as SYSTEM scheduled tasks, starting at boot with full hardware access and no console window
- **Status Alert** — The remote agent popup shows a status header indicating current health: connected, firewall blocked, or specific issues like missing sensor_bridge
- **Temperature Badge** — When CPU temperature is unavailable on a remote agent, a warning badge appears next to the temperature gauge with a tooltip explaining why
- **Cross-Platform** — Linux, macOS, and Windows support with automatic OS/arch detection

### Chat & Logs
- **Multi-Tab Chat** — Parallel conversations with per-tab persistence, rename, and close; Ctrl+1–9 and Ctrl+Shift+Arrow keyboard tab switching
- **System Prompts & Templates** — Customizable behavior with pre-built templates and policy management
- **Model Parameters** — Per-tab temperature, top_p, top_k, min_p, repeat_penalty, and max_tokens controls; active-params dot indicator when non-defaults are set
- **Streaming with Reasoning** — Real-time SSE streaming with thinking/reasoning block support; typing indicator while waiting for first token
- **Explicit Mode** — Toggle for uncensored content on models that require guardrail override
- **Syntax Highlighting** — Fenced code blocks highlighted via highlight.js (atom-one-dark theme); per-block header shows language, line count, and copy button; highlighting applied on finalized messages only (not during streaming)
- **Smart Scroll** — Auto-scroll only when already near the bottom; scroll-to-bottom button shows unread message count badge
- **Chat History Pagination** — Long conversations render only the most recent N messages (default 15) for performance; "Load More" button reveals older batches; limit is configurable per-tab
- **Token Count Display** — Input character count shows approximate token estimate (`~N tok`) with warning color at 800+ tokens and error color at 1500+
- **Personalized Empty State** — Greeting shows active AI name and loaded model name; suggested prompts grid with stagger animation
- **Animated Panels** — System prompt and model params panels open/close with smooth max-height transitions; send button shows spinner during generation

![Chat Interface](docs/screenshots/03-chat.png)

- **Real-Time Logs** — Live server log output in the UI (local sessions)

![Logs](docs/screenshots/05-logs.png)

### GPU & System Metrics
Local sessions show real-time hardware monitoring with sparkline graphs:

![GPU & System Metrics](docs/screenshots/04-gpu-metrics.gif)

### Desktop
- **System Tray** — Native tray icon (optional, disabled with `--headless` or `--no-tray`)
- **PWA Support** — Installable as a standalone app on mobile and desktop
- **Headless Mode** — Web/API server only, no tray or desktop UI
- **App Version Display** — Current version shown in the sidebar nav footer
- **Auto-Update** — Background update check on launch; update pill appears in the top nav when a new release is available. Click to open a release notes panel with one-click update:
  - **macOS / Linux** — Downloads the new binary, atomically replaces the running executable, and restarts the process. Browser reconnects automatically.
  - **Windows** — Downloads and extracts the new `.exe`, writes a detached batch helper to `%TEMP%`, then exits. The helper waits for the process to stop, copies the new binary in place, and relaunches it.

## Supported Hardware

| Vendor | Tool | Detection |
|--------|------|-----------|
| AMD | `rocm-smi` | Auto-detected |
| NVIDIA | `nvidia-smi` | Auto-detected |
| Apple Silicon | `mactop` | Auto-detected (Apple Silicon only) |
| Windows (CPU temp) | `sensor_bridge.exe` | Bundled with Windows release; auto-installed as scheduled task on remote agents |

GPU backend is auto-detected at startup. Override with `--gpu-backend apple|rocm|nvidia|none`.

## Installation

### Pre-built Binaries

Download the latest release from the [Releases page](../../releases/latest).

| Platform | File |
|----------|------|
| Linux x86_64 | `llama-monitor-linux-x86_64` |
| Linux aarch64 | `llama-monitor-linux-aarch64` |
| Windows x86_64 | `llama-monitor-windows-x86_64.zip` |
| macOS Apple Silicon | `llama-monitor-macos-aarch64.tar.gz` |

#### macOS (Apple Silicon)

```bash
tar -xzf llama-monitor-macos-aarch64.tar.gz
xattr -dr com.apple.quarantine ./llama-monitor-macos-aarch64
./llama-monitor-macos-aarch64
```

#### Linux

```bash
chmod +x llama-monitor-linux-x86_64
./llama-monitor-linux-x86_64
```

#### Windows

Extract the ZIP. The bundle includes `llama-monitor.exe` and `sensor_bridge.exe` (for CPU temperature via LibreHardwareMonitor).

When managed remotely via the dashboard, both the agent and sensor_bridge are installed as SYSTEM scheduled tasks that start at boot. The SSH user performing the install must be a local administrator. If CPU temperature shows as unavailable after install, the sensor_bridge may need to be started — the dashboard will indicate this in the remote agent popup.

### From Source

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/nmorgowicz-org/llama-monitor.git && cd llama-monitor
cargo build --release
```

The binary is at `target/release/llama-monitor` — a single self-contained executable with the frontend embedded at compile time.

### Dependencies

- **llama.cpp** — `llama-server` binary (with `--metrics` and `--jinja` support)
- **GPU monitoring** (optional):
  - AMD: `rocm-smi`
  - NVIDIA: `nvidia-smi`
  - Apple Silicon: `mactop` (`brew install mactop`)

## CLI Reference

### Server & Session Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--llama-server-path` | `-s` | `llama-server` | Path to `llama-server` binary |
| `--llama-server-cwd` | | `.` | Working directory for llama-server |
| `--models-dir` | `-m` | _(none)_ | Directory containing `.gguf` models |
| `--port` | `-p` | `7778` | Monitor web UI port |
| `--host` | | `127.0.0.1` | Bind address for web UI (use `0.0.0.0` for LAN) |
| `--basic-auth` | | _(none)_ | Enable HTTP Basic Auth (`user:password`) |
| `--presets-file` | | `~/.config/llama-monitor/presets.json` | Custom presets file path |
| `--sessions-file` | | `~/.config/llama-monitor/sessions.json` | Custom sessions file path |

### GPU Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--gpu-backend` | `auto` | Force GPU backend: `auto`, `rocm`, `nvidia`, `apple`, `none` |
| `--gpu-arch` | _(auto)_ | GPU architecture for ROCm (e.g. `gfx906`, `gfx1100`, `auto`) |
| `--gpu-devices` | _(all)_ | Visible GPU device indices (e.g. `0,1,2,3`) |

### Agent Mode Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--agent` | | Run as a lightweight remote metrics agent |
| `--agent-host` | `127.0.0.1` | Bind address for agent mode |
| `--agent-port` | `7779` | Port for agent mode |
| `--agent-token` | _(none)_ | Bearer token for agent authentication |

### Remote Agent Flags (Dashboard)

| Flag | Description |
|------|-------------|
| `--remote-agent-url` | Override remote agent URL for dashboard polling |
| `--remote-agent-token` | Bearer token for polling a remote agent |
| `--remote-agent-ssh-autostart` | Enable SSH autostart when agent is unreachable |
| `--remote-agent-ssh-target` | SSH target for autostart (e.g. `user@host`) |
| `--remote-agent-ssh-command` | Remote command to start the agent via SSH |

### Other Flags

| Flag | Description |
|------|-------------|
| `--headless` | Disable tray and desktop UI; serve web/API only |
| `--no-tray` | Skip tray icon but otherwise behave normally |
| `--llama-poll-interval` | Llama metrics polling interval in seconds (default: 1) |

## Configuration

All settings can be configured from the web UI (gear icon) or via CLI flags. UI settings take precedence and persist to disk.

### Persisted Files

| File | Purpose |
|------|---------|
| `~/.config/llama-monitor/sessions.json` | Session definitions (spawn/attach mode, ports, endpoints) |
| `~/.config/llama-monitor/presets.json` | Model presets with all llama.cpp parameters |
| `~/.config/llama-monitor/ui-settings.json` | Web UI preferences (paths, ports, presets, agent settings) |
| `~/.config/llama-monitor/gpu-env.json` | GPU environment config (architecture, device indices) |
| `~/.config/llama-monitor/chat-tabs.json` | Chat tab histories, system prompts, per-tab model parameters |

Session data is saved every 30 seconds and on explicit save.

### Presets

The preset editor groups parameters into collapsible sections:

- **Model & Memory** — Model path (with file browser), GPU layers, no-mmap, mlock
- **Context & KV Cache** — Context size, KV quantization, flash attention
- **Batching & Slots** — Batch size, micro-batch, parallel slots
- **GPU Distribution** — Tensor split, split mode, main GPU
- **Threading** — Generation and batch thread counts
- **Rope Scaling** — YaRN/linear scaling, frequency base/scale
- **Speculative Decoding** — ngram-mod, draft model, draft min/max
- **Advanced** — Seed, system prompt file, extra CLI args

## Web UI

### Sidebar (Session Manager)
Lists all sessions with mode (Spawn/Attach), status (Running/Stopped/Disconnected), and port. Click to switch active session.

### Server Tab
Control bar with preset selector and port. Start/stop the server. Live inference metrics and GPU/system monitoring tables.

### Chat Tab
Multi-tab streaming chat proxied to the running llama-server's `/v1/chat/completions` endpoint. Features include:
- Per-tab system prompts with template library
- Model parameter controls (temperature, top_p, top_k, min_p, repeat_penalty); dirty-state indicator when non-defaults are active
- Reasoning/thinking blocks, Markdown rendering, and syntax-highlighted code blocks (highlight.js)
- Per-code-block headers: language label, line count, and copy button
- Chat history pagination with configurable visible-message limit (default 15) — older messages load on demand
- Token count estimate on input with color warnings at 800+ and 1500+ tokens
- Keyboard tab switching: Ctrl+1–9 by position, Ctrl+Shift+← / → to cycle
- Explicit mode toggle for uncensored content

### Logs Tab
Real-time server log output.

## API Reference

### Capabilities

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/capabilities` | Metric capabilities and availability reasons |

### Server & Sessions

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/start` | Start llama-server with preset |
| POST | `/api/stop` | Stop running llama-server |
| GET | `/api/sessions` | List all sessions |
| POST | `/api/sessions` | Create a new session |
| DELETE | `/api/sessions/{id}` | Delete a session |
| GET | `/api/sessions/active` | Get active session |
| POST | `/api/sessions/active` | Set active session |
| POST | `/api/sessions/spawn` | Spawn server with preset |

### Presets & Settings

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/presets` | List all presets |
| POST | `/api/presets` | Create a preset |
| PUT | `/api/presets/{id}` | Update a preset |
| DELETE | `/api/presets/{id}` | Delete a preset |
| POST | `/api/presets/reset` | Reset to defaults |
| GET | `/api/settings` | Get UI settings |
| PUT | `/api/settings` | Save UI settings |

### GPU & System

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/gpu-env` | Get GPU environment config |
| PUT | `/api/gpu-env` | Save GPU environment config |
| GET | `/api/browse?path=&filter=` | Browse filesystem (`gguf`, `executable`) |

### Chat

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/chat` | Streaming SSE proxy to active session's `/v1/chat/completions` |

### App Updates

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/remote-agent/releases/latest` | Latest GitHub release info (`tag_name`, `body`, `assets`) |
| POST | `/api/self-update` | Download latest release and replace the running binary; returns `{ ok, tag_name, restart_required }` |

### WebSocket

| Method | Path | Description |
|--------|------|-------------|
| WS | `/ws` | Real-time metrics push |

## Architecture

```
src/
  main.rs              -- Entry point: CLI, wiring, poller threads, tokio runtime
  cli.rs               -- Clap argument definitions
  config.rs            -- AppConfig resolved from CLI args
  state.rs             -- Shared AppState, Sessions, UiSettings, persistence
  agent.rs             -- Remote metrics agent server + polling + SSH management
  remote_ssh.rs        -- SSH connection handling, command execution, file transfer
  certs.rs             -- Self-signed TLS certificate generation (rcgen)
  lhm.rs               -- LibreHardwareMonitor sensor bridge (Windows CPU temp)
  lhm_persistence.rs   -- LHM scheduled-task state persistence
  system.rs            -- Cross-platform system metrics (CPU, RAM, motherboard)
  tray.rs              -- System tray (native-tray feature)
  gpu/
    mod.rs             -- GpuMetrics, GpuBackend trait, auto-detection
    rocm.rs            -- AMD ROCm via rocm-smi JSON
    nvidia.rs          -- NVIDIA via nvidia-smi CSV
    apple.rs           -- Apple Silicon via mactop
    env.rs             -- GPU environment config, architecture table
    dummy.rs           -- No-op backend for headless/testing
  llama/
    metrics.rs         -- Prometheus text format parser
    server.rs          -- Subprocess management (start/stop/kill)
    poller.rs          -- Async polling loop for /health, /metrics, /slots
  system/
    poller.rs          -- System metrics polling thread (CPU, RAM, temp)
  presets/
    mod.rs             -- ModelPreset, CRUD, file persistence
  models/
    mod.rs             -- GGUF file discovery and filename parsing
  web/
    mod.rs             -- Warp route composition, CSP headers, basic auth
    api.rs             -- REST API handlers (1400+ lines)
    ws.rs              -- WebSocket real-time metrics push (server → client)
    static_assets.rs   -- Embedded frontend assets (include_str! at compile time)
static/
  index.html           -- Dashboard HTML (single-page app)
  app.js               -- Frontend JavaScript (~8600 lines, vanilla JS)
  compact.html         -- Compact tray popover view
  manifest.json        -- PWA manifest
  sw.js                -- Service worker (PWA offline support)
  lhm.js               -- LibreHardwareMonitor frontend integration
  icon.svg             -- Application icon
  css/                 -- Stylesheet modules (split for AI agent readability)
    tokens.css         -- CSS custom properties, light theme variable overrides
    base.css           -- Reset, body, typography, element defaults
    layout.css         -- Health strip, nav bar, sidebar, page/content layout
    cards-inference.css -- Inference metric cards (speed, context, generation, activity rail)
    agent-modal.css    -- Remote agent setup modal
    cards-hardware.css -- GPU/system hardware cards, ring/sparkline/chip visualizations
    components.css     -- Buttons, modal shell, forms, models list
    chat.css           -- Chat interface and toast notifications
    setup-view.css     -- Setup/welcome view, analytics, shortcuts, responsive breakpoints
    settings-modal.css -- Premium settings modal, form controls, light theme overrides
sensor_bridge/
  Program.cs           -- .NET sensor bridge for Windows CPU temperature (LibreHardwareMonitor)
  sensor_bridge.csproj -- .NET project file
scripts/
  build-release-targets.sh  -- Multi-platform release build
  build-single-target.sh    -- Single-target build helper
  release-preflight.sh      -- Pre-release checks
tests/
  integration/
    capabilities.rs    -- Capability detection integration tests
  ui/                  -- Playwright UI test suite
    *.spec.js          -- End-to-end UI tests
    screenshot.mjs     -- Screenshot automation
    gif.mjs            -- GIF capture for docs
docs/
  api.md                          -- REST API reference
  cli-flags.md                    -- CLI flag reference
  cross-compilation.md            -- Multi-platform build guide
  20260426-security_hardening.md  -- Security audit findings and remediation status
  20260427-chat_enhancements.md   -- Chat UI overhaul implementation plan
  (+ additional design and implementation docs)
```

### CSS Module Index

Each CSS file has a `CONTAINS:` comment at the top listing its key selectors. To find where a class is styled:

```bash
grep -r "\.your-class" static/css/
```

| File | What to open it for |
|------|---------------------|
| `tokens.css` | Changing colors, spacing, font, radius, shadow variables |
| `base.css` | Body background, typography scale, element resets |
| `layout.css` | Navigation, sidebar, health strip, page structure, dashboard grid |
| `cards-inference.css` | Speed/context/generation metric cards, activity rail, KV arc |
| `agent-modal.css` | Remote agent setup flow modal |
| `cards-hardware.css` | GPU/system metric cards, ring gauges, sparklines, chip bars |
| `components.css` | Buttons, general modals, form fields, models list |
| `chat.css` | Chat message thread, input area, toast notifications |
| `setup-view.css` | Setup/welcome screen, shortcuts overlay, responsive breakpoints |
| `settings-modal.css` | Settings modal internals, toggles, checkboxes, light theme |

### Data Flow

```
GPU (rocm-smi/nvidia-smi/mactop)  -->  GPU Poller (500ms)  --> AppState
System (sysinfo/sensors)           -->  System Poller (5s)  --> AppState
llama-server /metrics              -->  Llama Poller (1s)   --> AppState
Remote Agent /metrics              -->  Agent Poller (2s)   --> AppState
                                                              |
                                                         WebSocket (500ms)
                                                              |
                                                           Browser
```

## Development

```bash
cargo run              # Debug mode
cargo test             # Run tests
cargo clippy -- -D warnings  # Lint
cargo fmt              # Format
cargo build --release  # Production binary
```

- Frontend files in `static/` are embedded at compile time via `include_str!`
- No Node.js or build tooling required
- Single binary deployment — no external assets needed

## License

MIT
