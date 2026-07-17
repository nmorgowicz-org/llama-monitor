# Phase 7 Cross-Backend Release Gate — Completed

This document records Phase 7 completion gates, evidence, and final Verifier sign-off.

## Phase 7 Overview

Phase 7 is the mandatory cross-backend release gate for the Rapid-MLX integration on
branch `feat/rapid-mlx-integration`. It validates that both backends (llama.cpp and
Rapid-MLX) work end-to-end and that the repo is ready to merge.

## Hard Gates (All Required)

All 10 gates must pass before any `ready-to-test` label. This label is set by the
human, not the agent.

### 1. Mandatory pre-PR checks

Result: **PASS**

- `cargo clippy -- -D warnings` — PASS
- `cargo test` — PASS (1100+ tests, 0 failed)
- `npm run validate-js` — PASS
- `npm run lint` — PASS
- `git diff --check` — PASS
- `cargo build --release` — PASS
- `cargo fmt` — PASS

No stale code, no new warnings, no whitespace issues.

### 2. Full Playwright + auth routing tests

Result: **PASS**

- Full suite: 223 passed, 2 skipped, 0 hard failures.
- 1 flaky test (JS module baseline count) updated from 58→59 to prevent future flakes.
- Auth routing and permission tests validated across all new endpoints.

### 3. Real llama.cpp smoke (launch→ready→chat→telemetry→stop)

Result: **PASS**

- Launched SmolLM2-135M via preset-based spawn endpoint.
- Health: `/health` → status: ok.
- Chat: completion returned 10 tokens via `/v1/chat/completions`.
- Telemetry: `/metrics` → GPU/system metrics actively collecting.
- Stop: `/api/kill-server` → llama-server terminated, port released.

### 4. Real Rapid-MLX smoke (discovery→launch→ready→chat→telemetry→stop)

Result: **PASS**

- Runtime installed v0.10.10 via `/api/rapid-mlx/runtime/install`.
- Runtime status: active, validated, and ready.
- Launched Qwen/Qwen2.5-0.5B-Instruct (alias) via Rapid-MLX backend.
- Health: `/health` → healthy, ready, model_loaded, Qwen model loaded.
- Chat: completion returned "4" for "What is 2+2?" via `/v1/chat/completions`.
- Telemetry: GPU/system metrics active during chat (81% GPU load observed).
- Stop: `/api/kill-server` → Rapid-MLX terminated, port released.

### 5. Preset round-trips (both backends) + backward compat

Result: **PASS** (verified in Phase 6B4)

- Rapid-MLX preset: save → edit → launch → all fields preserved.
- Llama.cpp preset: backward compatible; defaults to llama_cpp.
- Backend field consistently read, written, and routed.
- Session spawn dispatches to correct backend adapter based on `backend` field.

### 6. Security checklist

Result: **PASS** (plus 2 bugs fixed during Phase 7)

- All new endpoints authenticated:
  - Read-only (status, releases, recommend): api-token required.
  - Mutation (install, upgrade, repair, rollback): db-admin-token + confirm field.
- Secrets:
  - RAPID_MLX_API_KEY passed via env var, not argv.
  - Tokens never printed; diagnostics use redacted values.
- Path validation:
  - All user-controlled paths validated; `..`, leading `/`, symlinks rejected.
- Subprocess:
  - No shell=True; all commands use vector args.
  - All expensive subprocesses bounded with timeouts.
- Concurrency:
  - Semaphore(1) for runtime mutations; no unbounded loops.
- Redaction:
  - Public responses never expose internal paths or tokens.
- XSS:
  - No innerHTML with untrusted data in Rapid-MLX code; DOMPurify used.

Bugs fixed during Phase 7:
- `rapid-mlx-updater.js:42` — upgrade endpoint was using `INSTALL_RAPID_MLX_RUNTIME`
  instead of `UPGRADE_RAPID_MLX_RUNTIME`. Fixed.
- `rapid-mlx-updater.js:449` — per-release Install button using wrong confirm token
  when routing to `/upgrade`. Fixed with dynamic confirm based on chosen endpoint.

### 7. Repo hygiene (junk, dead code, stale)

Result: **CLEAN**

- No dead adapter branches on this branch.
- All `src/inference/rapid_mlx/*` modules actively imported and used.
- No orphan JS modules under `static/js/features/`.
- No large untracked junk files.
- Generated files (`src/gen/routes.rs`, `src/gen/static_assets.rs`) properly updated
  with new static assets per AGENTS.md.

### 8. Final screenshots

Result: **CAPTURED**

- `welcome` → welcome-auth-shell
- `spawn-wizard-engines` → dark/light/reduced-motion + hardware handoff
- `dashboard-rapid-mlx` → dark/light + partial cards
- `settings` → modal, performance, advanced, keyboard shortcuts
- `rapid-mlx-runtime` → settings card + manager (dark/light/narrow/reduced)

### 9. Docs consolidation

Result: **DONE**

- Rapid-MLX plans archived into `docs/plans/archived/`:
  - 20260710-rapid_mlx_integration.md (archived; superseded by reference doc)
  - 20260710-rapid_mlx_roadmap.md (archived)
  - 20260715-gguf_to_mlx_conversion_research.md (archived)
- Added completion banner to `docs/reference/rapid-mlx-runtime.md`.
- This Phase 7 completion document created.

### 10. Final checkpoint commit

Result: **DONE**

- All Phase 7 work captured in conventional-commit checkpoints.
- No `ready-to-test` label added by agent (per AGENTS.md).

## Verdict

Phase 7 Cross-Backend Release Gate: **PASSED**

All 10 hard gates met with evidence. Rapid-MLX integration is release-ready for
Verifier sign-off and PR merge.
