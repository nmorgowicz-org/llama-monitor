# 2026-08-05 — Capture Harness: Local Runtime Options (addendum)

Branch: feat/rapid-mlx-integration
Status: PLANNED (future work, non-urgent — addendum, not part of the active Tier 5/6 defect-verification sweep)

## Context

`tests/ui/capture/harness/attach.mjs` exposes exactly one way to get live telemetry/chat into a capture: `attachToServer(page)`, which drives `#setup-endpoint-url` + `doAttachFromSetup()` against `REMOTE_SERVER` (`process.env.REMOTE_SERVER || 'http://192.168.2.16:8001'`, defined in `harness/paths.mjs`). ~20 scenarios import it (`core/dashboard`, `core/chat`, `core/sidebar`, `core/panels`, `core/smoke`, `core/guided-gen`, `core/navbar`, `config/settings`, `config/tls`, `config/appearance-palette`, `features/*`, `models/models-v2`, …). When that physical box is busy or off, those scenarios throw `Attach API request timed out` after 45s, or capture stale UI.

The repo already contains a working *local spawn* capture path: `scenarios/wizard-rapidmlx/rapid-mlx-live.mjs` seeds a preset via `POST /api/presets`, refreshes it with `loadPresets()`, calls `doStart()` from `/js/features/attach-detach.js` (which POSTs `/api/sessions/spawn` with an admin token), waits via `waitForRapidTelemetry()` in `harness/shot.mjs`, chats, then `doStop()` + `deleteRapidLiveTestPreset()` in a `finally`. No equivalent exists for llama.cpp, and none of it is reusable by ordinary scenarios today.

**Goal**: make the telemetry source a **selectable strategy** with three implementations — remote attach (unchanged default), local llama.cpp spawn, local MLX spawn — without deprecating the remote path. Additive only; the user explicitly wants all three retained as options.

## Design

### 1. New module `tests/ui/capture/harness/source.mjs`

Single entry point every scenario calls instead of `attachToServer`:

```js
export async function connectSource(page, opts = {}) -> { kind, teardown() }
```

Resolution order (first match wins):
1. `opts.force` (scenario hard-requires a source, e.g. `rapid-mlx-live` forces `local-mlx`).
2. `--source <remote|local-llamacpp|local-mlx|auto>` CLI flag parsed in `index.mjs::parseArgs`.
3. `CAPTURE_SOURCE` env var.
4. Per-scenario default declared in the `SCENARIOS` registry (`source: 'remote'`).
5. Global default: `remote` — **unchanged behaviour**, so every existing invocation and CI usage keeps working byte-for-byte.

`auto` = probe `REMOTE_SERVER` with a 3s `fetch('/health')`; if unreachable, fall back to `local-llamacpp`, then `local-mlx`, then throw with a message naming all three and how to select one explicitly.

Each strategy returns a uniform handle so scenarios stay source-agnostic; `teardown()` is a no-op for remote and does `doStop()` + preset delete for local ones. `attach.mjs::attachToServer` stays exported and unchanged — `source.mjs` just wraps it as the `remote` strategy. No scenario is forced to migrate in one go.

### 2. `local-llamacpp` strategy (`harness/local-llamacpp.mjs`)

Reuses the existing spawn mechanism, does not reinvent it:

- **Model**: resolved from `CAPTURE_LOCAL_GGUF`, else a conventional path `~/.config/llama-monitor/models/gguf/<capture model>`. Recommend **SmolLM2-135M-Instruct-Q8_0.gguf (~145 MB)** or **Qwen2.5-0.5B-Instruct-Q4_K_M (~400 MB)**. 135M loads in well under 2s on M5 Max and produces real (if dumb) tokens — fine for screenshots, where the *shape* of the UI matters, not answer quality. Where a scenario shows model-name chrome, note that "SmolLM2-135M" will appear in the screenshot; scenarios that need a plausible-looking model name should stay on `remote`.
- **Binary**: the harness spawns llama-monitor with `HOME=TEMP_HOME`, and `seedConfig()` does **not** copy `~/.config/llama-monitor/bin/`. So pass `['--llama-server-path', <real llama-server>]` (and `--llama-server-cwd`) as `extraArgs` from the scenario `setup()` — same mechanism `seedModelsDirFixture()` already uses. Default to `~/.config/llama-monitor/bin/llama-server`, overridable via `CAPTURE_LLAMA_SERVER_PATH`.
- **Flow**: `POST /api/presets` with `{ id: 'capture-local-llamacpp', backend: 'llama_cpp', model_path, port, context_size: 4096, ... }` → `loadPresets(id)` → `doStart()` → poll `/api/sessions/active` until `status === 'Running'` → optional `/v1/models` probe on the spawned port.
- **Port**: derive from the harness port exactly like `rapid-mlx-live` does (`9321 + (harnessPort - 8892)`) to avoid collisions across parallel runs; pre-kill stragglers with the same `lsof … | xargs kill -9` guard.

### 3. `local-mlx` strategy (`harness/local-mlx.mjs`)

Straight extraction of the seed/start/wait/stop block already proven in `rapid-mlx-live.mjs`:

- Gate on `GET /api/llama-binary/platform-info` → `rapid_mlx_local_available`; if false, either skip (developer scenarios) or fall through to `local-llamacpp` (under `auto`).
- Model default `mlx-community/Qwen3-0.6B-4bit` (~400 MB, already the scenario's default) overridable via `RAPID_MLX_LIVE_MODEL` / `RAPID_MLX_LIVE_MODEL_PATH` — keep those env names, they are already documented elsewhere.
- Reuse `waitForRapidTelemetry()` and `deleteRapidLiveTestPreset()` from `harness/shot.mjs` rather than duplicating; consider moving both into `source.mjs`/`local-mlx.mjs` as part of the Phase A split hygiene, re-exporting from `shot.mjs` for compatibility.

### 4. Scenario migration

Mechanical, one line per scenario: `await attachToServer(page)` → `const src = await connectSource(page)`, and add `try/finally { await src.teardown(); }` where the scenario spawns anything. Migrate in three batches so a regression is bisectable:
- Batch 1 (proof): `core/dashboard`, `core/chat`, `core/smoke`.
- Batch 2: remaining `core/` + `config/`.
- Batch 3: `features/` + `models/`.

`core/navbar` already tolerates attach failure (`try { … } catch`); keep that tolerance.

## Timing, disk, portability trade-offs

| Source | Time to live telemetry | Disk | Portable? |
|---|---|---|---|
| remote (192.168.2.16:8001) | ~2–5s when the box is free; 45s timeout when it is not | 0 | no (LAN-only, single user) |
| local-llamacpp + SmolLM2-135M | ~3–6s (spawn + load) | ~145 MB | yes on any llama.cpp platform |
| local-mlx + Qwen3-0.6B-4bit | ~10–25s first run (HF fetch), ~5–10s cached | ~400 MB + HF cache | Apple Silicon only |

Both local paths are *faster and more deterministic* than a contended remote attach, but they add a model artifact. Recommendation: **do not commit models to git and do not auto-download in CI.** Ship `scripts/fetch-capture-model.mjs` (or a documented `hf download` one-liner) plus a clear error from `local-llamacpp` when the GGUF is absent, telling the operator the exact command to fetch it. CI keeps using `remote`/`--no-attach` scenarios only; local sources are a developer-machine affordance.

## Keeping remote first-class

- Default stays `remote`; no existing command line invocation changes meaning.
- `attachToServer` remains exported with its current signature and `REMOTE_SERVER` default.
- `printUsage()` gains a "Telemetry sources" section documenting all three as equal peers, not one as legacy.
- Add `--source remote` explicitly to a couple of doc examples so it reads as a deliberate choice, not a fallback.

## Risks / open questions

- `doStart()` requires an admin token via `/api/db/admin-token`; confirm it resolves under the temp-HOME seeded config for both local strategies (rapid-mlx-live's success implies yes, but should be re-verified for the llama.cpp path specifically).
- Screenshots become model-name-dependent; audit which captures render the model identity in-frame before switching them off `remote`.
- Parallel capture runs must not collide on inference ports — the derived-port scheme only covers this if harness ports also differ.
- `seedConfig()` may need `bin/` handling if we ever want local llama.cpp without the CLI `--llama-server-path` override.

## Phasing

1. `source.mjs` + `remote` strategy only; migrate Batch 1; prove zero behaviour change.
2. `local-llamacpp.mjs` + model fetch script + docs.
3. `local-mlx.mjs` extracted from `rapid-mlx-live.mjs`; refactor that scenario to consume it.
4. `auto` probing + Batches 2–3 + `printUsage()` rewrite.

## Critical files for implementation

- `tests/ui/capture/harness/attach.mjs`
- `tests/ui/capture/harness/paths.mjs`
- `tests/ui/capture/index.mjs`
- `tests/ui/capture/scenarios/wizard-rapidmlx/rapid-mlx-live.mjs`
- `tests/ui/capture/harness/shot.mjs`
## Phase 1 progress (2026-08-09)

Implemented the source-selection seam in `tests/ui/capture/harness/source.mjs` and migrated the proof scenarios `chat`, `dashboard`, and `smoke`. Remote attach remains the only implemented strategy and the default; `--source`, `CAPTURE_SOURCE`, scenario defaults, and force precedence are now represented in the contract. The source handle exposes `kind` and `teardown()` so later local strategies can own process/preset cleanup without changing scenario code.

Validation: release build passed; source-contract/argument parsing checks passed; smoke capture passed with `--no-attach`; JS validation and lint passed. A remote dashboard capture was attempted but the configured remote endpoint did not complete the existing 45-second `/api/attach` handshake, so the live remote receipts remain pending an available runtime. Local llama.cpp and local MLX strategies remain Phase 2/3 work.

### Remote attach timeout update (2026-08-09)

The default `/api/attach` wait is now 120 seconds, with `CAPTURE_ATTACH_TIMEOUT_MS` available for slower or faster environments. The earlier 45-second reference describes the pre-update capture attempt. Timeout cleanup removes the response listener before rejecting, preventing stale handlers during retries.

### Phase 1 remote proof closure (2026-08-09)

The attach ordering defect was fixed: migrated scenarios now navigate to the app before invoking `connectSource()`, matching the original attach contract. This supersedes the earlier pending-capture note. Fresh release-built captures passed sequentially for `dashboard`, `chat`, and `smoke` using `--source remote` with the configured endpoint. The Phase 1 remote strategy gate is closed; local llama.cpp and local MLX strategies remain Phase 2/3 work.
