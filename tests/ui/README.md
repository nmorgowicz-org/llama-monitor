# UI Test Documentation

## Setup

```bash
cd tests/ui
npm install
```

## Run Tests

```bash
# Run all tests
npm test

# Run specific test file
npm test capability-rendering.spec.js

# Run with browser visible
npm test -- --headless=false

# Debug mode
npm test -- --debug
```

## Test Coverage

- ✅ Top navigation bar rendering
- ✅ Sidebar navigation rendering
- ✅ Dashboard grid rendering
- ✅ Inference metrics rendering
- ✅ GPU table rendering
- ✅ Chat and Logs tabs
- ✅ Session modal structure
- ✅ Settings modal with tabs
- ✅ Analytics modal structure
- ✅ Export modal structure
- ✅ Keyboard shortcuts modal
- ✅ User preferences modal
- ✅ Theme system (light/dark)
- ✅ Responsive design (desktop/laptop/mobile)
- ✅ Remote agent mode
- ✅ Error states
- ✅ Keyboard shortcuts
- ✅ Accessibility features
- ✅ Console error detection (no fatal JS errors, no assignment to constant errors)

## Manual Testing

```bash
# Start server with headless mode
cargo run -- --headless --port 9999

# Open browser
open http://localhost:9999

# Test each UI component
# 1. Navigate through all tabs
# 2. Test Start/Stop server
# 3. Test Settings modal
# 4. Test Analytics modal
# 5. Test Export modal
# 6. Test keyboard shortcuts (Ctrl+? or Shift+/)
# 7. Test theme toggle
```

## Capture Harness

Use the consolidated capture harness for repo screenshots and GIF assets:

```bash
# List available scenarios
node tests/ui/capture.mjs --list-scenarios

# Core artifact screenshots
SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario artifacts

# Chat-only artifact refresh
SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario artifacts --chat-only

# Guided generation screenshots
SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario new-features

# Docs/review stills
SCREENSHOT_PORT=8894 node tests/ui/capture.mjs --scenario docs

# Sparkline validation captures
SCREENSHOT_PORT=8898 node tests/ui/capture.mjs --scenario sparkline

# Animated GIFs
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --gpu-only
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --inference-only
```

When adding screenshots for a new feature:

1. Add a new scenario function to `tests/ui/capture.mjs` or extend an existing one.
2. Reuse the shared helpers there for boot, attach, screenshot tab cleanup, and capture logging.
3. Register the scenario in the `SCENARIOS` map and add a usage example here.

Troubleshooting:

- If screenshots do not match your source edits, rebuild first with `cargo build --release`.
- If remote metrics or agent connectivity are missing, verify `REMOTE_SERVER` and the local config/token state.
- If a capture depends on a popup or hover surface, log geometry/state from the scenario so invisible/clipped UI is obvious in CI logs.

---

**Last Updated**: 2026-05-10
