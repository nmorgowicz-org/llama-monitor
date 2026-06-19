# UI Test Documentation

## Automated UI tests

Install dependencies from the repository root:

```bash
npm install
```

Run the Playwright suite:

```bash
npm test
npm test tests/ui/capability-rendering.spec.js
npm test -- --headless=false
npm test -- --debug
```

To exercise form-auth mode, pass extra server args through `LLAMA_MONITOR_TEST_ARGS`:

```bash
LLAMA_MONITOR_TEST_ARGS="--form-auth admin:secret123" npm test -- tests/ui/chat/auth-shell.spec.js
```

The UI suite covers:

- Top navigation, sidebar, and tab navigation
- Server/dashboard rendering
- Chat, logs, and chat-side controls
- Settings and configuration entry points
- Remote-agent flows
- Theme and responsive behavior
- Console/runtime regressions

## Screenshot and GIF capture harness

All repo-managed screenshots and GIFs go through `tests/ui/capture.mjs`.

The harness:

- launches `target/release/llama-monitor` on a temporary local port
- seeds a temporary config from local `ui-settings.json`, `presets.json`, and `gpu-env.json` when present
- attaches to `REMOTE_SERVER` unless the scenario is explicitly no-attach

If any `static/` file changed, rebuild first:

```bash
cargo build --release
```

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SCREENSHOT_PORT` | `8892` | Base port to try for the spawned dashboard |
| `REMOTE_SERVER` | `http://192.168.2.16:8001` | Remote llama.cpp server used by attach-based scenarios |
| `SCREENSHOT_FORM_AUTH` | `admin:secret123` | Credentials used for the auth-shell still captured by the `welcome` scenario |

### Listing scenarios

```bash
node tests/ui/capture.mjs --list-scenarios
```

### Current scenarios

| Scenario | Purpose |
|----------|---------|
| `welcome` | Welcome/setup screen plus form-auth shell without remote attach |
| `chat` | Core chat view, telemetry overlay, and logs |
| `guided-gen` | Context notes, suggestions, quick guide, director, surprise, explicit mode |
| `sidebar` | Chat sidebar, message-search flyout, context menu, title filter |
| `settings` | Settings modal, performance tab, advanced tab, user preferences, persona, models, shortcuts |
| `panels` | Behavior, model, style, and prompt-debug surfaces |
| `dashboard` | Server tab and GPU section |
| `sparkline` | Sparkline validation stills and clipped metric captures |
| `gifs` | Animated inference and GPU/system captures |
| `smoke` | Startup smoke validation |
| `appearance-palette` | Settings Appearance palette stills and light-mode dashboard |
| `navbar` | Top nav bar close-ups: idle-dark, low-power active, idle-light; requires `--close-up` |

### Common commands

```bash
# Welcome screen only
node tests/ui/capture.mjs --scenario welcome

# Core chat / logs / telemetry
SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario chat

# Guided-generation surfaces
SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario guided-gen

# Sidebar and search surfaces
SCREENSHOT_PORT=8893 node tests/ui/capture.mjs --scenario sidebar

# Settings and modal surfaces
SCREENSHOT_PORT=8894 node tests/ui/capture.mjs --scenario settings

# Appearance palettes and light-mode dashboard
SCREENSHOT_PORT=8899 node tests/ui/capture.mjs --scenario appearance-palette --no-attach

# Chat configuration panels
SCREENSHOT_PORT=8896 node tests/ui/capture.mjs --scenario panels

# Server tab and GPU section
SCREENSHOT_PORT=8897 node tests/ui/capture.mjs --scenario dashboard

# Sparkline validation
SCREENSHOT_PORT=8898 node tests/ui/capture.mjs --scenario sparkline

# GIFs
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --gpu-only
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --inference-only

# Smoke run
SCREENSHOT_PORT=8899 node tests/ui/capture.mjs --scenario smoke
```

### Useful options

| Option | Description |
|--------|-------------|
| `--gpu-only` | For `gifs`, capture only GPU/system animation |
| `--inference-only` | For `gifs`, capture only inference animation |
| `--no-attach` | Skip remote attach for scenarios that can run locally |
| `--close-up` | Capture extra element-level detail shots for debugging |
| `--list-scenarios` | Print the registered scenario names |

## Output locations

| Path | Purpose |
|------|---------|
| `docs/screenshots/` | Promoted hero assets used directly in `README.md` |
| `docs/screenshots/artifacts/` | Raw harness output for stills, GIFs, and validation captures |

The harness now writes its outputs to `docs/screenshots/artifacts/`. Promote selected stills or GIFs into `docs/screenshots/` only when you intentionally want a README-facing hero asset.

## Dashboard-related scenarios

When you change dashboard / metrics / server tab visuals, use this subset instead of the full suite:

- `dashboard` — primary Server tab + GPU section (produces `dashboard-performance-section.png`, `settings-server-tab.png`, `dashboard-gpu-section.png`)
- `gifs` — animated metric graphs
  - `--gpu-only` — only GPU/system metrics GIF
  - `--inference-only` — only inference metrics GIF
- `tune-panel` — tuning panel on Server tab
- `benchmark-results` — benchmark results view
- `llama-updater` — updater pill + version modal
- `appearance-palette` — includes light-mode dashboard screenshot

## Updating the harness

When adding or changing screenshot coverage:

1. Extend an existing scenario in `tests/ui/capture.mjs` when the surface already belongs to one.
2. Add a new scenario only when the coverage area is clearly distinct.
3. Register the scenario in the `SCENARIOS` map.
4. Update the usage text in `printUsage()`.
5. Update this README in the same change.

## Troubleshooting

- If captures do not reflect your latest UI edits, rebuild with `cargo build --release`.
- If attach-based scenarios fail, confirm `REMOTE_SERVER` is reachable and returns normal llama.cpp responses.
- If a port is busy, raise `SCREENSHOT_PORT`; the harness scans forward from that base.
- If a popover or panel appears missing, add geometry/state logging to the scenario before skipping the capture.
- If the scenario leaves extra test chats behind, use the shared screenshot-tab helpers in the harness instead of ad hoc tab mutations.
