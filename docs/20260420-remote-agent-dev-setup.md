# Remote Agent Development Setup

This is the current practical setup for testing remote host metrics from a Mac dashboard against a Windows 11 llama-server host.

## Network Layout

- MacBook: runs the full llama-monitor dashboard on port `7778`.
- Windows 11 machine: runs `llama-server` on `192.0.2.16:8001`.
- Windows 11 machine: runs `llama-monitor --agent` on port `7779`.

When the dashboard is attached to `http://192.0.2.16:8001`, it automatically tries to poll:

```text
http://192.0.2.16:7779/metrics
```

## Start The Windows Agent

On the Windows machine:

```powershell
.\llama-monitor.exe --agent --agent-host 0.0.0.0 --agent-port 7779
```

## SSH Autostart

If SSH works from the Mac to the Windows host, the dashboard can try to start the remote agent when it is unreachable.

The preferred remote install path is:

```text
%APPDATA%\llama-monitor\bin\llama-monitor.exe
```

For macOS/Linux remote hosts, the preferred path is:

```text
~/.config/llama-monitor/bin/llama-monitor
```

In Configuration -> Remote Agent:

```text
Start over SSH if unreachable: checked
SSH Target: user@example-host
SSH Start Command:
```

Leave `SSH Start Command` empty to use the OS-specific default. The dashboard detects Windows vs. macOS/Linux over SSH and starts the binary from the config-bin path.

On Windows, the default uses Task Scheduler (`schtasks`) instead of `start` because OpenSSH can terminate child console processes when the SSH session exits.

For a dev checkout that has not installed the binary yet, use an override:

```text
start "" /B cmd /C "cd /d C:\Users\nick\llama-monitor-agent-test && cargo run -- --agent --agent-host 0.0.0.0 --agent-port 7779 --gpu-backend nvidia"
```

This exposes:

```text
GET /health
GET /metrics
GET /metrics/system
GET /metrics/gpu
```

For first-pass home-network development, the token is optional. To require a bearer token:

```powershell
.\llama-monitor.exe --agent --agent-host 0.0.0.0 --agent-port 7779 --agent-token "replace-with-a-long-random-token"
```

Then start the Mac dashboard with:

```bash
cargo run -- --remote-agent-token "replace-with-a-long-random-token"
```

## Attach From The Mac Dashboard

In the dashboard, attach to:

```text
http://192.0.2.16:8001
```

If the agent is reachable, the top strip shows `Remote Agent` and System/GPU sections are populated from the Windows host. If it is not reachable, the dashboard remains inference-only.

## Windows Firewall

Allow inbound TCP on:

- `8001` for `llama-server`
- `7779` for the llama-monitor remote metrics agent

Keep `7779` restricted to the private home network.
