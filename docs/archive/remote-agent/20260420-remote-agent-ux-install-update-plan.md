# Remote Agent UX, Install, And Update Plan

Date: 2026-04-20

## Goal

Make remote host telemetry feel like a normal part of the web app, not a manual sidecar setup.

When a user attaches to a remote `llama-server`, the app should:

1. infer the likely remote metrics agent URL,
2. detect whether the agent is reachable,
3. use SSH to detect the remote OS/architecture,
4. install the matching release binary when needed,
5. start the remote agent,
6. show clear firewall or SSH failures,
7. offer update when the installed agent is older than the latest release.

## Current Release Assets

Latest inspected release: `v0.5.1`

Assets:

| Platform | Architecture | Asset |
|---|---:|---|
| Windows | x86_64 | `llama-monitor-windows-x86_64.exe` |
| Linux | x86_64 | `llama-monitor-linux-x86_64` |
| Linux | aarch64 | `llama-monitor-linux-aarch64` |
| macOS | aarch64 | `llama-monitor-macos-aarch64.tar.gz` |

The macOS artifact is intentionally archived because the executable bit can be lost when uploading/downloading a raw binary. The installer must download the `.tar.gz`, extract it, copy the contained `llama-monitor` binary, and run `chmod +x` after installation.

## Remote Install Locations

Use config-local paths on the remote machine:

| OS | Install Path |
|---|---|
| Windows | `%APPDATA%\llama-monitor\bin\llama-monitor.exe` |
| Linux/macOS | `~/.config/llama-monitor/bin/llama-monitor` |

The dashboard running on the local machine should never assume its own binary can run remotely. It must install a binary matching the remote OS and architecture.

## UX Design

Add a Remote Agent panel in Settings with these states:

| State | Meaning | Primary Action |
|---|---|---|
| `Unknown` | Host has not been checked | Detect |
| `SSH unavailable` | Cannot SSH to remote host | Edit SSH Target |
| `Unsupported platform` | No matching release asset | Manual setup |
| `Not installed` | SSH works but no agent binary found | Install Agent |
| `Installed, stopped` | Binary exists but `/health` unreachable | Start Agent |
| `Running` | `/health` and `/metrics` work | Update / Restart |
| `Firewall blocked` | Start succeeded but LAN HTTP cannot reach agent | Show firewall guidance |
| `Update available` | Installed version older than latest release | Update Agent |

Primary fields:

- llama-server endpoint
- inferred agent URL
- SSH target
- agent port
- auth token
- auto-start toggle
- auto-update notification toggle

Advanced fields:

- custom agent URL
- custom install path
- custom start command
- custom release URL
- GPU backend override

## Backend API Checklist

- [x] `GET /api/remote-agent/releases/latest`
  - Query latest GitHub release.
  - Return tag and supported assets.
  - Cache result briefly to avoid repeated GitHub calls.

- [x] `POST /api/remote-agent/detect`
  - Input: `ssh_target`, optional `agent_url`.
  - Detect remote OS with:
    - Windows: `ssh target cmd.exe /C ver`
    - Unix: `ssh target uname -s`
  - Detect architecture:
    - Windows: `%PROCESSOR_ARCHITECTURE%`
    - Unix: `uname -m`
  - Map to release asset.
  - Check remote install path.
  - Check agent `/health`.

- [ ] `POST /api/remote-agent/install`
  - Detect platform.
  - Download matching release asset locally.
  - If asset is `.tar.gz`, extract it locally before upload.
  - Copy via `scp` to a temp remote path.
  - Move into config-bin path.
  - Set executable bit on Unix.
  - Verify binary exists.

- [ ] `POST /api/remote-agent/start`
  - Start installed binary using OS-specific logic.
  - Windows default: Task Scheduler one-shot task.
  - Unix default: `nohup ... &`.
  - Poll `/health` after start.
  - Return firewall-specific guidance if SSH start succeeded but HTTP failed.

- [ ] `POST /api/remote-agent/update`
  - Compare local latest release with remote `llama-monitor --version`.
  - Stop running agent if needed.
  - Install latest asset.
  - Restart agent.
  - Verify `/metrics`.

- [ ] `POST /api/remote-agent/stop`
  - Windows: `taskkill /IM llama-monitor.exe /F`.
  - Unix: track pidfile or `pkill -f`.

## Frontend Checklist

- [ ] Replace the current simple Remote Agent settings block with a status panel.
- [ ] Add `Detect` button.
- [ ] Add `Install Agent` button.
- [ ] Add `Start Agent` button.
- [ ] Add `Update Agent` button.
- [ ] Add a status timeline area for SSH/download/install/start steps.
- [ ] Add firewall guidance when HTTP polling fails after successful start.
- [ ] Surface release version and remote installed version.
- [ ] Keep advanced command overrides collapsed.

## Binary Distribution Strategy

Preferred order:

1. Use GitHub latest release artifact matching the remote platform.
2. Use a user-selected local binary as a developer fallback.
3. Use a custom command override for unusual machines.
4. Later, optionally bundle common agent binaries for offline installs.

Bundling all binaries inside the app can improve offline UX, but it increases bundle size and release complexity. Since the project already publishes multi-platform release artifacts, release download is the best default.

## Security Checklist

- [ ] Generate a token for the remote agent.
- [ ] Store the token in local settings.
- [ ] Install token config on the remote host.
- [ ] Start agent with token.
- [ ] Poll with `Authorization: Bearer`.
- [ ] Warn clearly when the user chooses unauthenticated LAN metrics.

## Implementation Order

1. Add backend platform detection and asset mapping.
2. Add release discovery endpoint.
3. Add detect endpoint and UI detect button.
4. Add install endpoint using release download + SCP.
5. Add start endpoint and firewall diagnostics.
6. Add update endpoint.
7. Add tokenized agent config.
8. Polish UI states and copy.

## Ryne Test Case

Remote host:

```text
ssh target: user@example-host
llama-server: http://192.0.2.16:8001
agent: http://192.0.2.16:7779
platform: Windows x86_64
asset: llama-monitor-windows-x86_64.exe
install: %APPDATA%\llama-monitor\bin\llama-monitor.exe
```

Expected flow:

1. Attach to `http://192.0.2.16:8001`.
2. Open Settings -> Remote Agent.
3. Enter `user@example-host`.
4. Click Detect.
5. App identifies Windows x86_64 and matching release artifact.
6. If missing or outdated, click Install/Update.
7. Click Start or rely on auto-start.
8. Dashboard shows Remote Agent connected and remote System/GPU metrics.
