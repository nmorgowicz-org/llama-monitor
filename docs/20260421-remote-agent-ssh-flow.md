# Remote Agent SSH Flow

This document describes the intended UX and implementation boundaries for remote-agent control.

## Product Rule

The app must never SSH into, install onto, or start processes on a remote host just because a user typed or attached an endpoint.

Remote control is allowed only after one of these explicit user actions:

- The user clicks a remote-agent action such as `Check Host`, `Install & Start`, `Start Agent`, `Update Agent`, `Stop Agent`, or `Restart Agent`.
- The user has intentionally enabled `After attach, try SSH start if the agent is unreachable` for the current remote endpoint.
- The user starts a model preset that is configured to spawn or control a server.

Attaching to `http://host:port` should only attach to that llama-server endpoint. It should not imply SSH access, remote installation, or cloud-provider probing.

## Current Implementation

The web UI calls backend endpoints under `/api/remote-agent/*`.

- `POST /api/remote-agent/detect` uses the dedicated Rust SSH backend in `src/remote_ssh.rs` to detect the remote OS/architecture and check whether the agent is already installed.
- `POST /api/remote-agent/install` downloads the matching GitHub release asset locally, then uses the dedicated SSH backend's SCP support to place it under the remote install path.
- `POST /api/remote-agent/start` uses the dedicated SSH backend to run either the OS-specific default agent command or the user's custom command.
- `POST /api/remote-agent/stop` uses the dedicated SSH backend to stop the agent process.
- Agent HTTP health is checked on the inferred or configured agent URL, usually `http://remote-host:7779`.

The backend accepts either a traditional target such as `user@host` or a structured target such as `ssh://user@host:2222`. The guided setup UI fills the structured target when a non-default port is used.

Default install locations:

- Unix/Linux/macOS: `~/.config/llama-monitor/bin/llama-monitor`
- Windows: `%APPDATA%\llama-monitor\bin\llama-monitor.exe`

Default agent port:

- `7779`

Default agent bind address:

- `0.0.0.0`, so the browser can reach it from another machine if the firewall allows the port.

## SSH Authentication

The dedicated backend supports:

- SSH agent/keychain authentication
- Password authentication for the current operation
- Private key file authentication with an optional passphrase
- Trust-on-first-use host-key verification

Passwords and passphrases from the guided setup are sent only with the explicit remote-agent action request. They are not stored in `ui-settings.json`.

Trusted host keys are stored separately in `~/.config/llama-monitor/ssh-known-hosts.json`. Before detect/install/start/update/stop, the backend requires the scanned host key to match the stored value. If the key changes, the operation is rejected until the user reviews and re-trusts the host.

## Desired Guided SSH Setup

When no SSH target is configured, the UI guides the user through:

1. Remote host address, for example `192.0.2.16`.
2. SSH username.
3. SSH port, default `22`.
4. Authentication method:
   - Existing SSH config/key
   - Password for this session
   - Private key file with optional passphrase
5. Confirmation screen explaining:
   - Which host will be contacted
   - Which release asset will be downloaded
   - Where it will be installed on the remote host
   - Which command will start the agent
   - Which port must be reachable from the browser

The guide previews the plan and fills the runtime SSH settings. The app still runs detect/install/start only after the user clicks an explicit action button.

## Recommended Implementation Path

The shell-out implementation has been replaced by `src/remote_ssh.rs`, which wraps `ssh2`/libssh2.

Remaining hardening work:

- Add cancellation/progress events for long installs.
- Add keychain storage only if the user explicitly opts in.
- Add integration coverage for password/private-key auth in a disposable test environment.

The UI should model the flow as a wizard:

- Step 1: Target
- Step 2: Authentication
- Step 3: Plan
- Step 4: Progress
- Step 5: Result and firewall guidance

The advanced text field for a custom SSH command should remain available for expert users, but it should not be the main path for first-time setup.
