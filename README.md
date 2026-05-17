# Llama Monitor

Monitoring-first web dashboard for [llama.cpp](https://github.com/ggml-org/llama.cpp) servers. It tracks inference throughput, context pressure, GPU/system telemetry, remote-agent health, and multi-session state in one UI, with chat tools layered on top when you want to work from the same surface.

## Quick Start

```bash
./llama-monitor
# Open http://localhost:7778
```

## Features

### Live Monitoring Cockpit

The top nav and Server tab show throughput, context pressure, request activity, slot state, and model/runtime details as your endpoint works. Local sessions read host telemetry directly; remote sessions gain the same depth through the remote agent.

![Inference Metrics](docs/screenshots/inference-metrics.gif)

### Remote Host Telemetry

Attach to a remote llama.cpp server, then add the companion remote agent when you need GPU, CPU, RAM, and host-health metrics from that machine. The header Agent flow and runtime configuration panel handle install, start, update, and repair actions.

![GPU & System Metrics](docs/screenshots/dashboard-gpu-section.png)

### Multi-Session Chat Workspace

Chat tabs, prompt controls, telemetry overlays, and logs live next to the monitoring dashboard so you can inspect behavior and interact with the model from the same app.

![Chat Interface](docs/screenshots/chat-chat.png)

### Guided Generation

A per-tab notes sidebar, AI-generated suggestions, quick guide flows, and director/surprise tools let you steer the next reply without rebuilding the whole prompt stack.

![Suggestions Results](docs/screenshots/guided-gen-suggestions-results.png)

### Personas & Prompt Tooling

Manage built-in and custom personas, explicit-policy variants, and outbound prompt inspection from the same workspace.

![Prompt Debug Inspector](docs/screenshots/panels-prompt-debug.png)

### TLS, ACME & mTLS

Built-in TLS with ACME (Let’s Encrypt) and mTLS for remote agents. Choose No HTTPS, Self-Signed, Bring Your Own Key, or fully automated ACME with DNS-01 and renewal. Optional, flexible, and designed for local-first and production deployments.

![Security & Certificates](docs/screenshots/tls-certificates-tab.png)

---

**Monitoring reference**: [Dashboard Capabilities](docs/reference/dashboard.md)  
**Remote telemetry setup**: [Remote Agent](docs/reference/remote-agent.md)  
**Chat and guided generation**: [Chat](docs/reference/chat.md)  
**TLS / ACME / mTLS**: [TLS Architecture](docs/reference/tls-architecture.md)

## Supported Hardware

| Vendor | Tool | Detection |
|--------|------|-----------|
| AMD | `rocm-smi` | Auto-detected |
| NVIDIA | `nvidia-smi` | Auto-detected |
| Apple Silicon | `mactop` | Auto-detected |
| Windows (CPU temp) | `sensor_bridge.exe` | Bundled |

## Installation

Pre-built binaries are available on the [latest release](../../releases/latest). To build from source:

```bash
git clone https://github.com/nmorgowicz-org/llama-monitor.git
cd llama-monitor
cargo build --release
```

## Documentation

- [Dashboard Capabilities](docs/reference/dashboard.md) — Monitoring surfaces, telemetry, refresh behavior
- [Remote Agent](docs/reference/remote-agent.md) — Remote host telemetry, SSH setup, agent lifecycle
- [Chat](docs/reference/chat.md) — Chat tabs, personas, guided generation, prompt tooling
- [Real-Time Communication](docs/reference/realtime-communication.md) — WebSocket schema, polling, network detection
- [API Reference](docs/reference/api.md) — REST endpoints
- [CLI Reference](docs/reference/cli-flags.md) — Supported flags
- [Cross-Compilation](docs/reference/cross-compilation.md) — Build targets and toolchains
- [Capability Flags](docs/reference/capabilities.md) — Metric capability system

## Development

```bash
cargo run
cargo test
cargo clippy -- -D warnings
cargo fmt
cargo build --release
```

Frontend assets under `static/` are embedded at compile time. There is no Node build step for the shipped app, but the repo uses Node-based tooling for linting, UI tests, and screenshot capture.

## License

MIT
