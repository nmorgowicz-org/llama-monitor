# CLI Flags Reference

All CLI flags can be viewed by running:

```bash
llama-monitor --help
```

---

## Core Configuration

### `--port`, `-p` (default: 7778)

**Type:** Integer  
**Description:** Port for the monitor web UI

```bash
llama-monitor --port 8000
llama-monitor -p 9999
```

### `--llama-server-path`, `-s`

**Type:** Path  
**Description:** Path to the llama-server binary

```bash
llama-monitor --llama-server-path /usr/local/bin/llama-server
```

### `--llama-server-cwd`

**Type:** Path  
**Description:** Working directory for llama-server

```bash
llama-monitor --llama-server-cwd /home/user/models
```

### `--models-dir`, `-m`

**Type:** Path  
**Description:** Directory containing .gguf model files for auto-discovery

```bash
llama-monitor --models-dir /home/user/models
```

### `--presets-file`

**Type:** Path  
**Description:** Path to presets JSON file

```bash
llama-monitor --presets-file ~/.config/llama-monitor/presets-custom.json
```

### `--sessions-file`

**Type:** Path  
**Description:** Path to sessions JSON file

```bash
llama-monitor --sessions-file ~/.config/llama-monitor/sessions-custom.json
```

---

## GPU Configuration

### `--gpu-backend` (default: "auto")

**Type:** String  
**Description:** GPU monitoring backend: auto, rocm, nvidia, apple, none

Available backends:
- `auto` - Auto-detect based on system
- `rocm` - AMD ROCm (Linux)
- `nvidia` - NVIDIA CUDA (Linux/Windows)
- `apple` - Apple Silicon (macOS)
- `none` - Disable GPU monitoring

```bash
llama-monitor --gpu-backend rocm
llama-monitor --gpu-backend apple
llama-monitor --gpu-backend none
```

### `--gpu-arch`

**Type:** String  
**Description:** GPU architecture for ROCm environment (e.g. gfx906, gfx1100, auto)

```bash
llama-monitor --gpu-arch gfx1100
llama-monitor --gpu-arch auto
```

### `--gpu-devices`

**Type:** String  
**Description:** Visible GPU device indices (comma-separated, e.g. 0,1,2,3)

```bash
llama-monitor --gpu-devices 0,1
llama-monitor --gpu-devices 2,3
```

---

## Polling Configuration

### `--llama-poll-interval` (default: 1)

**Type:** Integer  
**Description:** Llama metrics polling interval in seconds

```bash
llama-monitor --llama-poll-interval 5
```

---

## Headless/Tray Configuration

### `--headless`

**Type:** Boolean  
**Description:** Run in headless mode (no tray, no desktop UI)

Use when running on a headless server or when you want to run the server without the desktop interface.

```bash
llama-monitor --headless
```

### `--no-tray`

**Type:** Boolean  
**Description:** Disable tray icon (override automatic detection)

Use when you want to prevent the tray icon from appearing, even on systems where it would normally be available.

```bash
llama-monitor --no-tray
```

---

## Remote Agent Configuration

### `--agent`

**Type:** Boolean  
**Description:** Run as a lightweight remote metrics agent instead of the full dashboard

When enabled, llama-monitor runs as a metrics collection service that can be queried by other tools. This mode is designed for remote monitoring scenarios.

```bash
llama-monitor --agent
```

### `--agent-host` (default: "127.0.0.1")

**Type:** String  
**Description:** Host/interface for remote metrics agent mode

```bash
llama-monitor --agent-host 0.0.0.0
```

### `--agent-port` (default: 7779)

**Type:** Integer  
**Description:** Port for remote metrics agent mode

```bash
llama-monitor --agent-port 8888
```

### `--agent-token`

**Type:** String  
**Description:** Optional bearer token required by remote metrics agent mode

```bash
llama-monitor --agent --agent-token "your-secret-token"
```

---

## Remote Agent Connection (Dashboard Mode)

When running the full dashboard (without `--agent`), these flags configure how it connects to a remote metrics agent:

### `--remote-agent-url`

**Type:** String  
**Description:** Override remote agent URL used by dashboard polling

```bash
llama-monitor --remote-agent-url http://192.168.1.100:7779
```

### `--remote-agent-token`

**Type:** String  
**Description:** Optional bearer token used when polling a remote metrics agent

```bash
llama-monitor --remote-agent-token "your-secret-token"
```

### `--remote-agent-ssh-autostart`

**Type:** Boolean  
**Description:** Enable SSH autostart when a remote metrics agent is unreachable

```bash
llama-monitor --remote-agent-ssh-autostart
```

### `--remote-agent-ssh-target`

**Type:** String  
**Description:** SSH target used to autostart remote metrics agent (e.g. user@host)

```bash
llama-monitor --remote-agent-ssh-target user@192.168.1.100
```

### `--remote-agent-ssh-command`

**Type:** String  
**Description:** Remote command run over SSH to start the metrics agent

```bash
llama-monitor --remote-agent-ssh-command "systemctl start llama-agent"
```

---

## Examples

### Local Development

```bash
llama-monitor \
  --port 8000 \
  --models-dir ~/models \
  --llama-server-path ~/llama.cpp/server/llama-server
```

### Headless Server

```bash
llama-monitor \
  --headless \
  --port 8000 \
  --llama-server-path /opt/llama-server
```

### Remote Agent

```bash
llama-monitor \
  --agent \
  --agent-host 0.0.0.0 \
  --agent-port 7779
```

### AMD ROCm GPU Monitoring

```bash
llama-monitor \
  --gpu-backend rocm \
  --gpu-arch gfx906 \
  --gpu-devices 0,1,2,3
```

### Apple Silicon

```bash
llama-monitor \
  --gpu-backend apple
```

---

## Environment Variables

Some settings can also be configured via environment variables:

| Variable | Flag Equivalent | Description |
|----------|-----------------|-------------|
| `LLAMA_MONITOR_PORT` | `--port` | Web UI port |
| `LLAMA_MONITOR_GPU_BACKEND` | `--gpu-backend` | GPU monitoring backend |
| `LLAMA_MONITOR_HEADLESS` | `--headless` | Headless mode |

---

## API Endpoints

The remote metrics agent exposes these endpoints:

- `GET /metrics` - Raw metrics in Prometheus format
- `GET /health` - Health check endpoint
- `GET /capabilities` - Available metrics and reasons

See `docs/20260420-remote-agent-api.md` for more details.

---

**Last Updated**: 2026-04-20
