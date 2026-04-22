# UI Test Runbook

**Date:** 2026-04-20  
**Scope:** Browser smoke tests, screenshots, and visual QA for the Llama Monitor web UI.

## Prerequisites

The UI tests live in `tests/ui` and use Playwright.

Install once:

```bash
cd tests/ui
npm install
```

If browsers are missing:

```bash
cd tests/ui
npx playwright install
```

## Start The App

Static UI assets are embedded into the Rust binary at compile time. After changing `static/index.html`, `static/style.css`, or `static/app.js`, restart the app before testing.

Recommended smoke server:

```bash
cargo run -- --headless --port 7778
```

Leave it running in one terminal.

## Run UI Tests

In another terminal:

```bash
cd tests/ui
LLAMA_MONITOR_UI_URL=http://127.0.0.1:7778 npm test
```

List tests without running:

```bash
cd tests/ui
npm test -- --list
```

Run one test by title:

```bash
cd tests/ui
LLAMA_MONITOR_UI_URL=http://127.0.0.1:7778 npm test -- -g "settings opens"
```

## Optional SSH Integration Test

The normal UI suite must not require SSH access. To test the remote-agent SSH path against the same machine or a disposable runner, set an explicit SSH target and run the gated integration spec:

```bash
cd tests/ui
LLAMA_MONITOR_UI_URL=http://127.0.0.1:7778 \
LLAMA_MONITOR_SSH_TARGET=user@127.0.0.1 \
npm run test:ssh
```

For password or private-key auth through the dedicated SSH backend:

```bash
LLAMA_MONITOR_UI_URL=http://127.0.0.1:7778 \
LLAMA_MONITOR_SSH_TARGET=ssh://user@127.0.0.1:22 \
LLAMA_MONITOR_SSH_PASSWORD='password-for-this-test-only' \
npm run test:ssh
```

```bash
LLAMA_MONITOR_UI_URL=http://127.0.0.1:7778 \
LLAMA_MONITOR_SSH_TARGET=ssh://user@127.0.0.1:22 \
LLAMA_MONITOR_SSH_KEY_PATH="$HOME/.ssh/id_ed25519" \
npm run test:ssh
```

Use a documentation-safe target in committed tests. Do not hardcode personal usernames, private hostnames, or private LAN IPs.

This test scans and trusts the SSH host key, then calls `/api/remote-agent/detect`. It verifies that the app can use the dedicated SSH backend to classify the target OS, architecture, install path, and basic agent state. It does not install binaries, start processes, or open firewall ports.

Trusted test host keys are persisted by the app in `~/.config/llama-monitor/ssh-known-hosts.json`.

For local self-SSH, the machine must already allow SSH login for that target. On macOS this usually means Remote Login is enabled and `ssh user@127.0.0.1 true` works in a terminal.

Run headed for debugging:

```bash
cd tests/ui
LLAMA_MONITOR_UI_URL=http://127.0.0.1:7778 npm run test:headed
```

## Screenshots For Analysis

Desktop screenshot:

```bash
npx playwright screenshot --wait-for-timeout 1000 http://127.0.0.1:7778 /tmp/llama-monitor-desktop.png
```

Mobile screenshot:

```bash
npx playwright screenshot --viewport-size 390,844 --wait-for-timeout 1000 http://127.0.0.1:7778 /tmp/llama-monitor-mobile.png
```

Use the local image viewer tool on those files when working as an AI agent.

## What To Check Visually

- Endpoint strip reads as status, not an editable input.
- Top nav is styled and not browser-default controls.
- Sidebar is visible on desktop and horizontal/usable on mobile.
- Server, Chat, and Logs tabs switch cleanly.
- Settings, Sessions, Models, and Profile controls open without console errors.
- Metric cards and tables do not overlap at desktop or mobile widths.
- Remote/local status badges are legible and not misleading.

## Browser Console Errors

Treat app errors as blockers:

- `ReferenceError`
- `TypeError`
- failed modal open/close handlers
- missing DOM IDs

Ignore browser-extension noise such as:

```text
content.js: Unsupported site: localhost
```

That message is from an extension, not Llama Monitor.

## Artifacts

Playwright artifacts are ignored by git:

- `test-results/`
- `playwright-report/`
- `blob-report/`
- the same directories under `tests/ui/`

Do not commit generated screenshots unless they are intentionally added as documentation.

## Cleanup

Find the smoke server:

```bash
lsof -tiTCP:7778 -sTCP:LISTEN
```

Stop it:

```bash
kill -9 <pid>
```

Only use `kill -9` for the local smoke server you started.

## Definition Of Done

For UI work, finish with:

```bash
cargo fmt -- --check
cargo check
rg -o 'id="[^"]+"' static/index.html | sort | uniq -c | awk '$1 > 1'
cd tests/ui && LLAMA_MONITOR_UI_URL=http://127.0.0.1:7778 npm test
```

Also capture and inspect at least one desktop and one mobile screenshot for layout changes.
