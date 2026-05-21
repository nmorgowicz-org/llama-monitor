# Dashboard Capabilities & Metrics

Llama Monitor's monitoring surface is split between a live top-nav cockpit, the Server tab, and host telemetry cards that light up when the app can read local hardware or reach a remote agent.

## Monitoring Surfaces

### Top-nav cockpit

The compact strip in the top navigation shows the current endpoint state without leaving the active tab:

| Chip | Description |
|------|-------------|
| **State** | Current llama.cpp activity such as idle, attach, prompting, or generating |
| **Throughput** | Prompt (`P`) and generation (`G`) speed in tokens/sec |
| **Context** | Highest context-pressure percentage across open chat tabs |
| **GPU** | Temperature of the hottest available GPU |
| **Sparkline** | Recent generation-speed history |

Clicking the cockpit jumps to the Server tab. On narrower layouts the GPU and sparkline chips collapse first, then the context chip.

### Capability popover

Hovering the endpoint status chip in the top nav opens a popover listing per-subsystem telemetry states:

| Row | Meaning |
|-----|---------|
| **Inference** | Whether llama.cpp inference metrics are live |
| **Slots** | Whether slot data is available |
| **Metrics** | Whether throughput / context metrics are being reported |
| **Generation progress** | Whether the server exposes live generation budget |
| **Throughput** | Shows "retained avg + live estimate" if metrics are available |
| **Context capacity** | Whether context capacity is known |
| **Context usage** | Live if exposed by llama.cpp; otherwise "derived from chat" |
| **Host metrics** | Whether GPU/system telemetry is available |
| **Remote agent** | Connected or disconnected |

The popover is populated in real time from WebSocket data; each row shows a green LED for live/ok and a muted indicator when unavailable.

### Server tab

The Server tab is the main monitoring dashboard. It combines llama.cpp inference data from `/metrics` and `/slots` with host telemetry when available.

| Card | What it shows |
|------|----------------|
| **Throughput** | Prompt and generation speeds, peak tracking, throughput ratio bar, metric age, and delta indicators |
| **Generation** | Output tokens, remaining budget, generation ring progress, stage indicators (Prompt/Output), live output estimation sparkline |
| **Context Window** | Gauge or fleet view of context pressure across chat tabs |
| **Slot Activity** | Per-slot state, output tokens, context usage, slot utilization bar, and batch efficiency |
| **Request Activity** | Activity rail (recent request timeline), request count, and average duration |
| **Model & Decoding** | Active model name, quantization, sampler config inline, speculative decoding chip and config grid |

![Server Tab](../screenshots/settings-server-tab.png)

### Fine-grained metrics

Additional metrics and indicators shown on the Server tab when data is available:

- **Peak throughput tracking**: Highest observed prompt and generation t/s are tracked and shown as "peak" labels.
- **Throughput ratio bar**: Displays the prompt-to-generation speed ratio when both are active.
- **Metric age indicators**: Shows how old the latest throughput data is (e.g., "2s ago").
- **Metric delta indicators**: Briefly shows +/- changes when throughput values shift.
- **Slot utilization bar**: Percentage of slots currently processing.
- **Batch efficiency**: Displays "busy slots per decode" on multi-slot servers.
- **Speculative decoding**:
  - A chip indicates whether speculative decoding is enabled and its type.
  - A config grid shows speculative parameters when exposed.
- **Sampler config inline**: Key sampler settings (temp, top_k, top_p, etc.) shown inline when available.
- **Generation ring progress**: A ring visualization of how far along the current generation budget is.
- **Stage indicators**: Shows whether the server is in prompt or output phase.
- **Live output estimation**: A sparkline tracking estimated live generation rate.
- **Activity rail**: A timeline bar of recent requests, color-coded by prompt vs. generation phases.
- **Recent task strip**: Summarizes the last completed task (task ID, output tokens, duration, estimated t/s).
- **Request stats**: Total completed requests and average duration over the last 10 minutes.

### Context Window card

The Context Window card has two toggleable views:

- **Gauge view**: Shows a central gauge of context pressure (live runtime or busiest chat), a chat-strip of tracked chats, and aggregate stats.
- **Fleet view**: Shows per-chat rows with context pressure bars, aggregate utilization, and overflow for many chats.

Behavior:

- When llama.cpp exposes live KV-cache tokens, the card uses that.
- When it does not, the card derives context pressure from chat message history (cumulative output tokens plus last request's input tokens vs. capacity).
- Chats unchanged for 7+ days are dimmed and labeled "stale."
- If a smaller model is loaded and one or more chats exceed its context window, a warning toast appears with per-chat "Compact now" buttons.

## Host Telemetry

Host metrics are available in two ways:

- **Local session**: the dashboard reads GPU/system data directly from the same machine.
- **Remote session with agent**: the remote agent reports GPU/system/process telemetry back to the dashboard.
- **Remote session without agent**: you still get inference metrics, but GPU/system cards stay limited.

### GPU metrics

| Metric | Local sources |
|--------|---------------|
| Utilization | `rocm-smi`, `nvidia-smi`, `mactop` |
| Power draw | `rocm-smi`, `nvidia-smi` |
| VRAM usage | `rocm-smi`, `nvidia-smi`, `mactop` |
| Core clock | `rocm-smi`, `nvidia-smi` |
| Memory clock | `rocm-smi`, `nvidia-smi` |
| Temperature | `rocm-smi`, `nvidia-smi`, `mactop` |

Each metric shows a current value plus a sparkline or alternate visualization where supported.

Power capping:

- If power consumption reaches the configured power limit, the card highlights the metric with a cap indicator and exclamation mark.

Clock visualization:

- GPU clocks can be shown as dual-ring orbits (one for core, one for memory) with meters, or as chips, or as plain numeric values.

![GPU & System Metrics](../screenshots/dashboard-gpu-section.png)

### System metrics

| Metric | Source |
|--------|--------|
| CPU load and model | `sysinfo` |
| CPU temperature | Linux thermal zones, `mactop`, or `sensor_bridge.exe` on Windows |
| CPU clock | `/proc/cpuinfo` on Linux, `mactop` on macOS |
| RAM usage | `sysinfo` |
| Motherboard / platform info | platform-specific host inspection |

CPU clock visualization:

- Can be shown as a single ring orbit with meter, as a chip, or as a plain numeric value.

Sensor bridge (Windows):

- On Windows, if CPU temperature is unavailable, a "No temp data" badge may appear.
- A callout with a "Setup" button is shown when the sensor_bridge service is not yet installed; clicking it triggers a UAC prompt to install the service.

## Capability states

The UI exposes telemetry availability directly:

| State | Meaning |
|-------|---------|
| **Full telemetry** | Inference metrics plus host GPU/system data |
| **Inference only** | Connected to llama.cpp, but no host telemetry source is available |
| **Limited** | Partial host telemetry is available but some sensors are missing |
| **Error** | The dashboard cannot reach the required endpoint |

This matters most for remote endpoints: attaching to a remote llama.cpp server alone does not grant GPU or system metrics.

## Telemetry Grade

Remote endpoints use a unified 9-state telemetry grade to derive the agent connection quality:

| Grade | Meaning |
|-------|---------|
| `local_full` | Local session with full telemetry |
| `remote_inference_only` | Remote attach with no agent |
| `remote_agent_connecting` | Agent connection in progress |
| `remote_agent_connected` | Agent connected and healthy |
| `remote_agent_degraded` | Agent connected but protocol version below minimum |
| `remote_agent_firewall_blocked` | Agent connected via SSH but HTTP health unreachable |
| `remote_agent_update_available` | Agent connected but a newer version exists |
| `remote_partial_sensors` | Agent connected but some host sensors unavailable |
| `remote_error` | Agent connection failed or unreachable |

The grade chip is displayed on the agent badge for remote endpoints. The endpoint status strip uses grade-based labels. GPU and system cards show grade-aware empty-state copy when telemetry is partial or unavailable.

## Network detection

If the browser supports the Network Information API, the dashboard:

- Shows a small network status indicator with latency, downlink, and Data Saver status.
- In **Auto** refresh-rate mode, automatically adjusts the WebSocket polling interval based on connection quality:
  - Fast (4G/low RTT): 500 ms
  - Moderate (3G or 100–300 ms RTT): 1–2 s
  - Slow (2G or >300 ms RTT, or Data Saver): 2–5 s
- Displays an "Offline" indicator when the browser goes offline.

## Remote agent advanced states

For remote endpoints, the agent status area can show:

- **Connected**: Agent running and reachable.
- **Firewall blocked**: Agent connected via SSH but HTTP port unreachable; shows a "Fix" button to open the setup modal.
- **Update available**: A newer agent version exists; shows an "Upgrade" button.
- **Tooltip**: Hovering the agent status shows version and agent URL.
- **Grade chip**: A compact chip on the agent badge reflects the current telemetry grade (see [Telemetry Grade](#telemetry-grade)).

## Setup Screen — Recent Endpoints

The setup screen's attach card is replaced with a recent-endpoints dashboard:

- Shows up to 10 recent attach-mode sessions, fetched via `GET /api/sessions/recent`
- Each entry displays the endpoint name, relative last-connected time, connection count, and a status indicator
- One-click reconnect to any listed endpoint
- A manual attach section remains available below the recent list for new endpoints

## Refresh rate

The dashboard pushes live data over WebSocket. The backend clamps the interval to **200 ms minimum** and **10 s maximum**; the default is **500 ms**.

In the UI, go to **Settings → Performance → Dashboard Refresh Rate**. The current presets are:

| UI choice | Effective interval |
|-----------|--------------------|
| **Auto** | Adapts to network conditions using the browser Network Information API when available |
| **Normal** | 500 ms |
| **Balanced** | 1 s |
| **Battery Saver** | 2 s |
| **Slow Connection** | 5 s |

`Auto` uses the Network Information API (when available) to choose between 500 ms, 1 s, 2 s, or 5 s based on detected connection quality and Data Saver mode. See Network detection for details. If the browser cannot report network quality, it falls back to 500 ms.

## Settings vs. Configuration

The UI now separates **user-facing settings** from **runtime configuration**:

### Settings modal

Open **Settings** from the header or with `Ctrl+,`.

This modal owns:

- Guided-generation toggles and prompt defaults under **Chat**
- Dashboard refresh rate under **Performance**
- The handoff to runtime controls under **Advanced → Open Runtime Configuration**

Do not rely on the Settings tab labels as the place to configure process launch paths or remote-agent connectivity. Those runtime controls live in the separate Configuration modal.

### Configuration modal

Open **Configuration** from **Settings → Advanced → Open Runtime Configuration**.

This modal owns the runtime-specific controls:

- **Local llama-server executable**: executable path and optional process working directory
- **GPU Environment**: local ROCm architecture, local GPU device list, local ROCm path
- **Remote Agent**: agent URL/token, SSH target, optional SSH autostart, guided SSH setup, install/start/update/remove actions

The endpoint you attach to is still chosen from the main session/setup flow. Configuration does not replace the attach/spawn session controls.

## Visualization options

GPU and System cards each have a gear menu with per-metric visualization choices and a reset button. Selected styles persist in `localStorage`.

Available styles:

- **Load / Power / RAM**: bar, ring, or sparkline.
- **VRAM**: bar, stacked (used vs. free), ring, or sparkline.
- **GPU clocks**: dual-ring (core + memory), chips, or numeric-only.
- **CPU clock**: ring, chip, or numeric-only.

The reset button restores each card's defaults (bar for load/power/VRAM/ram, chips for GPU clocks, chip for CPU clock).

## Keyboard shortcuts

Open the shortcuts modal with `Ctrl+/`.

| Shortcut | Action |
|----------|--------|
| `Ctrl+1` | Server tab |
| `Ctrl+2` | Chat tab |
| `Ctrl+3` | Logs tab |
| `Ctrl+1-9` | Jump to chat tab N |
| `Ctrl+Shift+Left/Right` | Previous or next chat tab |
| `Ctrl+,` | Open Settings |
| `Ctrl+Enter` | Start server |
| `Ctrl+.` | Stop server |
| `Escape` | Close the active modal |

![Keyboard Shortcuts](../screenshots/panels-keyboard-shortcuts.png)
