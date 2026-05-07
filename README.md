# Llama Monitor

Web dashboard for managing [llama.cpp](https://github.com/ggerganov/llama.cpp) servers with real-time GPU and system monitoring. Supports local and remote deployments, multi-session management, and a lightweight agent mode for headless machines.

## Quick Start

```bash
# Download the latest release and run
./llama-monitor

# Open http://localhost:7778 in your browser
```

On first launch, attach to an existing server or spawn a new one:

![Welcome Screen](docs/screenshots/01-welcome.png)

## Modes of Operation

| Mode | Description |
|------|-------------|
| **Dashboard** (default) | Full web UI with session management, GPU/system monitoring, chat, and server controls |
| **Agent** (`--agent`) | Lightweight remote metrics endpoint for headless machines |

```bash
./llama-monitor --agent --agent-host 0.0.0.0 --agent-port 7779
```

## Features

### Monitoring

- **GPU Metrics** — Temperature, load, VRAM, power, clock speeds (AMD ROCm, NVIDIA, Apple Silicon)
- **System Metrics** — CPU, temperature, load, RAM, motherboard model
- **Inference Metrics** — Throughput, slot status, context window, generation progress
- **Context Window Card** — Live gauge across all active chats with per-chat usage
- **Capability-Aware UI** — Top nav status pill reflects live telemetry level

![Inference Metrics](docs/screenshots/02-inference-metrics.gif)

### Server Management

- Spawn/stop llama-server from the UI with configurable presets
- File browser for binary and model discovery
- All llama.cpp parameters grouped into collapsible preset sections

### Remote Agents

- SSH-based install, start, stop, update, and removal
- Version detection with one-click in-place auto-update
- Windows Task Scheduler for boot-start with full hardware access
- Cross-platform: Linux, macOS, Windows

### Chat

- Multi-tab with pin, rename, keyboard switching (Ctrl+1-9, Ctrl+Shift+Arrow)
- System prompts, persona templates, per-tab model parameters
- Streaming with reasoning blocks, syntax highlighting, context compaction
- Token count estimates, history pagination, export, message edit/regenerate

![Chat Interface](docs/screenshots/03-chat.png)

### GPU & System Metrics

Real-time hardware monitoring with sparkline graphs:

![GPU & System Metrics](docs/screenshots/04-gpu-metrics.gif)

### Desktop

- System tray (optional, `--headless` or `--no-tray` to disable)
- PWA support, headless mode, auto-update with one-click in-place upgrade

## Supported Hardware

| Vendor | Tool | Detection |
|--------|------|-----------|
| AMD | `rocm-smi` | Auto-detected |
| NVIDIA | `nvidia-smi` | Auto-detected |
| Apple Silicon | `mactop` | Auto-detected |
| Windows (CPU temp) | `sensor_bridge.exe` | Bundled with release |

## Installation

Pre-built binaries on the [Releases page](../../releases/latest):

| Platform | File |
|----------|------|
| Linux x86_64 | `llama-monitor-linux-x86_64` |
| Linux aarch64 | `llama-monitor-linux-aarch64` |
| Windows x86_64 | `llama-monitor-windows-x86_64.zip` |
| macOS Apple Silicon | `llama-monitor-macos-aarch64.tar.gz` |

### From Source

```bash
git clone https://github.com/nmorgowicz-org/llama-monitor.git && cd llama-monitor
cargo build --release
```

Single self-contained binary at `target/release/llama-monitor`.

## Documentation

- [Dashboard Capabilities](docs/reference/dashboard.md) — Metrics, monitoring, and hardware support
- [Remote Agent](docs/reference/remote-agent.md) — Headless deployment, SSH management, auto-update
- [Chat](docs/reference/chat.md) — Multi-tab chat, telemetry, context compaction
- [Real-Time Communication](docs/reference/realtime-communication.md) — WebSocket schema, polling intervals, network detection
- [API Reference](docs/reference/api.md) — REST endpoints
- [Capabilities](docs/reference/capabilities.md) — Metric capability flags
- [CLI Reference](docs/reference/cli-flags.md) — All flags and options
- [Cross-Compilation](docs/reference/cross-compilation.md) — Build targets and toolchains

## Development

```bash
cargo run              # Debug mode
cargo test             # Run tests
cargo clippy -- -D warnings  # Lint
cargo fmt              # Format
cargo build --release  # Production binary
```

Frontend in `static/` is embedded at compile time. No Node.js build step for the backend.

## License

MIT
