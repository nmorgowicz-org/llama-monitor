# UI/UX and Monitoring Roadmap

**Date:** 2026-04-19  
**Status:** Planning guidance for future implementation  
**Scope:** Main dashboard, tray dropdown, local/remote metric behavior, Linux/headless behavior

## Summary

This document captures recommended product, UI, and architecture improvements discovered while fixing the tray dropdown and Linux/headless behavior.

The main theme is that llama-monitor should make the active monitoring mode obvious. A local managed server can expose inference, system, and GPU metrics. A remote attached llama-server endpoint can currently expose inference metrics only. The UI should treat those as distinct states instead of showing empty metric tables or relying on scattered null checks.

The app should feel like a monitoring cockpit first: live endpoint health, model/runtime status, inference throughput, context usage, memory pressure, GPU pressure, and clear availability states should be visible immediately. Setup, presets, paths, and launch controls are important, but should not dominate the first screen once a session exists.

## Goals

1. Make local vs remote monitoring explicit.
2. Render all metric sections from backend capability flags.
3. Keep unavailable data out of the way, with concise reasons when needed.
4. Improve the tray dropdown as a fast status glance, not a full dashboard.
5. Support Linux desktop and headless server users deliberately.
6. Prepare a clean path for future remote host telemetry.

## Current Behavior and Problem Areas

### Local vs Remote Attach

The app now hides local system and GPU metrics when attached to a non-local endpoint. This is the correct behavior for the current architecture because those metrics would otherwise describe the machine running llama-monitor, not the remote llama-server host.

However, the user experience can still be clearer. The UI should not merely omit data. It should communicate the monitoring mode:

- Local session: inference, system, and GPU metrics can be available.
- Remote attach: inference metrics are available from llama.cpp; host metrics are unavailable unless a remote metrics agent is added later.
- Headless mode: web/API remains available, but no tray is expected.

### Empty or Missing Metrics

Empty cells and collapsed values can look broken. They should be replaced with capability-aware rendering. If a section is unavailable, either hide it entirely or show one concise reason at the relevant level.

Examples:

| Condition | Recommended UI |
|---|---|
| Remote endpoint | Hide system/GPU sections; show `Remote endpoint - host metrics unavailable` near endpoint status |
| CPU temp unsupported | Hide CPU temp row or show `Sensor unavailable` in main dashboard only |
| GPU backend missing | Show `GPU metrics unavailable - no supported backend detected` in setup/status area |
| llama.cpp metrics disabled/unreachable | Show a connection health warning |
| No active generation | Show inference rows with idle state, not an error |

## Recommended Architecture

### Add Metric Capability Flags

The backend should expose a small capability model with the app state. This gives the frontend a single source of truth for which UI sections should render.

Example shape:

```json
{
  "capabilities": {
    "inference": true,
    "system": false,
    "gpu": false,
    "cpu_temperature": false,
    "memory": false,
    "host_metrics": false,
    "tray": true
  },
  "mode": {
    "session_kind": "attach",
    "endpoint_kind": "remote",
    "tray_mode": "desktop"
  },
  "availability": {
    "system_reason": "remote_endpoint",
    "gpu_reason": "remote_endpoint",
    "cpu_temperature_reason": "remote_endpoint"
  }
}
```

Exact names can change, but the important concept is that the UI should not infer capabilities from partially missing metric values. The backend should decide what is possible and why.

### Suggested Capability Fields

| Field | Meaning |
|---|---|
| `inference` | llama.cpp metrics are reachable and can be displayed |
| `system` | local host CPU/RAM metrics are valid for the active endpoint |
| `gpu` | local GPU metrics are valid for the active endpoint |
| `cpu_temperature` | CPU temperature sensor data is available |
| `memory` | system memory data is available |
| `host_metrics` | host hardware metrics are valid for the active endpoint |
| `tray` | tray UI is expected to be available |

### Suggested Mode Fields

| Field | Values | Notes |
|---|---|---|
| `session_kind` | `spawn`, `attach`, `none` | User mental model for how llama.cpp is controlled |
| `endpoint_kind` | `local`, `remote`, `unknown` | Whether host metrics should describe this machine |
| `tray_mode` | `desktop`, `headless`, `failed` | Useful for diagnostics and settings |

### Suggested Availability Reasons

Keep these machine-readable, then map them to human-readable UI strings.

Possible values:

- `available`
- `remote_endpoint`
- `no_display`
- `tray_unavailable`
- `sensor_unavailable`
- `backend_unavailable`
- `command_missing`
- `permission_denied`
- `metrics_unreachable`
- `not_applicable`

## Remote Host Metrics: Future Direction

Do not try to fake remote system/GPU metrics by reading the local machine. That creates user confusion because the displayed CPU/GPU would not be the server doing inference.

The better long-term model is a lightweight remote agent:

```text
llama-monitor desktop/web UI
    |
    |-- llama.cpp metrics endpoint
    |
    `-- llama-monitor-agent on remote host
          |-- CPU/RAM metrics
          |-- GPU metrics
          |-- temperatures
          `-- optional process metrics
```

The remote agent should expose a small authenticated API. The main app can then attach to both the llama.cpp server and the metrics agent. This makes ownership clear:

- llama.cpp endpoint answers inference questions.
- llama-monitor-agent answers host hardware questions.
- The UI labels the source of each metric.

Security should be considered before implementation. Avoid unauthenticated hardware/process telemetry on a LAN by default.

## Main Dashboard UX Recommendations

### Dashboard First

After startup, the first screen should be the operational dashboard, not a settings page. The user should immediately see:

- Active session or endpoint
- Local/remote mode
- Model name when known
- Server health
- Prompt throughput
- Generation throughput
- Context usage
- RAM/VRAM pressure when local
- CPU/GPU temperature when available

### Endpoint Health Strip

Add a small persistent strip near the top of the dashboard. This should remove ambiguity about what is being monitored.

Local example:

```text
Local - http://127.0.0.1:8080 - inference OK - system OK - GPU OK
```

Remote example:

```text
Remote - http://192.168.1.50:8080 - inference OK - host metrics unavailable
```

Failure example:

```text
Remote - http://192.168.1.50:8080 - metrics unreachable
```

This strip should use restrained status indicators: small dots, compact badges, or short text. Avoid large banners unless the app is unusable.

### Attach vs Spawn Mental Model

The UI should clearly separate the two major workflows:

| Mode | UI Should Emphasize |
|---|---|
| Spawn local server | model path, preset, GPU backend, process controls, local system/GPU metrics |
| Attach to existing endpoint | endpoint URL, connection health, inference metrics, remote/local distinction |

Remote attach screens should not show local launch details as if they apply to the remote server. Spawn screens should include process controls and launch configuration.

### Status Severity

Add a simple severity model for important metrics:

| Severity | Examples |
|---|---|
| Normal | context below 70%, temperatures normal, server reachable |
| Warning | context above 85%, high VRAM, high temp, slow/no metrics updates |
| Critical | server unreachable, context near max, out-of-memory risk |

Use subtle visual treatment:

- small colored status dot
- left border on a metric row/card
- compact badge
- restrained text color

Do not turn the whole UI into an alert surface. Most monitoring should remain calm and scannable.

## Tray Dropdown UX Recommendations

### Purpose

The tray dropdown should answer one question quickly:

> Is my model healthy right now?

It should not try to be the full dashboard. The user should be able to glance at it in one or two seconds.

### Recommended Content Order

1. Endpoint/session header
2. Inference status
3. Context usage
4. Local hardware pressure, only when available
5. Short mode/availability note when needed

Example for a local endpoint:

```text
llama-monitor
Local - qwen3-32b

Prompt       182 tok/s
Generate      24 tok/s
Context   4,096 / 32,768 - 12.5%

GPU          71% - 18.4 / 24 GB
CPU          38% - 62 C
RAM        21.2 / 64 GB
```

Example for a remote endpoint:

```text
llama-monitor
Remote - 192.168.1.50:8080

Prompt       182 tok/s
Generate      24 tok/s
Context   4,096 / 32,768 - 12.5%

Host metrics unavailable for remote endpoint
```

### Auto Height

The tray dropdown should continue to auto-size based on visible metric groups. When only inference metrics are visible, the dropdown should be compact. When local hardware metrics are available, it can grow.

Implementation guidance:

- Keep fixed width for visual stability.
- Let height be content-driven within min/max bounds.
- Use the existing compact page resize IPC to request the correct native window size.
- Avoid leaving reserved blank space for unavailable sections.

### Context Usage

Context usage should always include both numbers and percentage when available:

```text
Context  12,384 / 65,536 - 18.9%
```

This is more useful than percentage alone because the raw limit varies dramatically by model/server configuration.

## Headless and Tray Behavior

### Desired Behavior

The app should support three environments gracefully:

| Environment | Expected Behavior |
|---|---|
| Desktop macOS/Windows/Linux | Start tray if available, serve web UI/API |
| Linux without `DISPLAY`/`WAYLAND_DISPLAY` | Skip tray, serve web UI/API |
| Any platform where tray creation fails | Log warning, continue serving web UI/API |

This is now partially implemented. A future improvement is to make it explicit in the command-line interface.

### Add Explicit CLI Flags

Recommended flags:

```bash
llama-monitor --headless
llama-monitor --no-tray
```

Suggested semantics:

| Flag | Behavior |
|---|---|
| `--headless` | Do not start tray or open desktop UI. Serve web/API only. |
| `--no-tray` | Skip tray icon but otherwise behave normally. |

`--headless` and `--no-tray` may initially be aliases if there is no other desktop UI behavior. Keep both if they help user intent: server users think "headless"; desktop users think "no tray."

### Linux Runtime Notes

Linux desktop tray behavior depends on the desktop environment and installed libraries.

For Ubuntu Desktop 24.04, expected runtime packages include:

- `libwebkit2gtk-4.1-0`
- `libayatana-appindicator3-1`
- a desktop shell/status notifier implementation

Build-time packages include:

- `libwebkit2gtk-4.1-dev`
- `libayatana-appindicator3-dev`

Ubuntu GNOME usually has AppIndicator support, but other desktop environments may vary. On Wayland, exact popover positioning may be compositor-controlled. The tray popover should use best-effort positioning and fall back to a sane monitor-relative position.

## First-Run UX

Avoid a marketing-style landing page. If no sessions exist, the first screen should still be the app, but with a focused setup panel.

Recommended first-run actions:

- Start local server
- Attach to existing endpoint
- Select model path
- Pick preset
- Choose/detect GPU backend

The goal is to get the user to a live monitoring state quickly.

## Visual Design Direction

### Product Feel

The app should feel like a modern local AI operations console. It should be dense enough for technical users, but polished enough to feel intentional and premium. Avoid the look of a plain HTML admin table.

Recommended qualities:

- Calm, high-contrast dashboard surface.
- Strong information hierarchy.
- Compact but readable metric rows.
- Subtle motion that helps users understand changing state.
- Visual rhythm through spacing, dividers, and typography.
- Capability-aware empty states rather than blank panels.

Avoid:

- Large decorative gradients with no information value.
- Overly rounded, toy-like cards.
- Neon-only color palettes.
- Heavy glow effects.
- Giant marketing-style hero sections.
- Unexplained animations that compete with metric scanning.

The app is a monitoring tool. It should look premium through restraint, precision, and excellent state design.

### Layout System

Use a dashboard layout that puts live operational state first.

Suggested desktop structure:

```text
┌──────────────────────────────────────────────────────────────┐
│ Top bar: endpoint, mode, health, quick actions               │
├──────────────────────────────────────────────────────────────┤
│ Key metrics: tok/s, context, memory, GPU, temperature         │
├──────────────────────────────────────────────────────────────┤
│ Main area: charts and detailed metric tables                  │
├──────────────────────────────────────────────────────────────┤
│ Secondary: sessions, presets, logs, launch controls           │
└──────────────────────────────────────────────────────────────┘
```

Use full-width sections and grouped metric bands rather than nesting cards inside cards. Cards are useful for repeated metric units, but page sections should not all become floating boxes.

Recommended card style:

- Border radius: 6-8px.
- Border: subtle 1px line.
- Background: slightly elevated from page background.
- Shadow: minimal or none.
- Use left borders, status dots, or small badges for state.

### Typography

Monitoring UI depends on numbers. Use a clear sans-serif for labels and a tabular-number style for metrics.

Recommended CSS:

```css
.metric-value,
.metric-unit,
.metric-row-number {
  font-variant-numeric: tabular-nums;
}
```

Guidance:

- Keep labels short.
- Keep units visible but visually secondary.
- Align numeric values so changes are easy to scan.
- Use font weight to indicate hierarchy, not large size jumps.
- Do not scale font size with viewport width.

### Color System

Use color as state, not decoration. The app should support dark mode well because monitoring tools are often left open.

Recommended palette roles:

| Role | Purpose |
|---|---|
| Background | App shell and large surfaces |
| Surface | Metric groups, tables, controls |
| Surface elevated | Popovers, dropdowns, modals |
| Text primary | Main labels and values |
| Text secondary | Units, helper labels, unavailable reasons |
| Border | Separators and grouping |
| Accent | Primary action and selected state |
| Success | Healthy/connected |
| Warning | High usage or degraded metrics |
| Critical | Unreachable, near-limit, error |
| Info | Remote/headless/capability notes |

Avoid building the whole interface around one hue. The app should not become entirely purple, blue, beige, slate, or orange. A restrained neutral base with a few state colors will look more durable.

### Charts and Visualizations

Charts should answer operational questions, not decorate the page.

Recommended visualizations:

| Metric | Visualization | Notes |
|---|---|---|
| Prompt tok/s | Sparkline + current value | Useful when prompt processing spikes briefly |
| Generation tok/s | Sparkline + current value | Primary live performance chart |
| Context usage | Horizontal capacity bar | Always show used/max and percent |
| RAM/VRAM | Horizontal capacity bar | Use warning/critical thresholds |
| GPU load | Sparkline or compact area chart | Pair with current percent |
| CPU load | Sparkline or compact area chart | Secondary to inference metrics |
| Temperature | Line chart or threshold bar | Emphasize warning/critical bands |
| Server health | Status timeline | Useful for disconnect/reconnect visibility |

Avoid pie charts and donut charts for this app. They waste space and make precise values harder to compare. Horizontal bars, sparklines, and small line charts are better for a monitoring dashboard.

### Chart Implementation Approach

Start with lightweight, dependency-minimal visualizations:

- CSS capacity bars for context, RAM, VRAM, CPU, GPU load.
- Canvas sparklines for tok/s, temperature, and utilization history.
- No heavy charting library unless the app needs axes, legends, zooming, or multi-series interactions.

For the tray dropdown, prefer CSS bars and tiny sparklines. For the full web app, a small chart helper built around `<canvas>` is likely enough.

Suggested helper shape:

```js
class Sparkline {
  constructor(canvas, options) {}
  setData(points) {}
  render() {}
}
```

Keep chart history bounded in memory. A good default is 60-180 samples depending on polling interval. The UI should survive long-running sessions without accumulating unbounded arrays.

### Chart Behavior

Charts should be stable under live updates.

Guidance:

- Smooth value transitions over 120-200ms.
- Avoid animating the entire chart on every sample.
- Keep axes or scale behavior stable enough that the chart does not visually jump.
- Use tabular numbers for live values beside charts.
- Show current value, not just history.
- Use thresholds for capacity/temperature colors.
- Preserve readable contrast in dark mode.

For example:

```text
Generate  24.8 tok/s   ▁▂▄▅▆▆▇▅▆
Context   12,384 / 65,536   18.9%   [██████░░░░░░░░░░░░]
VRAM      18.4 / 24.0 GB    76.7%   [███████████████░░░░]
```

### Animation and Motion

Use motion to clarify change, not to entertain.

Good uses:

- Smooth number changes.
- Capacity bars easing to a new width.
- New warning state fading in.
- Tray dropdown opening with a very short opacity/scale transition.
- Connection status changing with a brief pulse.

Avoid:

- Constant background motion.
- Decorative particles.
- Large bouncing transitions.
- Animating every metric independently in a distracting way.
- Long transitions that make live metrics feel laggy.

Recommended timings:

| Motion | Duration |
|---|---|
| Number/bar update | 120-200ms |
| Popover open/close | 90-140ms |
| Warning state appearance | 150-250ms |
| Page section transition | 160-220ms |

Respect `prefers-reduced-motion`. In reduced-motion mode, disable nonessential transitions and update values directly.

### Main Web App Premium Pass

A future premium UI pass should focus on these specific areas:

1. **Top status bar**
   - Endpoint URL, local/remote badge, health status, session mode, quick open/settings action.
   - Keep it compact and persistent.

2. **Hero metric row**
   - Generation tok/s, prompt tok/s, context usage, memory pressure, GPU pressure.
   - These are the metrics users should see first.

3. **Metric bands**
   - Group inference, memory, GPU, CPU, and server health into clear bands.
   - Use compact rows and bars, not large empty cards.

4. **Detailed tables**
   - Keep detailed numeric tables available, but visually secondary.
   - Use hover/focus states and aligned tabular numbers.

5. **Session controls**
   - Make spawn vs attach visually distinct.
   - Put destructive process actions behind clear buttons and states.

6. **Logs and diagnostics**
   - Use a terminal-like block for logs only where logs are expected.
   - Do not let logs dominate the dashboard unless the server is failing.

### Tray Dropdown Premium Pass

The tray dropdown should feel like a native-quality status panel.

Recommended style:

- Fixed compact width.
- Auto height based on visible metrics.
- Strong header with app/session/endpoint.
- Inference-first content.
- Context line with raw count and percent.
- Local hardware mini rows only when available.
- One concise capability note for remote/headless cases.
- No large tables.
- No scrolling unless absolutely necessary.

Suggested structure:

```text
llama-monitor        ● OK
Remote - 192.168.1.50:8080

Generate       24.8 tok/s   ▁▂▄▅▆▆▇
Prompt        182.0 tok/s   ▂▃▇▅▁▁▂
Context   12,384 / 65,536   18.9%
[██████░░░░░░░░░░░░]

Host metrics unavailable for remote endpoint
```

For local:

```text
llama-monitor        ● OK
Local - qwen3-32b

Generate       24.8 tok/s   ▁▂▄▅▆▆▇
Prompt        182.0 tok/s   ▂▃▇▅▁▁▂
Context   12,384 / 65,536   18.9%
[██████░░░░░░░░░░░░]

GPU        71%   18.4 / 24 GB
CPU        38%   62 C
RAM        33%   21.2 / 64 GB
```

The tray should prioritize confidence and quick interpretation. If a metric row does not fit cleanly, remove it from the tray and keep it in the main dashboard.

### Interaction Details

Recommended interactions:

- Clicking a metric in the tray opens the main dashboard focused on that metric group.
- Clicking the endpoint/status header opens connection/session details.
- Warning badges in the main app should be clickable and reveal the reason.
- Hover states should be subtle and useful, not merely decorative.
- Use keyboard focus styles for web UI controls.

### Responsive Behavior

The main web UI should support three practical widths:

| Width | Behavior |
|---|---|
| Narrow/mobile | Single-column metric stack, compact controls |
| Tablet/small desktop | Two-column metric bands |
| Wide desktop | Dashboard with metric row, charts, and detail panels |

The tray dropdown should not simply reuse the full dashboard responsively. It needs a purpose-built compact layout.

### Visual QA Checklist

Before considering a UI polish pass complete:

1. Test local endpoint with all metrics.
2. Test remote endpoint with inference-only metrics.
3. Test idle server with no active generation.
4. Test server unreachable state.
5. Test high context usage warning.
6. Test missing GPU backend.
7. Test missing CPU temperature.
8. Test narrow browser width.
9. Test tray dropdown with inference-only content.
10. Test tray dropdown with full local hardware content.
11. Test dark mode contrast.
12. Test `prefers-reduced-motion`.
13. Confirm text does not overlap or truncate badly.
14. Confirm charts do not grow memory unbounded over long sessions.

## Suggested Implementation Plan

### Phase 1: Capability Model

1. Add backend capability and availability reason structs.
2. Populate endpoint locality: local, remote, unknown.
3. Populate metric availability reasons for inference, system, GPU, CPU temperature, and tray.
4. Expose capabilities in the existing state API/WebSocket payload.
5. Add unit tests for locality and capability generation.

### Phase 2: Capability-Aware UI

1. Update the main dashboard to render sections from capabilities.
2. Hide unavailable groups instead of showing empty rows.
3. Add concise availability messaging at the endpoint/status level.
4. Add the endpoint health strip.
5. Preserve useful idle states for inactive inference.

### Phase 3: Tray Dropdown Polish

1. Refactor compact tray UI to use the same capability payload.
2. Keep inference metrics first.
3. Show context as used/max plus percentage.
4. Show local hardware sections only when available.
5. Keep the dropdown auto-height behavior and validate small/large content cases.

### Phase 4: Explicit Headless Mode

1. Add `--headless` and `--no-tray` CLI flags.
2. Ensure flags override automatic tray startup.
3. Add log messages that clearly state the selected mode.
4. Test desktop and no-display behavior.

### Phase 5: Remote Metrics Agent Design

1. Draft an authenticated remote agent API.
2. Define metric schema and source labels.
3. Add UI affordance for "remote host metrics connected."
4. Avoid enabling remote host telemetry by default until auth and docs are solid.

## Acceptance Criteria

Future work based on this document should be considered successful when:

1. A remote attached endpoint shows inference metrics without blank system/GPU tables.
2. A local endpoint shows valid local system/GPU metrics when available.
3. CPU temperature absence is explained or hidden consistently.
4. The tray dropdown remains compact when only inference metrics are visible.
5. Context usage always shows raw token counts and percentage when available.
6. Linux desktop builds can show the tray popover with required runtime packages.
7. Headless Linux runs without display variables and keeps the web/API server alive.
8. macOS/Windows/Linux continue serving headlessly if tray startup fails.
9. The UI clearly labels local vs remote monitoring mode.
10. Automated tests cover capability decisions, especially local vs remote endpoint behavior.

## Notes for Future Agents

Before changing UI rendering, inspect the current state payload and compact tray page. Prefer adding a single backend capability source over duplicating local/remote checks in multiple frontend files.

When modifying Linux tray behavior, test on an actual Linux environment or the GitHub runner with the required GTK/WebKit packages installed. macOS cross-checks cannot prove Linux WebKitGTK compilation because the native system libraries are resolved through Linux `pkg-config`.

Keep the tray dropdown focused. If a metric would require explanation, it probably belongs in the main dashboard rather than the compact popover.
