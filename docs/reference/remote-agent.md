# Remote Agent

The remote agent extends Llama Monitor's telemetry to headless machines and remote servers. A lightweight agent process runs on the remote host and reports GPU, system, and inference metrics back to the dashboard.

## What the Agent Does

When you attach to a remote llama.cpp server without an agent, the dashboard only receives inference metrics from the `/metrics` and `/slots` endpoints. The agent adds:

- **GPU metrics** — Temperature, utilization, VRAM, power, clock speeds
- **System metrics** — CPU load, temperature, RAM, motherboard model
- **Health reporting** — Agent connectivity status, firewall detection, version info

With the agent running, a remote session shows **Full telemetry** (green status pill), identical to a local session.

## Architecture

```
Dashboard (your machine)                          Remote Agent (headless server)
                        ║
                        ║  HTTP polling (configurable interval)
                        ║
        ┌───────────────┘
        │
        ▼
   /metrics     Agent exposes combined metrics endpoint
   /slots       GPU + system + inference data in Prometheus format
   /health      Agent health and version info
```

The dashboard polls the agent's HTTP endpoint on a configurable interval. The agent collects data from local tools (`nvidia-smi`, `rocm-smi`, `mactop`, sysinfo) and forwards it.

## SSH-Based Management

The dashboard can manage the agent lifecycle on remote machines over SSH:

| Action | Description |
|--------|-------------|
| **Detect** | Tests SSH connectivity and checks if agent is installed |
| **Install** | Downloads the correct binary for the remote OS/arch and places it |
| **Start** | Launches the agent process in the background |
| **Stop** | Gracefully stops the agent |
| **Update** | Downloads and replaces the agent binary with the latest version |
| **Remove** | Uninstalls the agent and cleans up scheduled tasks |

### SSH Setup Flow

1. Open Settings > Advanced > Remote Agent
2. Enter SSH target (`user@host`) and optionally a custom command
3. The dashboard builds a structured target without contacting the host
4. Guided setup walks through install, start, and verification

### Auto-Start

When a remote agent becomes unreachable, the dashboard attempts SSH autostart once. If it fails, a Fix button appears in the header to open the agent menu for manual intervention.

## Windows Support

On Windows, both the agent and `sensor_bridge.exe` are installed as SYSTEM scheduled tasks:

- Start at boot with no console window
- Full hardware access for GPU and CPU temperature sensors
- No user login required

The SSH user performing the install must be a local administrator.

## Version Detection

The agent reports its installed version to the dashboard. The dashboard compares against the latest release and shows an "Update available" indicator when a newer version exists. Click the indicator to upgrade in-place.

## Configuration

| Flag | Default | Description |
|------|---------|-------------|
| `--agent` | | Run as a remote metrics agent |
| `--agent-host` | `127.0.0.1` | Bind address |
| `--agent-port` | `7779` | HTTP port |
| `--agent-token` | _(none)_ | Bearer token for authentication |

On the dashboard side:

| Flag | Description |
|------|-------------|
| `--remote-agent-url` | Override agent URL for polling |
| `--remote-agent-token` | Bearer token for agent authentication |
| `--remote-agent-ssh-autostart` | Enable SSH autostart when unreachable |
| `--remote-agent-ssh-target` | SSH target for autostart (`user@host`) |
| `--remote-agent-ssh-command` | Custom remote command to start the agent |

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Status pill yellow (Inference only) | Agent not running or unreachable | Start agent on remote host |
| Status pill orange (Limited) | Agent connected but sensors unavailable | Check GPU tools are installed |
| Temperature unavailable on Windows | `sensor_bridge.exe` not running | Dashboard shows Fix button to start it |
| "Firewall blocked" | Agent started but HTTP port blocked | Open agent port in firewall |
| Update available | Installed version behind latest | Click indicator to upgrade |

## Benefits

- **Full remote visibility** — Same metrics as local, no SSH tunneling needed
- **Lightweight** — Single binary, no Docker, minimal resource usage
- **Self-updating** — Version detection and one-click upgrade from the dashboard
- **Cross-platform** — Linux, macOS, Windows with automatic OS/arch detection
- **Boot persistence** — Scheduled tasks on Windows, systemd on Linux, launchd on macOS
