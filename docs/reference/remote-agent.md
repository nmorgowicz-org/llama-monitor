# Remote Agent

The remote agent adds host-level telemetry to remote llama.cpp endpoints. Without it, Llama Monitor can still attach to a remote server and read inference metrics, but GPU, CPU, RAM, and host-health data remain unavailable.

## What it adds

When a remote agent is installed and reachable, the dashboard can show:

- GPU temperature, utilization, VRAM, power, and clocks
- CPU load, temperature, RAM, and host/platform details
- Agent reachability, installed version, and update availability

This is what upgrades a remote attach session from **Inference only** to **Full telemetry**.

## Entry points in the UI

There are two current ways into remote-agent setup:

### Header Agent button

The **Agent** control in the top header is the fast path for remote telemetry. It is also where the dashboard surfaces status and a **Fix** action when a remote endpoint needs agent attention.

Use it when you want a guided setup flow focused on one remote host.

### Configuration modal

For manual control, open:

`Settings → Advanced → Open Runtime Configuration → Remote Agent`

This panel exposes the full runtime controls:

- Guided SSH setup
- Host check
- Release check
- Install and start
- Start, stop, restart, update, and remove
- Saved agent URL, token, SSH target, optional autostart, and optional custom SSH start command

## mTLS and trust

- Remote agents communicate with the dashboard over mTLS.
- The agent loads trust anchors from:
  - A legacy single CA (ca.pem), if present, and
  - All .pem files in the cas/ directory (multi-CA support).
- This allows multiple independent CAs to be trusted across different agents.
- If no CA is found, the agent refuses to start.

## Agent tokens

- The primary agent token is configured via --agent-token or the UI.
- The agent also supports multiple allowed tokens via agent-tokens.json:
  - File: ~/.config/llama-monitor/agent-tokens.json
  - Format: { "tokens": ["<token1>", "<token2>"] }
  - Any token in this list is accepted for authenticated agent endpoints.
- On startup, the primary token is automatically ensured in this file, enabling multi-client setups (for example, multiple dashboards polling the same agent).

## Protocol and versioning

- Agent endpoints:
  - GET /info: returns agent version and protocol_version.
  - GET /agent/info: same, plus agent_token for verification.
- The current protocol version is 1.0.0.
- The dashboard enforces a minimum protocol version when polling the agent.
- If the agent’s protocol_version is below 1.0.0 or missing:
  - The dashboard enables degraded compatibility mode instead of fully disconnecting.
  - Partial metrics may still be available; advanced features may be limited.

## Version mismatch and degraded mode

- If the remote agent’s protocol_version is too old or unknown:
  - The dashboard continues to treat the agent as connected.
  - It logs a warning and runs in degraded compatibility mode.
- If metrics parsing fails:
  - The dashboard prefers degraded mode over full disconnect,
    preserving partial telemetry when possible.

## Current setup flow

The real setup flow is now explicit and opt-in. Typing an endpoint or SSH target does not trigger SSH activity by itself.

### Guided SSH setup

In the Runtime Configuration panel, click **Guided SSH Setup**.

1. Enter the remote host, username, port, and auth method.
2. Click **Preview Plan** to review the inferred agent URL, SSH target, auth mode, and expected install path.
3. Click **Scan Host Key** to fetch the host fingerprint.
4. Click **Trust Host Key** if the fingerprint is correct.
5. Click **Use These Settings** to populate the saved SSH target and agent URL.
6. From the main Remote Agent controls, explicitly choose **Check Host**, **Install & Start**, or **Start Agent**.

The guide only prepares and validates settings. It does not contact the remote machine until you click one of the action buttons.

### Header setup modal

When the dashboard knows a remote endpoint needs an agent, the header flow opens a dedicated **Set Up Remote Agent** modal. That flow is step-based:

1. Enter the SSH host and port.
2. Choose authentication.
3. Scan and trust the host key.
4. Review the generated agent details.
5. Install/start the agent and save the resulting URL/token back into settings.

Use this flow when the app prompts you directly from a remote attach session.

## Saved runtime settings

Remote-agent connection details are persisted as runtime configuration, not ordinary UI preferences.

| Setting | Purpose |
|---------|---------|
| **Agent URL** | Explicit polling URL for the remote agent; if left blank, the app infers `http://<remote-host>:7779` from the attached endpoint host |
| **Agent Token** | Optional bearer token required by the agent |
| **SSH target** | Saved `user@host` target for manual agent actions or optional autostart |
| **After attach, try SSH start if the agent is unreachable** | Enables one autostart attempt after attaching to a remote endpoint |
| **Custom SSH start command** | Overrides the built-in OS-specific start command |

## SSH behavior and trust

Host-key verification is required before managed SSH operations.

1. The app scans the host key over SSH.
2. It shows the host, key type, and fingerprint.
3. Trusting the key stores it locally for future verification.
4. If the key changes later, the dashboard rejects the managed SSH operation until you re-scan and trust the new key.

The dedicated SSH backend supports:

- SSH agent / keychain
- Password for the current operation
- Private key path, with optional passphrase

## Autostart behavior

If **After attach, try SSH start if the agent is unreachable** is enabled, the dashboard will make one saved SSH start attempt after you attach to a remote endpoint and discover that the agent is unavailable.

This is not global background probing. It only applies after an attach, and only when you have explicitly enabled the setting.

## Platform behavior

### Windows

On Windows, the managed install uses scheduled tasks for both the agent and `sensor_bridge.exe`, allowing startup without an interactive session.

### Linux and macOS

The app chooses an OS-appropriate install/start path and command for the managed agent. The guided plan preview shows the expected install location before you run it.

## Troubleshooting

| Symptom | Likely cause | Action |
|---------|--------------|--------|
| **Inference only** status on a remote session | No agent is running or the agent URL is not reachable | Open the Agent flow or Runtime Configuration and start/install the agent |
| **Agent Started, HTTP Blocked** warning | The process started, but port `7779` is blocked or bound incorrectly | Open the remote firewall and confirm the agent is listening on `0.0.0.0` when remote access is required |
| Host-key mismatch | The remote server fingerprint changed | Re-scan the key and verify the host before trusting it again |
| Start/install buttons complain about missing SSH target | Guided settings were not applied or no target was saved | Run **Guided SSH Setup** and click **Use These Settings** |
| Agent is reachable but some host metrics are missing | The remote system cannot provide those sensors | Check the underlying platform tools such as `nvidia-smi`, `rocm-smi`, or OS temperature access |

## Stored secrets and security

Credentials used by the remote agent and internal APIs are encrypted at rest using AES-256-GCM and stored in `~/.config/llama-monitor/`.

Encryption:
- Llama-monitor:
  - Uses an encryption key:
    - From `LLAMA_MONITOR_ENCRYPTION_KEY` if set and ≥16 characters, or
    - Auto-generated and stored in `encryption-key` in the config directory.
  - Automatically encrypts:
    - `remote_agent_token`
    - `api-token`
    - `db-admin-token`
    - ACME `dns_config` values
- No manual key management is required.

Encrypted values:

- **Agent Token** (`remote_agent_token`):
  - Used as a bearer token when polling the remote agent.
  - Encrypted at rest.
  - Masked in `GET /api/settings`; real value available via `GET /api/settings/full` with `api-token` auth.
- **SSH password** (`remote_agent_ssh_password`):
  - Used for the current SSH operation when password auth is selected.
  - Not persisted long-term; used in-memory for the session.
- **Private key path** (`remote_agent_ssh_key_path`):
  - Path reference only; the key file itself is not copied into llama-monitor’s config.

Notes:
- These values are never logged in full (only “token generated”-style messages).
- They are not included in generic debug or health responses.
- For security-conscious setups:
  - Treat `~/.config/llama-monitor/` as sensitive.
  - Use file permissions and/or disk encryption where available.

## Tokens and rotation

Three tokens are used by llama-monitor:

- **API Token**:
  - File: `api-token`
  - Used to protect sensitive endpoints (attach, DB queries, TLS/ACME, etc.).
  - Encrypted at rest.
- **DB Admin Token**:
  - File: `db-admin-token`
  - Used for advanced database operations and restricted queries.
  - Encrypted at rest.
- **Agent Token**:
  - Stored in `ui-settings.json` as `remote_agent_token`.
  - Used to authenticate with the remote agent.
  - Encrypted at rest.

All three can be rotated from the UI:

- Open `Settings → Security & Certificates`.
- Use one of:
  - `Rotate Agent Token`
  - `Rotate API Token`
  - `Rotate DB Admin Token`
- Confirm the prompt. A new token is generated and the previous one is immediately invalid.

After rotating the API Token or DB Admin Token, restart llama-monitor to fully apply the change.

## CLI equivalents

The dashboard-side CLI flags for remote-agent integration are documented in [CLI Flags](cli-flags.md). The agent itself can be launched directly with `--agent`, `--agent-host`, `--agent-port`, and `--agent-token`.
