# Plan: Real-time model loading progress (via llama.cpp /models/sse)

Based on llama.cpp PR #24828 (merged) — "server: real-time model load progress tracking via /models/sse".

## Goal

Replace the opaque "Starting llama-server…" loading state with a live progress indicator
that shows what the server is doing and how far along it is while loading a model.

## Why

- Model loading (esp. large models) is the longest user-facing wait.
- Today we: show a toast, then poll /health. No idea if 1% or 99%.
- llama.cpp now exposes structured SSE events for exactly this — no log scraping.

## PR #24828 — what it gives us

New SSE events on `/models/sse` during loading:

- Stages:
  - `fit_params` — context/VRAM estimation
  - `text_model` — main model weights (0.0 → 1.0)
  - `mmproj_model` — multimodal projector (if applicable)
  - `spec_model` — speculative/draft model (if applicable)
- Events:
  - `model_status` with `status: "loading"` / `"loaded"` / `"sleeping"` / `"unloaded"`
  - `status_change` with:
    - `progress.stage`
    - `progress.value` (for stages that support it)
    - `info` on `loaded`

Details: diff in llama.cpp PR #24828; summary in the SSE examples there.

## Design overview

Three pieces:

1. Backend SSE consumer (Rust)
2. WebSocket broadcast to frontend
3. Frontend progress UI (JS/CSS)

All additive; no breaking changes.

### 1. Backend: SSE consumer task

Location: `src/llama/server.rs`, in `start_server`.

- After spawning llama-server (when `server_running` becomes true), spawn a Tokio task:
  - Connect to: `http://127.0.0.1:{port}/models/sse`
  - Use reqwest with SSE-style reading (streaming, newlines).
- On each event:
  - Parse JSON envelope:
    - `{ "model": "...", "event": "model_status", "data": { "status": "loading", "progress": { "stage": "text_model", "value": 0.42 } } }`
  - Extract:
    - `status`: loading/loaded/sleeping/unloaded
    - `progress.stage`: fit_params/text_model/mmproj_model/spec_model
    - `progress.value`: 0.0–1.0 (optional)
- If parse fails or the endpoint is missing:
  - Silently ignore; fallback is existing readiness flow.

Implementation notes:

- Introduce a small struct in `src/llama/server.rs`:

  - `ServerLoadingProgress`:
    - `stage: String`
    - `value: f32` (or None)
    - `status: String` ("loading" / "loaded" / "sleeping" / "unloaded")

- The SSE task:
  - On `model_status` / `status_change` with `progress`:
    - Send over a oneshot/broadcast channel into the main event loop,
      or publish via the existing WebSocket broadcaster.

### 2. WebSocket broadcast

Location: `src/web/ws.rs`

- Extend the existing message types with a new variant:

  - `WsMessage::ServerLoadingProgress { stage, value, status }`

- Wire the SSE consumer to emit this message when:
  - A new stage is reached, or
  - `value` changes (with some debounce, e.g. ≥5% or ≥500ms delta).

- Frontend already receives:
  - Logs
  - Metrics
  - Session status
  → this is a natural extension.

### 3. Frontend progress UI

Where to show it:

- During:
  - Spawn wizard start
  - Preset model launch
  - Tune panel restarts

High-level behavior:

- When the first `ServerLoadingProgress` message arrives after a start/restart:
  - Hide or overlay the "Starting llama-server…" toast with:
    - A compact progress bar + label in the status area
    - Or directly inside the chat startup area if that's cleaner.
- When loading completes (`status == "loaded"`):
  - Fade out the progress bar within 1–2 seconds.

Mapping stages → user labels:

- `fit_params`:
  - "Preparing context and VRAM estimation…"
- `text_model` (with value):
  - "Loading model weights… {value*100 as u8}%"
- `mmproj_model`:
  - "Loading multimodal projector…"
- `spec_model`:
  - "Loading speculative model…"
- `loaded`:
  - "Model loaded — server ready."

Technical notes:

- Implement as:
  - A new small module: `static/js/features/loading-progress.js`
  - DOM elements:
    - Container: `.server-loading-progress`
    - Bar: `.server-loading-progress-bar`
    - Label: `.server-loading-progress-label`
  - Behavior:
    - On first progress event → show container, set label, update bar width.
    - On subsequent events → update width + label.
    - On `loaded` → set width 100%, wait, then hide container.
- Use existing toast/status area or a dedicated stripe near the top.
- Respect reduced motion:
  - If `prefers-reduced-motion: reduce` → no animations; just bar + label.

Fallback:

- If we never receive progress events:
  - Fall back to existing behavior:
    - "Starting llama-server…"
    - Readiness polling
    - Error with last log lines on timeout.
- This ensures backward compatibility with older llama.cpp builds.

## File change summary

Key files affected (no code written yet):

- `src/llama/server.rs`
  - Spawn SSE consumer task after server start.
- `src/web/ws.rs`
  - Add new WsMessage variant for loading progress.
- `static/js/features/loading-progress.js`
  - Handle WebSocket progress events; render/update progress bar.
- `static/js/bootstrap.js`
  - Import new `loading-progress.js`.
- `static/css/features/loading-progress.css` (or extend existing CSS)
  - Styles for progress container, bar, label.
- `static/index.html` or layout:
  - Insert `.server-loading-progress` container in header/status area.
- `build.rs` scan:
  - Include new static files; commit generated `src/gen/*.rs`.

## Non-goals (for now)

- Per-model persistent progress history.
- Using progress to estimate ETA.
- Changes to auth, API shape, or spawn behavior.
